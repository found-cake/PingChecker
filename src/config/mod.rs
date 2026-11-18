use std::{
    io::Write,
    net::IpAddr,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<Appearance>,
    pub interval_seconds: u64,
    pub timeout_seconds: u64,
    pub failure_threshold: u32,
    pub beep_enabled: bool,
    #[serde(default = "default_beep_duration_ms")]
    pub beep_duration_ms: u64,
    #[serde(default = "default_beep_volume_percent")]
    pub beep_volume_percent: u8,
    pub targets: Vec<Target>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    Light,
    Dark,
}

fn default_beep_duration_ms() -> u64 {
    300
}

fn default_beep_volume_percent() -> u8 {
    20
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Target {
    pub name: String,
    #[serde(flatten)]
    pub endpoint: Endpoint,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "protocol", rename_all = "lowercase")]
pub enum Endpoint {
    Icmp { host: String },
    Tcp { host: String, port: u16 },
    Http { url: String },
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: None,
            interval_seconds: 5,
            timeout_seconds: 3,
            failure_threshold: 3,
            beep_enabled: true,
            beep_duration_ms: default_beep_duration_ms(),
            beep_volume_percent: default_beep_volume_percent(),
            targets: Vec::new(),
        }
    }
}

impl Endpoint {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Icmp { .. } => "ICMP",
            Self::Tcp { .. } => "TCP",
            Self::Http { .. } => "HTTP",
        }
    }

    pub fn address(&self) -> String {
        match self {
            Self::Icmp { host } => host.clone(),
            Self::Tcp { host, port } if host.parse::<std::net::Ipv6Addr>().is_ok() => {
                format!("[{host}]:{port}")
            }
            Self::Tcp { host, port } => format!("{host}:{port}"),
            Self::Http { url } => url.clone(),
        }
    }
}

fn valid_host(host: &str) -> bool {
    if host.parse::<IpAddr>().is_ok() {
        return true;
    }
    let dns = host.strip_suffix('.').unwrap_or(host);
    !dns.is_empty()
        && dns.len() <= 253
        && dns.split('.').all(|part| {
            !part.is_empty()
                && part.len() <= 63
                && part
                    .as_bytes()
                    .first()
                    .is_some_and(u8::is_ascii_alphanumeric)
                && part
                    .as_bytes()
                    .last()
                    .is_some_and(u8::is_ascii_alphanumeric)
                && part.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
        })
}

impl Config {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (1..=3600).contains(&self.interval_seconds),
            "interval must be 1 to 3600 seconds"
        );
        ensure!(
            (1..=60).contains(&self.timeout_seconds),
            "timeout must be 1 to 60 seconds"
        );
        ensure!(
            (1..=1000).contains(&self.failure_threshold),
            "failure threshold must be 1 to 1000"
        );
        ensure!(
            (100..=10000).contains(&self.beep_duration_ms),
            "beep duration must be 100 to 10000 milliseconds"
        );
        ensure!(
            self.beep_volume_percent <= 100,
            "beep volume must be 0 to 100 percent"
        );
        ensure!(self.targets.len() <= 256, "at most 256 targets are allowed");
        for target in &self.targets {
            ensure!(
                !target.name.trim().is_empty(),
                "target name cannot be blank"
            );
            match &target.endpoint {
                Endpoint::Icmp { host } => ensure!(valid_host(host), "invalid ICMP host: {host}"),
                Endpoint::Tcp { host, port } => {
                    ensure!(valid_host(host), "invalid TCP host: {host}");
                    ensure!(*port != 0, "TCP port must be nonzero");
                }
                Endpoint::Http { url } => {
                    let parsed = reqwest::Url::parse(url).context("invalid HTTP URL")?;
                    ensure!(
                        matches!(parsed.scheme(), "http" | "https") && parsed.host().is_some(),
                        "URL must be HTTP or HTTPS with a host"
                    );
                }
            }
        }
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self> {
        let contents =
            std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        let config: Self = serde_yaml_ng::from_slice(&contents).context("parsing configuration")?;
        config.validate()?;
        Ok(config)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut file =
            tempfile::NamedTempFile::new_in(parent).context("creating temporary configuration")?;
        serde_yaml_ng::to_writer(&mut file, self).context("serializing configuration")?;
        file.flush().context("flushing configuration")?;
        file.as_file().sync_all().context("syncing configuration")?;
        file.persist(path)
            .map_err(|error| error.error)
            .context("persisting configuration")?;
        Ok(())
    }

    pub fn path() -> Result<PathBuf> {
        let executable = std::env::current_exe().context("locating executable")?;
        Ok(executable
            .parent()
            .context("executable has no parent directory")?
            .join("ping.yml"))
    }
}

#[cfg(test)]
mod tests;
