use eframe::egui;

use super::dialogs::Action;
use crate::config::{Config, Endpoint, Target};

#[derive(Default, PartialEq)]
enum Protocol {
    #[default]
    Icmp,
    Tcp,
    Http,
}

pub struct Entry {
    protocol: Protocol,
    name: String,
    address: String,
    port: u16,
}

impl Default for Entry {
    fn default() -> Self {
        Self {
            protocol: Protocol::Icmp,
            name: String::new(),
            address: String::new(),
            port: 443,
        }
    }
}

pub fn draw(entry: &mut Entry, active: &Config, ui: &mut egui::Ui) -> Option<Action> {
    let mut action = None;
    ui.horizontal_wrapped(|ui| {
        ui.label("Protocol");
        egui::ComboBox::from_id_salt("protocol")
            .width(85.0)
            .selected_text(match entry.protocol {
                Protocol::Icmp => "ICMP",
                Protocol::Tcp => "TCP",
                Protocol::Http => "HTTP",
            })
            .show_ui(ui, |ui| {
                let selected = ui
                    .selectable_value(&mut entry.protocol, Protocol::Icmp, "ICMP")
                    .clicked()
                    | ui.selectable_value(&mut entry.protocol, Protocol::Tcp, "TCP")
                        .clicked()
                    | ui.selectable_value(&mut entry.protocol, Protocol::Http, "HTTP")
                        .clicked();
                if selected {
                    ui.close();
                }
            });
        let name_label = ui.label("Name");
        ui.add(
            egui::TextEdit::singleline(&mut entry.name)
                .desired_width(160.0)
                .hint_text("e.g. Office server"),
        )
        .labelled_by(name_label.id);
    });
    ui.horizontal_wrapped(|ui| {
        let address_label = ui.label(match entry.protocol {
            Protocol::Http => "URL",
            Protocol::Icmp | Protocol::Tcp => "Host / IP",
        });
        ui.add(
            egui::TextEdit::singleline(&mut entry.address)
                .desired_width(300.0)
                .hint_text(match entry.protocol {
                    Protocol::Http => "https://example.com/health",
                    Protocol::Icmp | Protocol::Tcp => "127.0.0.1 or server.example.com",
                }),
        )
        .labelled_by(address_label.id);
        if matches!(entry.protocol, Protocol::Tcp) {
            let port_label = ui.label("Port");
            ui.add(egui::DragValue::new(&mut entry.port).range(1..=65535))
                .labelled_by(port_label.id);
        }
    });
    ui.small(match entry.protocol {
        Protocol::Icmp => "Checks for an echo reply using the system ping.",
        Protocol::Tcp => {
            "Only checks that a TCP connection can be established; no application data is sent."
        }
        Protocol::Http => {
            "GET request · up to 5 redirects · only a final 2xx response counts as healthy."
        }
    });
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        if ui
            .add(egui::Button::new("Add").fill(ui.visuals().selection.bg_fill))
            .clicked()
        {
            let address = entry.address.trim().to_owned();
            let endpoint = match entry.protocol {
                Protocol::Icmp => Endpoint::Icmp { host: address },
                Protocol::Tcp => Endpoint::Tcp {
                    host: address,
                    port: entry.port,
                },
                Protocol::Http => Endpoint::Http { url: address },
            };
            let mut config = active.clone();
            config.targets.push(Target {
                name: entry.name.trim().to_owned(),
                endpoint,
            });
            action = Some(Action::Save(config));
        }
        if ui.button("Cancel").clicked() {
            action = Some(Action::Close);
        }
    });
    action
}
