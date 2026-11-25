use std::{
    sync::Arc,
    task::{Context as TaskContext, Poll, Wake, Waker},
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use eframe::egui;
use tokio::{runtime::Runtime, task::JoinSet};

use super::{probe, state::TargetState};
use crate::config::{Config, Endpoint};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Refresh {
    Inactive,
    Due,
    Waiting { remaining: Duration, progress: f32 },
}

struct Report {
    index: usize,
    result: Result<String, String>,
    elapsed: Duration,
    finished: Instant,
}

struct RepaintOnCompletion(egui::Context);

impl Wake for RepaintOnCompletion {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.request_repaint_of(egui::ViewportId::ROOT);
    }
}

pub struct Monitor {
    runtime: Runtime,
    client: Arc<probe::HttpClient>,
    jobs: JoinSet<Report>,
    completion_waker: Option<Waker>,
    pub states: Vec<TargetState>,
    next_refresh: Instant,
    busy: Vec<bool>,
}

impl Monitor {
    pub fn new(config: &Config) -> Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .context("Could not create the monitoring runtime")?;
        Ok(Self {
            runtime,
            client: Arc::default(),
            jobs: JoinSet::new(),
            completion_waker: None,
            states: config
                .targets
                .iter()
                .map(|_| TargetState::default())
                .collect(),
            next_refresh: Instant::now(),
            busy: vec![false; config.targets.len()],
        })
    }

    pub fn reset(&mut self, config: &Config, preserve_states: bool) {
        // Dropping the set cancels old jobs, including in-flight ping subprocesses.
        self.jobs = JoinSet::new();
        if !config
            .targets
            .iter()
            .any(|target| matches!(target.endpoint, Endpoint::Http { .. }))
        {
            self.client = Arc::default();
        }
        if !preserve_states {
            self.states = config
                .targets
                .iter()
                .map(|_| TargetState::default())
                .collect();
        }
        self.next_refresh = Instant::now();
        self.busy = vec![false; config.targets.len()];
    }

    pub fn reconfigure(&mut self, previous_config: &Config, config: &Config) {
        let mut previous = previous_config
            .targets
            .iter()
            .zip(std::mem::take(&mut self.states))
            .collect::<Vec<_>>();
        self.states = config
            .targets
            .iter()
            .map(|target| {
                previous
                    .iter()
                    .position(|(old, _)| *old == target)
                    .map(|index| previous.remove(index).1)
                    .unwrap_or_default()
            })
            .collect();
        self.reset(config, true);
    }

    pub fn refresh(&self, now: Instant, interval: Duration) -> Refresh {
        if self.busy.is_empty() {
            return Refresh::Inactive;
        }
        let remaining = self.next_refresh.saturating_duration_since(now);
        if remaining.is_zero() {
            Refresh::Due
        } else {
            Refresh::Waiting {
                remaining,
                progress: interval.saturating_sub(remaining).as_secs_f32() / interval.as_secs_f32(),
            }
        }
    }

    pub fn tick(&mut self, config: &Config, ctx: &egui::Context) -> Result<()> {
        let now = Instant::now();
        let round_due = !self.busy.is_empty() && now >= self.next_refresh;
        if !round_due && self.jobs.is_empty() {
            return Ok(());
        }
        // Register for completion, including panics, before relying on repaint notifications.
        let waker = self
            .completion_waker
            .get_or_insert_with(|| Waker::from(Arc::new(RepaintOnCompletion(ctx.clone()))))
            .clone();
        self.collect_reports(config, &waker)?;
        if !round_due {
            return Ok(());
        }

        // Start the next countdown immediately, even if every target is still busy.
        self.next_refresh = now + Duration::from_secs(config.interval_seconds);
        let timeout = Duration::from_secs(config.timeout_seconds);
        for (index, (target, busy)) in config.targets.iter().zip(&mut self.busy).enumerate() {
            if *busy {
                continue;
            }
            *busy = true;
            let client = Arc::clone(&self.client);
            let endpoint = target.endpoint.clone();
            self.jobs.spawn_on(
                async move {
                    let start = Instant::now();
                    let result = probe::check(&client, &endpoint, timeout)
                        .await
                        .map_err(|error| format!("{error:#}"));
                    Report {
                        index,
                        result,
                        elapsed: start.elapsed(),
                        finished: Instant::now(),
                    }
                },
                self.runtime.handle(),
            );
        }
        self.collect_reports(config, &waker)
    }

    fn collect_reports(&mut self, config: &Config, waker: &Waker) -> Result<()> {
        let mut cx = TaskContext::from_waker(waker);
        while let Poll::Ready(Some(report)) = self.jobs.poll_join_next(&mut cx) {
            match report {
                Ok(report) => {
                    self.states[report.index].record(
                        report.result,
                        report.elapsed,
                        report.finished,
                    );
                    self.busy[report.index] = false;
                }
                Err(error) => {
                    self.reset(config, true);
                    return Err(error).context("A monitoring task was interrupted");
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
