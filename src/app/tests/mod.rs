use super::*;
use crate::{
    checker::state::Health,
    config::{Endpoint, Target},
};
use anyhow::{Context, Result};

fn application(path: PathBuf) -> Result<App> {
    let config = Config {
        targets: vec![Target {
            name: "loopback".into(),
            endpoint: Endpoint::Icmp {
                host: "127.0.0.1".into(),
            },
        }],
        ..Config::default()
    };
    config.save(&path)?;
    Ok(App {
        monitor: Some(Monitor::new(&config)?),
        config,
        path: Some(path),
        message: None,
        writable: true,
        dialog: None,
        beeper: Beeper::default(),
    })
}

#[test]
fn saved_threshold_applies_without_losing_an_existing_outage() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("ping.yml");
    let mut app = application(path.clone())?;
    let monitor = app.monitor.as_mut().context("monitor unavailable")?;
    let state = monitor.states.first_mut().context("state unavailable")?;
    state.record(
        Err("offline".into()),
        Duration::ZERO,
        std::time::Instant::now(),
    );
    let candidate = Config {
        failure_threshold: 1,
        ..app.config.clone()
    };

    assert!(app.save(candidate));

    assert_eq!(Config::load(&path)?.failure_threshold, 1);
    let state = app
        .monitor
        .as_mut()
        .context("monitor unavailable")?
        .states
        .first_mut()
        .context("state unavailable")?;
    assert_eq!(state.health, Health::Down);
    assert!(state.alarm_due(app.config.failure_threshold, true));
    Ok(())
}

#[test]
fn invalid_reload_preserves_file_and_last_good_settings() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("ping.yml");
    let mut app = application(path.clone())?;
    let original = app.config.clone();
    let invalid = "interval_seconds: [broken";
    std::fs::write(&path, invalid)?;

    app.reload();

    assert_eq!(app.config, original);
    assert!(!app.writable);
    assert!(!app.save(Config::default()));
    assert_eq!(std::fs::read_to_string(path)?, invalid);
    Ok(())
}

#[test]
fn failed_save_does_not_apply_unsaved_settings() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("ping.yml");
    let mut app = application(path)?;
    let original = app.config.clone();
    app.path = Some(directory.path().join("missing").join("ping.yml"));

    assert!(!app.save(Config {
        interval_seconds: 30,
        ..original.clone()
    }));

    assert_eq!(app.config, original);
    Ok(())
}
