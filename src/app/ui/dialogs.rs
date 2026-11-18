use eframe::egui;

use super::entry::{self, Entry};
use crate::{
    app::App,
    config::{Appearance, Config},
};

pub enum Dialog {
    AddTarget(Entry),
    Settings(Config),
}

pub enum Action {
    Save(Config),
    Close,
    PreviewTheme(Appearance),
    TestBeep(Config),
    Reload,
}

pub fn draw(dialog: &mut Dialog, app: &App, ctx: &egui::Context) -> Option<Action> {
    let (title, id) = match &*dialog {
        Dialog::AddTarget(_) => ("Add target", "add_target_dialog"),
        Dialog::Settings(_) => ("Settings", "settings_dialog"),
    };
    let id = egui::Id::new(id);
    let mut action = None;
    let response = egui::Modal::new(id)
        .area(egui::Modal::default_area(id).fade_in(false))
        .frame(egui::Frame::popup(&ctx.style()).inner_margin(24))
        .show(ctx, |ui| {
            ui.set_width(560.0_f32.min(ctx.content_rect().width() - 96.0));
            ui.set_max_height(ctx.content_rect().height() - 96.0);
            ui.heading(title);
            ui.add_space(8.0);
            egui::ScrollArea::vertical()
                .id_salt("dialog_content")
                .show(ui, |ui| {
                    action = match dialog {
                        Dialog::AddTarget(entry) => entry::draw(entry, &app.config, ui),
                        Dialog::Settings(draft) => settings(draft, app, ui),
                    };
                    if let Some(message) = &app.message {
                        ui.add_space(8.0);
                        ui.label(message);
                    }
                });
        });
    if response.should_close() {
        Some(Action::Close)
    } else {
        action
    }
}

fn settings(draft: &mut Config, app: &App, ui: &mut egui::Ui) -> Option<Action> {
    let mut action = None;
    ui.horizontal(|ui| {
        ui.label("Theme");
        for (appearance, label) in [(Appearance::Light, "Light"), (Appearance::Dark, "Dark")] {
            if ui
                .selectable_value(&mut draft.theme, Some(appearance), label)
                .changed()
            {
                action = Some(Action::PreviewTheme(appearance));
            }
        }
    });
    ui.add_space(8.0);
    ui.add_enabled_ui(app.writable, |ui| {
        egui::Grid::new("settings_fields")
            .spacing([24.0, 10.0])
            .show(ui, |ui| {
                let label = ui.label("Request interval");
                ui.add(
                    egui::DragValue::new(&mut draft.interval_seconds)
                        .range(1..=3600)
                        .suffix(" s"),
                )
                .labelled_by(label.id);
                ui.end_row();
                let label = ui.label("Timeout");
                ui.add(
                    egui::DragValue::new(&mut draft.timeout_seconds)
                        .range(1..=60)
                        .suffix(" s"),
                )
                .labelled_by(label.id);
                ui.end_row();
                let label = ui.label("Failure threshold");
                ui.add(
                    egui::DragValue::new(&mut draft.failure_threshold)
                        .range(1..=1000)
                        .suffix(" failures"),
                )
                .labelled_by(label.id);
                ui.end_row();
                ui.label("Alert beep");
                ui.checkbox(&mut draft.beep_enabled, "Enabled");
                ui.end_row();
                let label = ui.label("Beep duration");
                ui.add(
                    egui::DragValue::new(&mut draft.beep_duration_ms)
                        .range(100..=10000)
                        .speed(100.0)
                        .custom_formatter(|milliseconds, _| format!("{:.1}", milliseconds / 1000.0))
                        .custom_parser(|seconds| {
                            seconds.parse::<f64>().ok().map(|value| value * 1000.0)
                        })
                        .suffix(" s"),
                )
                .labelled_by(label.id);
                ui.end_row();
                let label = ui.label("Volume");
                ui.add(egui::Slider::new(&mut draft.beep_volume_percent, 0..=100).suffix(" %"))
                    .labelled_by(label.id)
                    .on_hover_text("Output also depends on the system volume.");
                ui.end_row();
            });
        if ui.button("Test beep").clicked() {
            action = Some(Action::TestBeep(draft.clone()));
        }
    });
    ui.small("One alert per outage at the failure threshold; recovery rearms the alert.");
    ui.add_space(8.0);
    ui.small("Configuration file");
    ui.label(app.path.as_ref().map_or_else(
        || "Configuration path unavailable".into(),
        |path| path.display().to_string(),
    ));
    if ui.button("Reload YAML").clicked() {
        action = Some(Action::Reload);
    }
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                app.writable && *draft != app.config,
                egui::Button::new("Save settings"),
            )
            .clicked()
        {
            action = Some(Action::Save(draft.clone()));
        }
        if ui.button("Cancel").clicked() {
            action = Some(Action::Close);
        }
        if *draft != app.config {
            ui.small("Unsaved changes; theme is previewed.");
        }
    });
    action
}
