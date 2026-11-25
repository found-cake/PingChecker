use super::*;
use crate::config::{Endpoint, Target};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[test]
fn countdown_uses_the_round_deadline_even_when_all_targets_are_busy() -> Result<()> {
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
    monitor.next_refresh = now + Duration::from_secs(3);
    monitor.busy.fill(true);

    for (elapsed, expected_remaining, expected_progress) in [(0, 3, 0.4), (1, 2, 0.6), (2, 1, 0.8)]
    {
        let Refresh::Waiting {
            remaining,
            progress,
        } = monitor.refresh(now + Duration::from_secs(elapsed), Duration::from_secs(5))
        else {
            anyhow::bail!("expected a scheduled refresh");
        };
        assert_eq!(remaining, Duration::from_secs(expected_remaining));
        assert!((progress - expected_progress).abs() < 0.00001);
    }
    assert_eq!(
        monitor.refresh(now + Duration::from_secs(3), Duration::from_secs(5)),
        Refresh::Due
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

#[test]
fn a_round_starts_both_requests_without_waiting_and_does_not_overlap_them() -> Result<()> {
    let mut monitor = Monitor::new(&Config::default())?;
    let (first, second) = monitor.runtime.block_on(async {
        tokio::try_join!(
            tokio::net::TcpListener::bind("127.0.0.1:0"),
            tokio::net::TcpListener::bind("127.0.0.1:0"),
        )
    })?;
    let config = Config {
        targets: [&first, &second]
            .into_iter()
            .map(|listener| {
                let address = listener.local_addr()?;
                Ok(Target {
                    name: address.to_string(),
                    endpoint: Endpoint::Http {
                        url: format!("http://{address}/"),
                    },
                })
            })
            .collect::<Result<_>>()?,
        timeout_seconds: 60,
        ..Config::default()
    };
    monitor.reset(&config, false);
    let ctx = egui::Context::default();
    monitor.tick(&config, &ctx)?;
    let (_first_connection, _second_connection) = monitor.runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(2), async {
            tokio::try_join!(first.accept(), second.accept())
        })
        .await
    })??;

    // Neither server has replied. Simulate the next deadline without a wall-clock wait.
    let deadline = Instant::now();
    monitor.next_refresh = deadline;
    monitor.tick(&config, &ctx)?;
    assert_eq!(monitor.jobs.len(), 2, "one in-flight request per target");
    let interval = Duration::from_secs(config.interval_seconds);
    assert!(matches!(
        monitor.refresh(deadline, interval),
        Refresh::Waiting { remaining, .. } if remaining >= interval
    ));
    Ok(())
}

#[test]
fn completed_probe_wakes_a_waiting_ui_and_delivers_its_result() -> Result<()> {
    let mut config = Config::default();
    let mut monitor = Monitor::new(&config)?;
    let listener = monitor
        .runtime
        .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))?;
    config.targets.push(Target {
        name: "held response".into(),
        endpoint: Endpoint::Http {
            url: format!("http://{}/", listener.local_addr()?),
        },
    });
    monitor.reset(&config, false);

    let ctx = egui::Context::default();
    for _ in 0..3 {
        drop(ctx.run(egui::RawInput::default(), |_| {}));
    }
    let (repaint, wake) = std::sync::mpsc::sync_channel(1);
    ctx.set_request_repaint_callback(move |info| {
        if info.delay.is_zero() {
            match repaint.try_send(()) {
                Ok(())
                | Err(std::sync::mpsc::TrySendError::Full(()))
                | Err(std::sync::mpsc::TrySendError::Disconnected(())) => {}
            }
        }
    });
    monitor.tick(&config, &ctx)?;

    monitor.runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(2), async {
            let (socket, _) = listener.accept().await?;
            let mut socket = BufReader::new(socket);
            let mut line = Vec::new();
            loop {
                line.clear();
                anyhow::ensure!(
                    socket.read_until(b'\n', &mut line).await? != 0,
                    "incomplete HTTP request"
                );
                if line == b"\r\n" {
                    break;
                }
            }
            socket
                .write_all(b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n")
                .await?;
            Result::<()>::Ok(())
        })
        .await?
    })?;
    wake.recv_timeout(Duration::from_secs(2))
        .context("probe completion did not wake the UI")?;
    monitor.tick(&config, &ctx)?;
    let state = monitor.states.first().context("missing probe state")?;
    assert_eq!(state.health, crate::checker::state::Health::Up);
    assert_eq!(state.detail, "HTTP 204 No Content");
    Ok(())
}
