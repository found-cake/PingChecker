use super::ui_test_harness::Harness;
use crate::{app::ui::theme, config::Appearance};
use anyhow::{Context, Result};
use eframe::egui::{self, Pos2, Rect, epaint::Shape};

fn cancel_is_visible(output: &egui::FullOutput) -> bool {
    let bounds = output
        .platform_output
        .accesskit_update
        .as_ref()
        .and_then(|tree| {
            tree.nodes.iter().find_map(|(_, node)| {
                (node.label() == Some("Cancel"))
                    .then(|| node.bounds())
                    .flatten()
            })
        });
    let clip = output.shapes.iter().find_map(|shape| match &shape.shape {
        Shape::Text(text) if text.galley.job.text == "Cancel" => Some(shape.clip_rect),
        _ => None,
    });
    bounds.zip(clip).is_some_and(|(bounds, clip)| {
        bounds.x0 >= f64::from(clip.left())
            && bounds.x1 <= f64::from(clip.right())
            && bounds.y0 >= f64::from(clip.top())
            && bounds.y1 <= f64::from(clip.bottom())
    })
}

#[test]
fn dialogs_fit_short_forms_and_scroll_only_real_overflow() -> Result<()> {
    for appearance in [Appearance::Dark, Appearance::Light] {
        for size in [egui::vec2(1100.0, 720.0), egui::vec2(700.0, 520.0)] {
            let mut h = Harness::new()?;
            h.screen_size = size;
            h.app.config.theme = Some(appearance);
            theme::apply(&h.ctx, appearance);
            h.ctx.style_mut(|style| style.animation_time = 0.0);
            h.frame(Vec::new());
            h.click("Add target")?;
            for protocol in ["ICMP", "TCP", "HTTP"] {
                if protocol != "ICMP" {
                    h.click(if protocol == "TCP" { "ICMP" } else { "TCP" })?;
                    h.click(protocol)?;
                }
                assert!(
                    cancel_is_visible(&h.output),
                    "{protocol} needs scrolling at {size:?}, {appearance:?}"
                );
                if protocol == "TCP" {
                    h.click("Add")?;
                    let error = h.app.message.as_deref().context("TCP validation error")?;
                    assert!(
                        h.output.shapes.iter().any(|shape| match &shape.shape {
                            Shape::Text(text) if text.galley.job.text == error => shape
                                .clip_rect
                                .contains_rect(Rect::from_min_size(text.pos, text.galley.size())),
                            _ => false,
                        }),
                        "TCP validation error is clipped at {size:?}, {appearance:?}"
                    );
                }
            }
            h.click("Cancel")?;
            h.click("Settings")?;
            if size.y > 520.0 {
                assert!(
                    cancel_is_visible(&h.output),
                    "settings need scrolling despite available space at {size:?}, {appearance:?}"
                );
            } else {
                assert!(
                    !cancel_is_visible(&h.output),
                    "small settings should overflow"
                );
                let scroll_point = h
                    .output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        Shape::Text(text) if text.galley.job.text == "Theme" => {
                            Some(Pos2::new(text.pos.x + 4.0, shape.clip_rect.center().y))
                        }
                        _ => None,
                    })
                    .context("settings scroll viewport")?;
                h.frame(vec![
                    egui::Event::PointerMoved(scroll_point),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0.0, -2000.0),
                        modifiers: egui::Modifiers::NONE,
                    },
                ]);
                h.frame(Vec::new());
                assert!(
                    cancel_is_visible(&h.output),
                    "settings cannot scroll to Cancel"
                );
            }
        }
    }
    Ok(())
}
