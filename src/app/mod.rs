use std::{path::PathBuf, time::Duration};

use eframe::egui;

use self::{
    audio::Beeper,
    ui::{
        dialogs::{self, Dialog},
        entry::Entry,
        theme,
    },
};
use crate::{checker::monitor::Monitor, config::Config};

pub mod audio;
pub mod settings;
pub mod ui;

pub struct App {
    pub config: Config,
    pub path: Option<PathBuf>,
    pub monitor: Option<Monitor>,
    pub message: Option<String>,
    pub writable: bool,
    pub dialog: Option<Dialog>,
    beeper: Beeper,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::setup(&cc.egui_ctx);
        let mut message = None;
        let (path, config, writable) = match Config::path() {
            Ok(path) => {
                let loaded = if path.exists() {
                    Config::load(&path)
                } else {
                    let config = Config::default();
                    config.save(&path).map(|()| config)
                };
                match loaded {
                    Ok(config) => (Some(path), config, true),
                    Err(error) => {
                        message = Some(format!(
                            "Could not load YAML: {error:#}. Fix the file and reload."
                        ));
                        (Some(path), Config::default(), false)
                    }
                }
            }
            Err(error) => {
                message = Some(format!(
                    "Could not locate the configuration file: {error:#}"
                ));
                (None, Config::default(), false)
            }
        };
        if let Some(appearance) = config.theme {
            theme::apply(&cc.egui_ctx, appearance);
        }
        let monitor = match Monitor::new(&config) {
            Ok(monitor) => Some(monitor),
            Err(error) => {
                message = Some(format!("Could not initialize monitoring: {error:#}"));
                None
            }
        };
        Self {
            config,
            path,
            monitor,
            message,
            writable,
            dialog: None,
            beeper: Beeper::default(),
        }
    }

    /// UI entry point shared by the native frame and real-widget tests.
    pub fn ui(&mut self, ctx: &egui::Context) {
        // Native OS appearance arrives with the first input frame, after new().
        if self.config.theme.is_none() {
            theme::initialize(ctx, &mut self.config);
        }
        if let Some(action) = ui::draw(self, ctx) {
            self.handle_action(action, ctx);
            ctx.request_repaint();
        }
        // Own the form locally while rendering; views cannot mutate App.
        if let Some(mut dialog) = self.dialog.take() {
            let action = dialogs::draw(&mut dialog, self, ctx);
            self.dialog = Some(dialog);
            if let Some(action) = action {
                match action {
                    dialogs::Action::Save(candidate) => {
                        if self.save(candidate) {
                            self.close_dialog(ctx);
                        }
                    }
                    dialogs::Action::Close => self.close_dialog(ctx),
                    dialogs::Action::PreviewTheme(appearance) => theme::apply(ctx, appearance),
                    dialogs::Action::TestBeep(draft) => self.test_beep(&draft),
                    dialogs::Action::Reload => {
                        if self.reload() {
                            self.dialog = Some(Dialog::Settings(self.config.clone()));
                            if let Some(appearance) = self.config.theme {
                                theme::apply(ctx, appearance);
                            }
                        }
                    }
                }
                ctx.request_repaint();
            }
        }
    }

    fn handle_action(&mut self, action: ui::Action, ctx: &egui::Context) {
        match action {
            ui::Action::AddTarget => {
                self.message = None;
                self.dialog = Some(Dialog::AddTarget(Entry::default()));
            }
            ui::Action::Settings => self.open_settings(ctx),
            ui::Action::CheckNow => {
                if let Some(monitor) = &mut self.monitor {
                    monitor.reset(&self.config, true);
                }
            }
            ui::Action::DismissMessage => self.message = None,
            ui::Action::RemoveTarget(index) => {
                let mut candidate = self.config.clone();
                candidate.targets.remove(index);
                self.save(candidate);
            }
        }
    }

    fn close_dialog(&mut self, ctx: &egui::Context) {
        self.dialog = None;
        if let Some(appearance) = self.config.theme {
            theme::apply(ctx, appearance);
        }
    }

    fn test_beep(&mut self, draft: &Config) {
        self.message = Some(match self.beeper.play(draft) {
            Ok(()) => "Beep played. Check the system volume and output device.".into(),
            Err(error) => format!("Could not play the beep: {error:#}"),
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(monitor) = &mut self.monitor {
            if let Err(error) = monitor.tick(&self.config, ctx) {
                self.message = Some(format!("Monitoring error: {error:#}"));
            }
            let mut alarm = false;
            for state in &mut monitor.states {
                alarm |= state.alarm_due(self.config.failure_threshold, self.config.beep_enabled);
            }
            if alarm && let Err(error) = self.beeper.play(&self.config) {
                self.message = Some(format!("Alert beep failed: {error:#}"));
            }
        }
        self.ui(ctx);
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

#[cfg(test)]
#[path = "tests/dialog_layout.rs"]
mod dialog_layout_tests;
#[cfg(test)]
#[path = "tests/layout.rs"]
mod layout_tests;
#[cfg(test)]
#[path = "tests/modal.rs"]
mod modal_tests;
#[cfg(test)]
#[path = "tests/settings.rs"]
mod settings_tests;
#[cfg(test)]
#[path = "tests/state.rs"]
mod state_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
#[path = "tests/harness.rs"]
mod ui_test_harness;
