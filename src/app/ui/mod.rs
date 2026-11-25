use eframe::egui;

use crate::{app::App, checker::state::Health};

pub mod countdown;
pub mod dialogs;
pub mod entry;
pub mod targets;
pub mod theme;

#[derive(Clone, Copy, Debug)]
pub enum Action {
    AddTarget,
    Settings,
    CheckNow,
    DismissMessage,
    RemoveTarget(usize),
}

pub fn draw(app: &App, ui: &mut egui::Ui) -> Option<Action> {
    let mut action = None;
    egui::Panel::top("header")
        .frame(egui::Frame::side_top_panel(ui.style()).inner_margin(24))
        .show(ui, |ui| {
            ui.heading(crate::APP_NAME);
            let (mut up, mut down, mut pending) = (0, 0, 0);
            if let Some(monitor) = &app.monitor {
                for state in &monitor.states {
                    match state.health {
                        Health::Up => up += 1,
                        Health::Down => down += 1,
                        Health::Pending => pending += 1,
                    }
                }
            } else {
                pending = app.config.targets.len();
            }
            ui.horizontal_wrapped(|ui| {
                ui.label(format!(
                    "{} targets · UP {up} · DOWN {down} · Pending {pending}",
                    app.config.targets.len()
                ));
                if ui
                    .add_enabled(app.writable, egui::Button::new("Add target"))
                    .clicked()
                {
                    action = Some(Action::AddTarget);
                }
                if ui
                    .add_enabled(
                        !app.config.targets.is_empty(),
                        egui::Button::new("Check now"),
                    )
                    .clicked()
                {
                    action = Some(Action::CheckNow);
                }
                if ui.button("Settings").clicked() {
                    action = Some(Action::Settings);
                }
            });
        });
    egui::Panel::bottom("refresh")
        .frame(egui::Frame::side_top_panel(ui.style()).inner_margin(egui::Margin::symmetric(24, 8)))
        .show(ui, |ui| {
            ui.set_height(44.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                countdown::draw(
                    ui,
                    app.monitor.as_ref().map(|monitor| {
                        monitor.refresh(
                            std::time::Instant::now(),
                            std::time::Duration::from_secs(app.config.interval_seconds),
                        )
                    }),
                );
            });
        });
    let opening_dialog = matches!(action, Some(Action::AddTarget | Action::Settings));
    if app.dialog.is_none() && !opening_dialog && app.message.is_some() {
        egui::Panel::bottom("notification")
            .frame(egui::Frame::side_top_panel(ui.style()).inner_margin(16))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if let Some(message) = &app.message {
                        ui.label(message);
                    }
                    if ui.button("Dismiss").clicked() {
                        action = Some(Action::DismissMessage);
                    }
                });
            });
    }
    egui::CentralPanel::default()
        .frame(egui::Frame::central_panel(ui.style()).inner_margin(24))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                if app.config.targets.is_empty() {
                    ui.strong("No monitoring targets.");
                    ui.label("Choose Add target above to start monitoring.");
                } else {
                    if let Some(index) = targets::draw(app, ui) {
                        action = Some(Action::RemoveTarget(index));
                    }
                }
            });
        });
    action
}
