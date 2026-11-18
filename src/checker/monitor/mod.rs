use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use eframe::egui;
use tokio::{runtime::Runtime, task::JoinSet};

use super::{probe, state::TargetState};
use crate::config::Config;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Refresh {
    Inactive,
    Due,
    Checking {
        count: usize,
    },
    Waiting {
        remaining: Duration,
        progress: f32,
        checking: usize,
    },
}

struct Report {
    index: usize,
    result: Result<String, String>,
    elapsed: Duration,
    finished: Instant,
}

pub struct Monitor {
    runtime: Runtime,
    client: reqwest::Client,
    jobs: JoinSet<Report>,
    pub states: Vec<TargetState>,
    due: Vec<Instant>,
    busy: Vec<bool>,
}

impl Monitor {
    pub fn new(config: &Config) -> Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .context("Could not create the monitoring runtime")?;
        let client = {
            let _guard = runtime.enter();
            reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::limited(5))
                .timeout(Duration::from_secs(60))
                .connect_timeout(Duration::from_secs(60))
                .pool_max_idle_per_host(1)
                .user_agent(concat!(
                    env!("CARGO_PKG_NAME"),
                    "/",
                    env!("CARGO_PKG_VERSION")
                ))
                .build()
                .context("Could not create the HTTP client")?
        };
        Ok(Self {
            runtime,
            client,
            jobs: JoinSet::new(),
            states: config
                .targets
                .iter()
                .map(|_| TargetState::default())
                .collect(),
            due: vec![Instant::now(); config.targets.len()],
            busy: vec![false; config.targets.len()],
        })
    }

    pub fn reset(&mut self, config: &Config, preserve_states: bool) {
        // Dropping the set cancels old jobs, including in-flight ping subprocesses.
        self.jobs = JoinSet::new();
        if !preserve_states {
            self.states = config
                .targets
                .iter()
                .map(|_| TargetState::default())
                .collect();
        }
        self.due = vec![Instant::now(); config.targets.len()];
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
        let checking = self.busy.iter().filter(|busy| **busy).count();
        let deadline = self
            .due
            .iter()
            .zip(&self.busy)
            .filter_map(|(due, busy)| (!busy).then_some(*due))
            .min();
        match deadline {
            None if self.due.is_empty() => Refresh::Inactive,
            None => Refresh::Checking { count: checking },
            Some(due) if due <= now => Refresh::Due,
            Some(due) => {
                let remaining = due.duration_since(now);
                Refresh::Waiting {
                    remaining,
                    progress: interval.saturating_sub(remaining).as_secs_f32()
                        / interval.as_secs_f32(),
                    checking,
                }
            }
        }
    }

    pub fn tick(&mut self, config: &Config, ctx: &egui::Context) -> Result<()> {
        while let Some(report) = self.jobs.try_join_next() {
            match report {
                Ok(report) => {
                    if let Some(state) = self.states.get_mut(report.index) {
                        state.record(report.result, report.elapsed, report.finished);
                    }
                    if let Some(busy) = self.busy.get_mut(report.index) {
                        *busy = false;
                    }
                }
                Err(error) => {
                    self.reset(config, true);
                    return Err(error).context("A monitoring task was interrupted");
                }
            }
        }
        let now = Instant::now();
        for (index, target) in config.targets.iter().enumerate() {
            let (Some(due), Some(busy)) = (self.due.get_mut(index), self.busy.get_mut(index))
            else {
                continue;
            };
            if *busy || now < *due {
                continue;
            }
            *busy = true;
            *due = now + Duration::from_secs(config.interval_seconds);
            let client = self.client.clone();
            let endpoint = target.endpoint.clone();
            let timeout = Duration::from_secs(config.timeout_seconds);
            let ctx = ctx.clone();
            self.jobs.spawn_on(
                async move {
                    let start = Instant::now();
                    let result = probe::check(&client, &endpoint, timeout)
                        .await
                        .map_err(|error| format!("{error:#}"));
                    let report = Report {
                        index,
                        result,
                        elapsed: start.elapsed(),
                        finished: Instant::now(),
                    };
                    ctx.request_repaint();
                    report
                },
                self.runtime.handle(),
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
