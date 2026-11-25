use super::ui_test_harness::Harness;
use crate::{
    app::ui::theme,
    config::{Appearance, Endpoint, Target},
};
use anyhow::{Context, Result};
use eframe::egui::{FullOutput, Rect, epaint::Shape};
use std::time::{Duration, Instant};

fn row_visual(output: &FullOutput, name: &str) -> Result<(Rect, bool)> {
    let bounds = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            Shape::Text(text) if text.galley.job.text == name => {
                Some(Rect::from_min_size(text.pos, text.galley.size()))
            }
            _ => None,
        })
        .with_context(|| format!("target row {name}"))?;
    let highlighted = output.shapes.iter().any(|shape| match &shape.shape {
        Shape::Rect(rect) => {
            rect.rect.contains_rect(bounds)
                && rect.fill.r() > rect.fill.g()
                && rect.fill.r() > rect.fill.b()
        }
        _ => false,
    });
    Ok((bounds, highlighted))
}

#[test]
fn alarmed_row_stays_red_until_recovery_without_highlighting_other_targets() -> Result<()> {
    let mut layouts = Vec::new();
    for appearance in [Appearance::Dark, Appearance::Light] {
        let mut h = Harness::new()?;
        let mut config = h.app.config.clone();
        config.theme = Some(appearance);
        config.failure_threshold = 2;
        config.beep_enabled = true;
        config.targets = ["healthy", "failing"]
            .into_iter()
            .map(|name| Target {
                name: name.into(),
                endpoint: Endpoint::Tcp {
                    host: "127.0.0.1".into(),
                    port: 443,
                },
            })
            .collect();
        assert!(h.app.save(config));
        theme::apply(&h.ctx, appearance);
        let now = Instant::now();
        h.app.monitor.as_mut().context("monitor")?.states[0].record(
            Ok("connected".into()),
            Duration::ZERO,
            now,
        );
        let mut layout = Vec::new();

        for (result, beep_due, highlighted) in [
            (Err("offline".into()), false, false),
            (Err("offline".into()), true, true),
            (Err("offline".into()), false, true),
            (Ok("connected".into()), false, false),
        ] {
            let state = &mut h.app.monitor.as_mut().context("monitor")?.states[1];
            state.record(result, Duration::ZERO, now);
            assert_eq!(state.alarm_due(2, true), beep_due);
            h.frame(Vec::new());
            h.frame(Vec::new());

            let (bounds, red) = row_visual(&h.output, "failing")?;
            assert_eq!(red, highlighted, "{appearance:?}: alarmed row appearance");
            layout.push(bounds);
            let (bounds, red) = row_visual(&h.output, "healthy")?;
            assert!(!red, "an unaffected target must not be highlighted");
            layout.push(bounds);
            let alert_labels = h.output.shapes.iter().filter(|shape| {
                matches!(&shape.shape, Shape::Text(text) if text.galley.job.text == "ALERT")
            }).count();
            assert_eq!(alert_labels, usize::from(highlighted));
        }
        layouts.push(layout);
    }
    assert_eq!(
        layouts[0], layouts[1],
        "alarm colors must not change theme geometry"
    );
    Ok(())
}
