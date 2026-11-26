use std::{sync::mpsc, thread, time::Duration};

use anyhow::{Context, Result, anyhow};
use rodio::{
    DeviceSinkBuilder, MixerDeviceSink, Player, Source, cpal::BufferSize, source::SineWave,
};

use crate::config::Config;

/// The mixer draining is not the device finishing: samples can still sit in the
/// OS buffer, so the stream stays open this much longer to avoid cutting them off.
const CLOSE_DELAY: Duration = Duration::from_millis(100);

/// Plays one beep on the current default output device and returns once the
/// device is open. The stream lives on a short thread that ends with the beep,
/// so nothing stays resident and output changes (e.g. headphones) apply each time.
pub fn play(config: &Config) -> Result<()> {
    let source = tone(config);
    let (opened, opened_rx) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("beep".into())
        .spawn(move || {
            let stream = open_output();
            let ready = stream
                .as_ref()
                .map(|_| ())
                .map_err(|error| anyhow!("{error:#}"));
            if opened.send(ready).is_ok()
                && let Ok(stream) = stream
            {
                let player = Player::connect_new(stream.mixer());
                player.append(source);
                player.sleep_until_end();
                thread::sleep(CLOSE_DELAY);
            }
        })
        .context("Failed to start the audio thread")?;
    opened_rx
        .recv()
        .context("The audio thread stopped before opening the output device")?
}

fn open_output() -> Result<MixerDeviceSink> {
    // rodio's fixed 50 ms buffer (2048 frames at 48 kHz) exceeds the range of
    // some Bluetooth outputs, so let the OS choose a size the device supports.
    let mut stream = DeviceSinkBuilder::from_default_device()
        .and_then(|builder| {
            builder
                .with_buffer_size(BufferSize::Default)
                .open_sink_or_fallback()
        })
        .context("Failed to open the audio output device")?;
    stream.log_on_drop(false);
    Ok(stream)
}

fn tone(config: &Config) -> impl Source<Item = f32> + Send + 'static {
    SineWave::new(880.0)
        .take_duration(Duration::from_millis(config.beep_duration_ms))
        .amplify(f32::from(config.beep_volume_percent) / 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tone_uses_configured_duration_and_volume() {
        for (duration, volume) in [(100, 0), (300, 20), (1200, 75), (10000, 100)] {
            let config = Config {
                beep_duration_ms: duration,
                beep_volume_percent: volume,
                ..Config::default()
            };
            let source = tone(&config);
            let one_millisecond =
                u64::from(source.sample_rate().get()) * u64::from(source.channels().get()) / 1000;
            let expected_samples = u64::from(source.sample_rate().get())
                * u64::from(source.channels().get())
                * duration
                / 1000;
            let (count, peak) = source.fold((0_u64, 0.0_f32), |(count, peak), sample| {
                (count + 1, peak.max(sample.abs()))
            });
            assert!(count.abs_diff(expected_samples) <= one_millisecond);
            let expected = f32::from(volume) / 100.0;
            assert!((peak - expected).abs() < 0.001, "{volume}%: {peak}");
        }
    }
}
