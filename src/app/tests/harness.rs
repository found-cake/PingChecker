use super::*;
use anyhow::{Context, Result};
use egui::{
    Event, FullOutput, Pos2,
    accesskit::{Action, ActionData, ActionRequest, NodeId, Role},
};

pub(super) struct Harness {
    pub app: App,
    pub ctx: egui::Context,
    pub output: FullOutput,
    pub screen_size: egui::Vec2,
    _directory: tempfile::TempDir,
}

impl Harness {
    pub fn new() -> Result<Self> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("ping.yml");
        let mut config = Config::default();
        config.save(&path)?;
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        theme::setup(&ctx);
        theme::initialize(&ctx, &mut config);
        let app = App {
            monitor: Some(Monitor::new(&config)?),
            config,
            path: Some(path),
            message: None,
            writable: true,
            dialog: None,
            beeper: Beeper::default(),
        };
        let mut harness = Self {
            app,
            ctx,
            output: FullOutput::default(),
            screen_size: egui::vec2(1100.0, 720.0),
            _directory: directory,
        };
        harness.frame(Vec::new());
        harness.frame(Vec::new());
        Ok(harness)
    }

    pub fn frame(&mut self, events: Vec<Event>) {
        self.output = self.ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, self.screen_size)),
                time: Some(self.ctx.input(|input| input.time) + 1.0 / 60.0),
                events,
                ..Default::default()
            },
            |ui| self.app.ui(ui),
        );
        self.output.textures_delta.clear();
    }

    pub fn click(&mut self, label: &str) -> Result<()> {
        let target = self
            .output
            .platform_output
            .accesskit_update
            .as_ref()
            .context("accessibility tree")?
            .nodes
            .iter()
            .find(|(_, node)| {
                node.supports_action(Action::Click)
                    && (node.label() == Some(label) || node.value() == Some(label))
            })
            .map(|(id, _)| *id)
            .with_context(|| format!("button {label}"))?;
        self.frame(vec![Event::AccessKitActionRequest(ActionRequest {
            target_tree: egui::accesskit::TreeId::ROOT,
            target_node: target,
            action: Action::Click,
            data: None,
        })]);
        self.frame(Vec::new());
        Ok(())
    }

    pub fn type_text(&mut self, index: usize, text: &str) -> Result<()> {
        let mut fields: Vec<_> = self
            .output
            .platform_output
            .accesskit_update
            .as_ref()
            .context("accessibility tree")?
            .nodes
            .iter()
            .filter(|(_, node)| node.role() == Role::TextInput)
            .map(|(id, node)| (*id, node.bounds().map_or(f64::MAX, |rect| rect.y0)))
            .collect();
        fields.sort_by(|left, right| left.1.total_cmp(&right.1));
        let target = fields.get(index).context("text field")?.0;
        self.frame(vec![Event::AccessKitActionRequest(ActionRequest {
            target_tree: egui::accesskit::TreeId::ROOT,
            target_node: target,
            action: Action::Focus,
            data: None,
        })]);
        self.frame(vec![Event::Text(text.to_owned())]);
        Ok(())
    }

    pub fn number(&self, label: &str) -> Result<(NodeId, f64)> {
        let tree = self
            .output
            .platform_output
            .accesskit_update
            .as_ref()
            .context("accessibility tree")?;
        let label = tree
            .nodes
            .iter()
            .find(|(_, node)| node.role() == Role::Label && node.value() == Some(label))
            .map(|(id, _)| *id)
            .with_context(|| format!("numeric label {label}"))?;
        let (id, node) = tree
            .nodes
            .iter()
            .find(|(_, node)| {
                node.supports_action(Action::SetValue) && node.labelled_by().contains(&label)
            })
            .context("numeric control")?;
        Ok((*id, node.numeric_value().context("numeric value")?))
    }

    pub fn set_number(&mut self, label: &str, value: f64) -> Result<()> {
        let target = self.number(label)?.0;
        self.frame(vec![Event::AccessKitActionRequest(ActionRequest {
            target_tree: egui::accesskit::TreeId::ROOT,
            target_node: target,
            action: Action::SetValue,
            data: Some(ActionData::NumericValue(value)),
        })]);
        self.frame(Vec::new());
        Ok(())
    }

    pub fn pointer_click(&mut self, pos: Pos2) {
        self.frame(vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        self.frame(vec![Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        self.frame(Vec::new());
    }

    pub fn escape(&mut self) {
        self.frame(vec![Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        self.frame(vec![Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]);
    }
}
