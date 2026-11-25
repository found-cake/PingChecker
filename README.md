# PingChecker

A native Rust desktop application for ICMP, TCP and HTTP availability monitoring.

## Run

```sh
cargo run --release
```

Or run `target/release/pingchecker` after `cargo build --release`.
On Windows the executable is `pingchecker.exe`.

Settings and targets live in **`ping.yml` beside the executable**, not in the
working directory. The first launch creates an empty default configuration.
With `cargo run --release`, the file is `target/release/ping.yml`.
The executable's directory must be writable.

## Use

- **Add target** opens the protocol, name, host/URL and TCP-port form.
  **Add** saves the target and closes the modal.
- **Settings** reads the saved YAML each time it opens. Change the interval,
  timeout, failure threshold, beep, duration, volume or theme, then select
  **Save settings**. Opening settings alone does not change running checks.
- **Check now** starts another round without waiting for the next deadline.
- **Test beep** previews the unsaved duration and volume. A new preview replaces
  the current beep. Automatic alerts use the active saved settings.

The main screen shows availability, latency, consecutive failures, continuous
downtime and the latest result. A fixed bottom-right circle shows the seconds
until the next scheduled refresh (for example `4s`), with elapsed interval progress
in its arc. Hover for the full refresh status; notifications appear above it.
This uses the monitor's actual deadlines, not a second independent timer.
When targets are still checking, their count is shown; no percentage is invented
for work with an unknown completion time. Empty, due and unavailable states
have explicit labels.

All application-owned interface text is English. User-supplied names, addresses
and external diagnostics are retained as provided. Dark and light modes have the
same typography, spacing and layout; only colors change.

The system theme is used only as the initial default when no theme is saved.
Settings offers **Light** and **Dark**, not a continuing system-follow mode.
Selection previews the theme; **Save settings** persists it across launches.
**Cancel**, Escape and backdrop clicks discard unsaved edits and restore the
active theme. Monitoring and alarms continue while a modal is open.
Failed reads or saves show an error without overwriting the original file.
Fix a malformed file and reopen Settings or use **Reload YAML** to recover.

Beep duration is **0.1-10 seconds**, and app volume is **0-100%**.
Defaults are 0.3 seconds and 20%; output also depends on system volume.
Zero percent mutes the sound without changing outage tracking.
The alert sounds once per outage at the configured consecutive failure count.
After an automatic alert, that target's row stays red with an **ALERT** status
through subsequent failures. A successful check clears the highlight.
Recovery resets the failure count and rearms the alert.
Saving changes or managing targets preserves surviving targets' outage state.
Restarting or explicitly selecting **Reload YAML** resets monitoring history.

There is no target editor: remove and add a target again, or edit the YAML and
reload it. Removing or adding targets preserves the other targets' failure
counts, downtime and alert state, including after row-index changes.

## Protocols

| Protocol | Input | Healthy result |
| --- | --- | --- |
| ICMP | Hostname or IP | Successful system `ping` echo reply |
| TCP | Hostname/IP and port | TCP connection established; no application payload |
| HTTP | HTTP or HTTPS URL | Final response status 200-299 |

HTTP uses GET, follows at most five redirects, keeps certificate verification,
and does not download the entire response body. TCP latency includes DNS and
connection establishment; ICMP includes process startup; HTTP measures through
response headers.

The interval is measured between check starts. A target never has overlapping
checks: if its check exceeds the interval, it becomes due again after completion.
Downtime begins when the first failure result arrives.

ICMP requires the operating system's `ping` command. Linux needs `iputils-ping`
and the distribution's required ICMP permissions. A firewall may block ICMP
while TCP or HTTP remains healthy.

## YAML

See `ping.example.yml`. An optional `theme` accepts only `light` or `dark`.
Old configurations without it or the newer beep fields remain compatible.

```yaml
theme: dark
interval_seconds: 5
timeout_seconds: 3
failure_threshold: 3
beep_enabled: true
beep_duration_ms: 300
beep_volume_percent: 20
targets:
  - name: Gateway
    protocol: icmp
    host: 192.168.0.1
  - name: Service
    protocol: tcp
    host: 127.0.0.1
    port: 8080
  - name: Health page
    protocol: http
    url: https://example.com/health
```

Intervals accept 1-3600 seconds, timeouts 1-60 seconds, and failure thresholds
1-1000. Up to 256 targets are supported. Beep duration is stored in milliseconds.
Only settings and targets are persisted; live monitoring history stays in memory.

## Build and verification

Build separately on macOS, Windows or Linux. Linux requires the distribution's
X11/Wayland and ALSA development packages. On Ubuntu-family distributions:

```sh
sudo apt install build-essential pkg-config libasound2-dev libudev-dev \
  libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev
```

Installed CJK fonts are optional fallbacks for user-supplied names.
Checks are not guaranteed during system sleep, and stop when the app exits.

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

See `QA.md` for verification evidence and manual/platform limitations.

## Finding your way around the code

Start with `AGENTS.md`, then `App::ui` in `src/app/mod.rs`.

- `app`: application state, settings application and audio; `app::ui` contains
  screens, dialogs, forms, countdown and theme.
- `checker`: monitoring, network probes and target state.
- `config`: YAML schema, validation and persistence.

Each folder's `mod.rs` declares its public submodules. UI regression tests and
their fixture live in `src/app/tests/`. The guide maps the frame/action flow
and verification commands.
