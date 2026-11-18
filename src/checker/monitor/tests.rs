use super::*;
use crate::config::{Endpoint, Target};

#[test]
fn countdown_uses_actual_idle_deadlines_and_excludes_busy_targets() -> Result<()> {
    let config = Config {
        targets: vec![
            Target {
                name: "one".into(),
                endpoint: Endpoint::Icmp {
                    host: "127.0.0.1".into()
                }
            };
            3
        ],
        ..Config::default()
    };
    let mut monitor = Monitor::new(&config)?;
    let now = Instant::now();
    monitor.due = vec![
        now + Duration::from_secs(1),
        now + Duration::from_secs(4),
        now + Duration::from_secs(3),
    ];
    monitor.busy = vec![true, false, false];

    for (elapsed, expected_remaining, expected_progress) in [(0, 3, 0.4), (1, 2, 0.6), (2, 1, 0.8)]
    {
        let Refresh::Waiting {
            remaining,
            progress,
            checking,
        } = monitor.refresh(now + Duration::from_secs(elapsed), Duration::from_secs(5))
        else {
            anyhow::bail!("expected a scheduled refresh");
        };
        assert_eq!(remaining, Duration::from_secs(expected_remaining));
        assert!((progress - expected_progress).abs() < 0.00001);
        assert_eq!(checking, 1);
    }
    assert_eq!(
        monitor.refresh(now + Duration::from_secs(3), Duration::from_secs(5)),
        Refresh::Due
    );
    monitor.busy.fill(true);
    assert_eq!(
        monitor.refresh(now, Duration::from_secs(5)),
        Refresh::Checking { count: 3 }
    );
    Ok(())
}

#[test]
fn no_targets_have_no_refresh_deadline() -> Result<()> {
    let monitor = Monitor::new(&Config::default())?;
    assert_eq!(
        monitor.refresh(Instant::now(), Duration::from_secs(5)),
        Refresh::Inactive
    );
    Ok(())
}
