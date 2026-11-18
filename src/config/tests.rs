use super::*;

#[test]
fn round_trip_and_invalid_data_preservation() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("ping.yml");
    let config = Config {
        theme: Some(Appearance::Light),
        beep_duration_ms: 1200,
        beep_volume_percent: 75,
        targets: vec![
            Target {
                name: "local".into(),
                endpoint: Endpoint::Icmp { host: "::1".into() },
            },
            Target {
                name: "tcp".into(),
                endpoint: Endpoint::Tcp {
                    host: "example.org".into(),
                    port: 443,
                },
            },
            Target {
                name: "web".into(),
                endpoint: Endpoint::Http {
                    url: "https://example.org/".into(),
                },
            },
        ],
        ..Config::default()
    };
    config.save(&path)?;
    assert_eq!(Config::load(&path)?, config);
    let invalid = serde_yaml_ng::to_string(&Config {
        interval_seconds: 0,
        ..config.clone()
    })?;
    std::fs::write(&path, &invalid)?;
    assert!(Config::load(&path).is_err());
    assert_eq!(std::fs::read(&path)?, invalid.as_bytes());
    let invalid_config = Config {
        interval_seconds: 0,
        ..config
    };
    assert!(invalid_config.save(&path).is_err());
    assert_eq!(std::fs::read(&path)?, invalid.as_bytes());
    Ok(())
}

#[test]
fn old_format_defaults_only_new_beep_fields() -> Result<()> {
    let yaml = "interval_seconds: 5\ntimeout_seconds: 3\nfailure_threshold: 3\nbeep_enabled: true\ntargets: []\n";
    let config: Config = serde_yaml_ng::from_str(yaml)?;
    assert_eq!(config, Config::default());
    assert!(serde_yaml_ng::from_str::<Config>(&format!("{yaml}theme: system\n")).is_err());
    let with_duration: Config = serde_yaml_ng::from_str(&format!("{yaml}beep_duration_ms: 500\n"))?;
    assert_eq!(with_duration.beep_duration_ms, 500);
    assert_eq!(with_duration.beep_volume_percent, 20);
    let with_volume: Config = serde_yaml_ng::from_str(&format!("{yaml}beep_volume_percent: 70\n"))?;
    assert_eq!(with_volume.beep_duration_ms, 300);
    assert_eq!(with_volume.beep_volume_percent, 70);
    assert!(serde_yaml_ng::from_str::<Config>(&yaml.replace("beep_enabled: true\n", "")).is_err());
    Ok(())
}

#[test]
fn validates_boundaries_and_hosts() {
    let mut config = Config::default();
    for host in ["-n", "a/b", "a b", "a:80", "", "bad..host", ".bad"] {
        config.targets = vec![Target {
            name: "host".into(),
            endpoint: Endpoint::Icmp { host: host.into() },
        }];
        assert!(config.validate().is_err(), "{host}");
    }
    for host in ["example.org.", "127.0.0.1", "::1"] {
        config.targets = vec![Target {
            name: "host".into(),
            endpoint: Endpoint::Icmp { host: host.into() },
        }];
        assert!(config.validate().is_ok(), "{host}");
    }
    config.targets = vec![Target {
        name: " ".into(),
        endpoint: Endpoint::Tcp {
            host: "localhost".into(),
            port: 0,
        },
    }];
    assert!(config.validate().is_err());
    config.targets[0].name = "x".into();
    assert!(config.validate().is_err());
    config.targets[0].endpoint = Endpoint::Http {
        url: "ftp://example.org".into(),
    };
    assert!(config.validate().is_err());
    config.targets.clear();
    for (interval, timeout, valid) in [
        (0, 1, false),
        (1, 1, true),
        (3600, 60, true),
        (3601, 1, false),
        (1, 0, false),
        (1, 61, false),
    ] {
        config.interval_seconds = interval;
        config.timeout_seconds = timeout;
        assert_eq!(config.validate().is_ok(), valid);
    }
    config.interval_seconds = 1;
    config.timeout_seconds = 1;
    for (threshold, valid) in [(0, false), (1, true), (1000, true), (1001, false)] {
        config.failure_threshold = threshold;
        assert_eq!(config.validate().is_ok(), valid);
    }
    config.failure_threshold = 3;
    for (duration, volume, valid) in [
        (99, 20, false),
        (100, 0, true),
        (10000, 100, true),
        (10001, 20, false),
        (300, 101, false),
    ] {
        config.beep_duration_ms = duration;
        config.beep_volume_percent = volume;
        assert_eq!(config.validate().is_ok(), valid);
    }
    config.beep_duration_ms = 300;
    config.beep_volume_percent = 20;
    config.targets = vec![
        Target {
            name: "x".into(),
            endpoint: Endpoint::Icmp {
                host: "localhost".into()
            },
        };
        257
    ];
    assert!(config.validate().is_err());
}
