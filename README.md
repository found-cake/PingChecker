# PingChecker

A native desktop app that monitors ICMP, TCP and HTTP targets and beeps when one goes down.

![PingChecker main screen](docs/screenshot.png)

## Features

- ICMP ping, TCP connect and HTTP GET (2xx) checks
- Latency, consecutive failures and downtime per target
- Beep after N consecutive failures; the alerted row stays red until it recovers
- Light / Dark theme, countdown to the next check

## Run

```sh
cargo run --release
```

Settings and targets are saved to `ping.yml` beside the executable.
See `ping.example.yml` for the format.

## Build

macOS, Windows and Linux. On Ubuntu:

```sh
sudo apt install build-essential pkg-config libasound2-dev libudev-dev \
  libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev
```

ICMP uses the system `ping` command.
