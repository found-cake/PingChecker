use std::time::Duration;

use anyhow::{Context, Result};
use rodio::{OutputStream, OutputStreamBuilder, Sink, Source, source::SineWave};

use crate::config::Config;

/// The stream and sink stay alive for the lifetime of the application.
#[derive(Default)]
pub struct Beeper {
    output: Option<(OutputStream, Sink)>,
}

impl Beeper {
    pub fn play(&mut self, config: &Config) -> Result<()> {
        if self.output.is_none() {
            let mut stream = OutputStreamBuilder::open_default_stream()
                .context("Failed to open the audio output device")?;
            stream.log_on_drop(false);
            let sink = Sink::connect_new(stream.mixer());
            self.output = Some((stream, sink));
        }
        if let Some((stream, sink)) = &mut self.output {
            // Dropping the old sink stops it without waiting for its queue to drain.
            *sink = Sink::connect_new(stream.mixer());
            sink.append(tone(config));
        }
        Ok(())
    }
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
                u64::from(source.sample_rate()) * u64::from(source.channels()) / 1000;
            let expected_samples =
                u64::from(source.sample_rate()) * u64::from(source.channels()) * duration / 1000;
            let (count, peak) = source.fold((0_u64, 0.0_f32), |(count, peak), sample| {
                (count + 1, peak.max(sample.abs()))
            });
            assert!(count.abs_diff(expected_samples) <= one_millisecond);
            let expected = f32::from(volume) / 100.0;
            assert!((peak - expected).abs() < 0.001, "{volume}%: {peak}");
        }
    }
}
