use eframe::egui::{self, Align2, FontId, Sense, Stroke, WidgetInfo, WidgetType};

use crate::checker::monitor::Refresh;

struct Presentation {
    progress: Option<f32>,
    label: String,
    center_text: String,
    checking: usize,
}

impl Presentation {
    fn status(center_text: &str, label: impl Into<String>) -> Self {
        Self {
            progress: None,
            label: label.into(),
            center_text: center_text.into(),
            checking: 0,
        }
    }
}

pub fn draw(ui: &mut egui::Ui, refresh: Option<Refresh>) {
    let presentation = match refresh {
        None => Presentation::status("--", "Monitoring unavailable"),
        Some(Refresh::Inactive) => Presentation::status("--", "No targets"),
        Some(Refresh::Due) => Presentation::status("0s", "Refresh due"),
        Some(Refresh::Checking { count }) => Presentation::status(
            "...",
            format!(
                "Checking {count} {}...",
                if count == 1 { "target" } else { "targets" }
            ),
        ),
        Some(Refresh::Waiting {
            remaining,
            progress,
            checking,
        }) => {
            let seconds = remaining
                .as_secs()
                .saturating_add(u64::from(remaining.subsec_nanos() != 0));
            let mut label = format!("Next refresh in {seconds} s");
            if checking != 0 {
                label.push_str(&format!(" · {checking} checking"));
            }
            Presentation {
                progress: Some(progress),
                label,
                center_text: format!("{seconds}s"),
                checking,
            }
        }
    };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(44.0, 44.0), Sense::hover());
    response.widget_info(|| {
        let mut info = WidgetInfo::labeled(
            WidgetType::ProgressIndicator,
            ui.is_enabled(),
            &presentation.label,
        );
        info.value = presentation.progress.map(|value| f64::from(value) * 100.0);
        info
    });
    let center = rect.center();
    let radius = 19.0;
    ui.painter().circle_stroke(
        center,
        radius,
        Stroke::new(3.0_f32, ui.visuals().widgets.noninteractive.bg_stroke.color),
    );
    if let Some(progress) = presentation.progress
        && progress > 0.0
    {
        let points = (0..=64_u16)
            .map(|step| {
                let angle = -std::f32::consts::FRAC_PI_2
                    + std::f32::consts::TAU * progress * f32::from(step) / 64.0;
                center + egui::Vec2::angled(angle) * radius
            })
            .collect();
        ui.painter().add(egui::Shape::line(
            points,
            Stroke::new(3.0_f32, ui.visuals().selection.stroke.color),
        ));
    }
    ui.painter().text(
        center,
        Align2::CENTER_CENTER,
        presentation.center_text,
        FontId::proportional(12.0),
        ui.visuals().text_color(),
    );
    response.on_hover_text(&presentation.label);
    if presentation.progress.is_none() {
        ui.label(presentation.label);
    } else if presentation.checking != 0 {
        ui.label(format!("{} checking", presentation.checking));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{Context, Result};
    use std::time::Duration;

    #[test]
    fn scheduled_progress_is_delivered_to_the_accessibility_tree() -> Result<()> {
        for (progress, milliseconds, percent, dx, dy, seconds) in [
            (0.25, 3750, 25.0, 19.0, 19.0, 4),
            (0.5, 2500, 50.0, 0.0, 38.0, 3),
            (0.75, 1250, 75.0, -19.0, 19.0, 2),
        ] {
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            let output = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    draw(
                        ui,
                        Some(Refresh::Waiting {
                            remaining: Duration::from_millis(milliseconds),
                            progress,
                            checking: 1,
                        }),
                    );
                });
            });
            let values: Vec<_> = output
                .platform_output
                .accesskit_update
                .into_iter()
                .flat_map(|tree| tree.nodes)
                .filter(|(_, node)| node.role() == egui::accesskit::Role::ProgressIndicator)
                .filter_map(|(_, node)| node.numeric_value())
                .collect();
            assert_eq!(values, vec![percent]);
            let center = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Circle(circle) => Some(circle.center),
                    _ => None,
                })
                .context("countdown circle")?;
            let text = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text)
                        if (text.pos + text.galley.size() * 0.5).distance(center) < 0.01 =>
                    {
                        Some(text.galley.job.text.as_str())
                    }
                    _ => None,
                })
                .context("centered countdown value")?;
            assert_eq!(text, format!("{seconds}s"));
            let arc = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Path(path) if path.points.len() == 65 => Some(path),
                    _ => None,
                })
                .context("progress arc")?;
            let first = arc.points.first().context("arc start")?;
            let last = arc.points.last().context("arc end")?;
            assert!((last.x - first.x - dx).abs() < 0.001);
            assert!((last.y - first.y - dy).abs() < 0.001);
        }
        Ok(())
    }
}
