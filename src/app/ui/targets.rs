use std::time::Duration;

use eframe::egui::{self, RichText};

use super::theme;
use crate::{app::App, checker::state::Health};

pub fn draw(app: &App, ui: &mut egui::Ui) -> Option<usize> {
    let mut remove = None;
    let alarmed: Vec<bool> = app.monitor.as_ref().map_or_else(Vec::new, |monitor| {
        monitor.states.iter().map(|state| state.alarmed).collect()
    });
    egui::ScrollArea::horizontal()
        .id_salt("target_table")
        .show(ui, |ui| {
            egui::Grid::new("targets")
                .striped(true)
                .with_row_color(move |row, style| {
                    if row
                        .checked_sub(1)
                        .and_then(|index| alarmed.get(index))
                        .copied()
                        .unwrap_or(false)
                    {
                        Some(if style.visuals.dark_mode {
                            theme::ALARM_ROW_DARK
                        } else {
                            theme::ALARM_ROW_LIGHT
                        })
                    } else {
                        (row % 2 == 1).then_some(style.visuals.faint_bg_color)
                    }
                })
                .spacing([20.0, 16.0])
                .min_col_width(60.0)
                .show(ui, |ui| {
                    for title in [
                        "Status",
                        "Target / Latest result",
                        "Protocol",
                        "Latency",
                        "Failures",
                        "Downtime",
                        "Actions",
                    ] {
                        ui.strong(title);
                    }
                    ui.end_row();
                    for (index, target) in app.config.targets.iter().enumerate() {
                        let state = app
                            .monitor
                            .as_ref()
                            .and_then(|monitor| monitor.states.get(index));
                        let health = state.map_or(&Health::Pending, |state| &state.health);
                        let color = theme::status_color(health, ui);
                        let text = if state.is_some_and(|state| state.alarmed) {
                            "ALERT"
                        } else {
                            match health {
                                Health::Pending => "Pending",
                                Health::Up => "UP",
                                Health::Down => "DOWN",
                            }
                        };
                        ui.label(RichText::new(text).color(color).strong());
                        ui.vertical(|ui| {
                            ui.set_width(270.0);
                            ui.add(
                                egui::Label::new(RichText::new(&target.name).strong()).truncate(),
                            )
                            .on_hover_text(&target.name);
                            let address = target.endpoint.address();
                            ui.add(egui::Label::new(&address).truncate())
                                .on_hover_text(&address);
                            if let Some(state) = state {
                                ui.add(
                                    egui::Label::new(RichText::new(&state.detail).small())
                                        .truncate(),
                                )
                                .on_hover_text(&state.detail);
                            }
                        });
                        ui.label(target.endpoint.label());
                        ui.label(state.and_then(|s| s.latency).map_or_else(
                            || "-".into(),
                            |d| format!("{:.1} ms", d.as_secs_f64() * 1000.0),
                        ));
                        ui.label(
                            state
                                .map_or_else(|| "-".into(), |s| s.consecutive_failures.to_string()),
                        );
                        ui.label(
                            state
                                .and_then(|s| s.down_since)
                                .map_or_else(|| "-".into(), |since| elapsed(since.elapsed())),
                        );
                        if ui
                            .add_enabled(app.writable, egui::Button::new("Remove"))
                            .clicked()
                        {
                            remove = Some(index);
                        }
                        ui.end_row();
                    }
                });
        });
    remove
}

fn elapsed(duration: Duration) -> String {
    let seconds = duration.as_secs();
    format!("{}m {}s", seconds / 60, seconds % 60)
}
