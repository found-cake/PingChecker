use super::ui_test_harness::Harness;
use super::*;
use crate::{
    checker::state::Health,
    config::{Endpoint, Target},
};
use anyhow::{Context, Result};
use std::time::Instant;

#[test]
fn check_now_cancels_pending_work_without_clearing_outage_state() -> Result<()> {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0))?;
    let mut h = Harness::new()?;
    let mut config = h.app.config.clone();
    config.targets.push(Target {
        name: "local".into(),
        endpoint: Endpoint::Tcp {
            host: "127.0.0.1".into(),
            port: listener.local_addr()?.port(),
        },
    });
    assert!(h.app.save(config));
    let monitor = h.app.monitor.as_mut().context("monitor")?;
    monitor.states.first_mut().context("state")?.record(
        Err("offline".into()),
        Duration::ZERO,
        Instant::now(),
    );
    monitor.tick(&h.app.config, &h.ctx)?;
    let interval = Duration::from_secs(h.app.config.interval_seconds);
    assert_eq!(
        monitor.refresh(Instant::now(), interval),
        crate::checker::monitor::Refresh::Checking { count: 1 }
    );
    h.frame(Vec::new());
    h.frame(Vec::new());

    h.click("Check now")?;

    let monitor = h.app.monitor.as_ref().context("monitor")?;
    assert_eq!(
        monitor.refresh(Instant::now(), interval),
        crate::checker::monitor::Refresh::Due
    );
    let state = monitor.states.first().context("state")?;
    assert_eq!(state.health, Health::Down);
    assert_eq!(state.consecutive_failures, 1);
    Ok(())
}
#[test]
fn removing_a_target_through_ui_updates_saved_and_live_targets() -> Result<()> {
    let mut h = Harness::new()?;
    let mut config = h.app.config.clone();
    config.targets.push(Target {
        name: "local".into(),
        endpoint: Endpoint::Icmp {
            host: "127.0.0.1".into(),
        },
    });
    assert!(h.app.save(config));
    h.frame(Vec::new());
    h.frame(Vec::new());

    h.click("Remove")?;

    assert!(h.app.config.targets.is_empty());
    assert!(h.app.monitor.as_ref().context("monitor")?.states.is_empty());
    assert!(
        Config::load(h.app.path.as_ref().context("path")?)?
            .targets
            .is_empty()
    );
    Ok(())
}

#[test]
fn adding_and_deleting_other_targets_preserves_an_alarmed_outage() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("ping.yml");
    let healthy = Target {
        name: "healthy".into(),
        endpoint: Endpoint::Tcp {
            host: "127.0.0.1".into(),
            port: 80,
        },
    };
    let failing = Target {
        name: "failing".into(),
        endpoint: Endpoint::Icmp {
            host: "127.0.0.1".into(),
        },
    };
    let added = Target {
        name: "new".into(),
        endpoint: Endpoint::Http {
            url: "http://localhost/health".into(),
        },
    };
    let config = Config {
        targets: vec![healthy.clone(), failing.clone()],
        ..Config::default()
    };
    config.save(&path)?;
    let mut monitor = Monitor::new(&config)?;
    let start = Instant::now();
    monitor.states.first_mut().context("healthy state")?.record(
        Ok("connected".into()),
        Duration::ZERO,
        start,
    );
    let state = monitor.states.get_mut(1).context("failing state")?;
    for seconds in 0..3 {
        state.record(
            Err("offline".into()),
            Duration::from_millis(17),
            start + Duration::from_secs(seconds),
        );
    }
    assert!(state.alarm_due(3, true));
    let checked_at = state.checked_at;
    let mut app = App {
        config,
        path: Some(path),
        monitor: Some(monitor),
        message: None,
        writable: true,
        dialog: None,
        beeper: Beeper::default(),
    };

    // Add C, remove the earlier healthy target (shifting A), then remove C.
    for targets in [
        vec![healthy, failing.clone(), added.clone()],
        vec![failing.clone(), added.clone()],
        vec![failing.clone()],
    ] {
        assert!(app.save(Config {
            targets,
            ..app.config.clone()
        }));

        let index = app
            .config
            .targets
            .iter()
            .position(|target| target == &failing)
            .context("surviving target")?;
        let monitor = app.monitor.as_mut().context("monitor")?;
        let state = monitor.states.get_mut(index).context("surviving state")?;
        assert_eq!(state.health, Health::Down);
        assert_eq!(state.consecutive_failures, 3);
        assert_eq!(state.down_since, Some(start));
        assert_eq!(state.checked_at, checked_at);
        assert_eq!(state.detail, "offline");
        assert_eq!(state.latency, Some(Duration::from_millis(17)));
        assert!(state.alarmed);
        assert!(
            !state.alarm_due(3, true),
            "the same outage must not alarm again"
        );
        if let Some(index) = app
            .config
            .targets
            .iter()
            .position(|target| target == &added)
        {
            let state = monitor.states.get(index).context("new state")?;
            assert_eq!(state.health, Health::Pending);
            assert_eq!(state.consecutive_failures, 0);
            assert!(!state.alarmed);
        }
    }
    Ok(())
}
