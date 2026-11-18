use super::App;
use anyhow::Context;
use eframe::egui;

use super::ui::{dialogs::Dialog, theme};
use crate::config::Config;

impl App {
    pub(super) fn open_settings(&mut self, ctx: &egui::Context) {
        self.message = None;
        let draft = match self
            .path
            .as_ref()
            .context("Configuration path unavailable")
            .and_then(|path| Config::load(path))
        {
            Ok(mut saved) => {
                saved.theme = saved.theme.or(self.config.theme);
                self.writable = true;
                saved
            }
            Err(error) => {
                self.writable = false;
                self.message = Some(format!(
                    "Could not load saved settings: {error:#}. Fix the file and reopen Settings. The last valid settings remain active."
                ));
                self.config.clone()
            }
        };
        if let Some(appearance) = draft.theme {
            theme::apply(ctx, appearance);
        }
        self.dialog = Some(Dialog::Settings(draft));
    }

    pub(super) fn save(&mut self, candidate: Config) -> bool {
        if !self.writable {
            return false;
        }
        let Some(path) = &self.path else {
            return false;
        };
        match candidate.save(path) {
            Ok(()) => {
                if let Some(monitor) = &mut self.monitor {
                    monitor.reconfigure(&self.config, &candidate);
                }
                self.config = candidate;
                self.message = Some("Settings saved and applied.".into());
                true
            }
            Err(error) => {
                self.message = Some(format!("Save failed; changes were not applied: {error:#}"));
                false
            }
        }
    }

    pub(super) fn reload(&mut self) -> bool {
        let Some(path) = &self.path else {
            return false;
        };
        match Config::load(path) {
            Ok(mut config) => {
                config.theme = config.theme.or(self.config.theme);
                if let Some(monitor) = &mut self.monitor {
                    monitor.reset(&config, false);
                }
                self.config = config;
                self.writable = true;
                self.message =
                    Some("YAML reloaded. Monitoring history and failure counts were reset.".into());
                true
            }
            Err(error) => {
                self.writable = false;
                self.message = Some(format!(
                    "Could not load YAML: {error:#}. The file is unchanged; monitoring continues with the last valid settings."
                ));
                false
            }
        }
    }
}
