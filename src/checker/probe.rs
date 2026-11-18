use std::{process::Stdio, time::Duration};

use anyhow::{Context, Result, bail};
use tokio::{net::TcpStream, process::Command, time};

use crate::config::Endpoint;

pub async fn check(
    client: &reqwest::Client,
    endpoint: &Endpoint,
    timeout: Duration,
) -> Result<String> {
    time::timeout(timeout, async {
        match endpoint {
            Endpoint::Icmp { host } => {
                #[cfg(target_os = "macos")]
                let executable = if host.parse::<std::net::Ipv6Addr>().is_ok() {
                    "/sbin/ping6"
                } else {
                    "/sbin/ping"
                };
                #[cfg(target_os = "linux")]
                let executable = "/bin/ping";
                #[cfg(windows)]
                let executable = std::path::PathBuf::from(
                    std::env::var_os("SystemRoot").context("SystemRoot is not set")?,
                )
                .join("System32")
                .join("PING.EXE");
                #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
                let executable = "ping";
                let mut command = Command::new(executable);
                command.kill_on_drop(true).stdin(Stdio::null());
                #[cfg(windows)]
                command.creation_flags(0x0800_0000).args(["-n", "1"]);
                #[cfg(unix)]
                command.args(["-n", "-c", "1"]);
                #[cfg(target_os = "linux")]
                if host.parse::<std::net::Ipv6Addr>().is_ok() {
                    command.arg("-6");
                }
                let output = command.arg(host).output().await.context("running ping")?;
                if output.status.success() {
                    Ok("Ping reply received".to_owned())
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let detail = if stderr.trim().is_empty() {
                        stdout.trim()
                    } else {
                        stderr.trim()
                    };
                    bail!("ping failed ({}): {}", output.status, detail)
                }
            }
            Endpoint::Tcp { host, port } => {
                let stream = TcpStream::connect((host.as_str(), *port))
                    .await
                    .context("TCP connection failed")?;
                drop(stream);
                Ok("TCP connected".to_owned())
            }
            Endpoint::Http { url } => {
                let response = client
                    .get(url)
                    .send()
                    .await
                    .context("HTTP request failed")?;
                let status = response.status();
                if !status.is_success() {
                    bail!("HTTP {status}");
                }
                Ok(format!("HTTP {status}"))
            }
        }
    })
    .await
    .context("probe timed out")?
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        task::JoinSet,
    };

    #[tokio::test]
    async fn http_probe_times_out_when_server_withholds_headers() -> Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = Endpoint::Http {
            url: format!("http://{}/", listener.local_addr()?),
        };
        let client = reqwest::Client::builder().no_proxy().build()?;
        let mut jobs = JoinSet::new();
        jobs.spawn(async move { check(&client, &endpoint, Duration::from_secs(60)).await });
        let (mut stream, _) = time::timeout(Duration::from_secs(5), listener.accept()).await??;
        let mut request = Vec::new();
        time::timeout(Duration::from_secs(5), async {
            while !request.ends_with(b"\r\n\r\n") {
                let mut byte = [0; 1];
                anyhow::ensure!(stream.read(&mut byte).await? == 1, "incomplete request");
                request.push(byte[0]);
            }
            Result::<()>::Ok(())
        })
        .await??;

        // Advance time only after the server has received the real request.
        time::pause();
        time::advance(Duration::from_secs(61)).await;
        let result = time::timeout(Duration::from_secs(1), jobs.join_next())
            .await?
            .context("probe task missing")??;

        let error = result.err().context("probe unexpectedly succeeded")?;
        assert!(error.to_string().contains("timed out"));
        Ok(())
    }

    #[tokio::test]
    async fn tcp_connects_without_sending_payload() -> Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        let server = async {
            let (mut stream, _) = listener.accept().await?;
            let mut buf = [0; 1];
            let count = stream.read(&mut buf).await?;
            anyhow::ensure!(count == 0, "TCP probe sent data");
            Result::<()>::Ok(())
        };
        let endpoint = Endpoint::Tcp {
            host: "127.0.0.1".into(),
            port,
        };
        let client = reqwest::Client::new();
        let (probe, server) = time::timeout(Duration::from_secs(5), async {
            tokio::join!(check(&client, &endpoint, Duration::from_secs(3)), server)
        })
        .await?;
        probe?;
        server?;
        Ok(())
    }

    #[tokio::test]
    async fn http_get_accepts_only_success_status() -> Result<()> {
        for (status, expected) in [
            (200, true),
            (204, true),
            (302, false),
            (404, false),
            (500, false),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").await?;
            let url = format!("http://{}/test", listener.local_addr()?);
            let server = async {
                let (mut stream, _) = listener.accept().await?;
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    let mut byte = [0; 1];
                    anyhow::ensure!(stream.read(&mut byte).await? == 1, "incomplete request");
                    request.push(byte[0]);
                }
                anyhow::ensure!(
                    request.starts_with(b"GET /test HTTP/1.1\r\n"),
                    "not a GET request"
                );
                stream.write_all(format!("HTTP/1.1 {status} Test\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").as_bytes()).await?;
                Result::<()>::Ok(())
            };
            let client = reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::limited(5))
                .build()?;
            let endpoint = Endpoint::Http { url };
            let (probe, server) = time::timeout(Duration::from_secs(5), async {
                tokio::join!(check(&client, &endpoint, Duration::from_secs(3)), server)
            })
            .await?;
            server?;
            assert_eq!(probe.is_ok(), expected, "HTTP {status}");
            if !expected {
                assert!(
                    probe
                        .err()
                        .context("expected error")?
                        .to_string()
                        .contains(&status.to_string())
                );
            }
        }
        Ok(())
    }
}
