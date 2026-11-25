use std::time::Duration;

use eframe::egui::{self, Align2, FontId, Sense, Stroke, WidgetInfo, WidgetType, emath::Rot2};

use crate::checker::monitor::Refresh;

const ARC_REPAINT_INTERVAL: Duration = Duration::from_millis(33);

struct Presentation {
    progress: Option<f32>,
    label: String,
    center_text: String,
    show_caption: bool,
}

impl Presentation {
    fn status(center_text: &str, label: impl Into<String>) -> Self {
        Self {
            progress: None,
            label: label.into(),
            center_text: center_text.into(),
            show_caption: true,
        }
    }
}

pub fn draw(ui: &mut egui::Ui, refresh: Option<Refresh>) {
    let presentation = match refresh {
        None => Presentation::status("--", "Monitoring unavailable"),
        Some(Refresh::Inactive) => Presentation::status("--", "No targets"),
        Some(Refresh::Due) => {
            ui.ctx().request_repaint();
            Presentation {
                show_caption: false,
                ..Presentation::status("0s", "Refresh due")
            }
        }
        Some(Refresh::Waiting {
            remaining,
            progress,
        }) => {
            let subsecond = Duration::from_nanos(u64::from(remaining.subsec_nanos()));
            let until_next_second = if subsecond.is_zero() {
                Duration::from_secs(1)
            } else {
                subsecond
            };
            // Keep background wakeups unchanged while animating the active window.
            let animate =
                ui.input(|input| input.focused && input.viewport().minimized != Some(true));
            ui.ctx().request_repaint_after(if animate {
                until_next_second.min(ARC_REPAINT_INTERVAL)
            } else {
                until_next_second
            });
            let seconds = remaining
                .as_secs()
                .saturating_add(u64::from(remaining.subsec_nanos() != 0));
            Presentation {
                progress: Some(progress),
                label: format!("Next refresh in {seconds} s"),
                center_text: format!("{seconds}s"),
                show_caption: false,
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
        const SEGMENTS: u16 = 64;
        let step = Rot2::from_angle(std::f32::consts::TAU * progress / f32::from(SEGMENTS));
        let mut offset = egui::vec2(0.0, -radius);
        // Reuse the same angular step instead of evaluating sin/cos at each point.
        let points = (0..=SEGMENTS)
            .map(|_| {
                let point = center + offset;
                offset = step * offset;
                point
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
    if presentation.show_caption {
        ui.label(presentation.label);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{Context, Result};
    #[test]
    fn scheduled_progress_is_delivered_to_the_accessibility_tree() -> Result<()> {
        for (progress, milliseconds, percent, seconds) in [
            (0.01, 4950, 1.0, 5),
            (0.25, 3750, 25.0, 4),
            (0.5, 2500, 50.0, 3),
            (0.75, 1250, 75.0, 2),
            (0.99, 50, 99.0, 1),
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
            let [value] = values.as_slice() else {
                anyhow::bail!("expected one accessible progress value, got {values:?}");
            };
            assert!((value - percent).abs() < 0.001);
            let center = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Circle(circle) => Some(circle.center),
                    _ => None,
                })
                .context("countdown circle")?;
            let mut text = output.shapes.iter().filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text),
                _ => None,
            });
            let label = text.next().context("countdown value")?;
            assert!((label.pos + label.galley.size() * 0.5).distance(center) < 0.01);
            assert_eq!(label.galley.job.text, format!("{seconds}s"));
            assert!(text.next().is_none(), "no separate checking caption");
            let arc = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Path(path) if path.points.len() == 65 => Some(path),
                    _ => None,
                })
                .context("progress arc")?;
            for (index, point) in (0..=64_u16).zip(&arc.points) {
                let angle = std::f32::consts::TAU * progress * f32::from(index) / 64.0;
                let expected = center + 19.0 * egui::vec2(angle.sin(), -angle.cos());
                assert!(point.distance(expected) < 0.001, "arc point {index}");
            }
        }
        Ok(())
    }

    #[test]
    fn arc_only_requests_intermediate_frames_in_the_active_window() -> Result<()> {
        for (focused, minimized, fast_repaint) in [
            (true, false, true),
            (false, false, false),
            (true, true, false),
        ] {
            let ctx = egui::Context::default();
            let mut input = egui::RawInput {
                focused,
                ..Default::default()
            };
            input
                .viewports
                .get_mut(&egui::ViewportId::ROOT)
                .context("root viewport")?
                .minimized = Some(minimized);
            // Let egui's startup repaints settle before measuring the widget's request.
            for _ in 0..3 {
                drop(ctx.run(input.clone(), |_| {}));
            }
            let output = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    draw(
                        ui,
                        Some(Refresh::Waiting {
                            remaining: Duration::from_millis(2500),
                            progress: 0.5,
                        }),
                    );
                });
            });
            let repaint_delay = output
                .viewport_output
                .get(&egui::ViewportId::ROOT)
                .context("root viewport output")?
                .repaint_delay;
            if fast_repaint {
                assert!(repaint_delay < Duration::from_millis(50));
            } else {
                assert!(repaint_delay > Duration::from_millis(400));
                assert!(repaint_delay <= Duration::from_millis(500));
            }
        }
        Ok(())
    }
}
