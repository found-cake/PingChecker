use super::*;
use anyhow::{Context, Result};
use egui::{accesskit::Role, epaint::Shape};

use super::ui_test_harness::Harness;

#[test]
fn default_screen_has_no_editable_forms() -> Result<()> {
    let h = Harness::new()?;
    let tree = h
        .output
        .platform_output
        .accesskit_update
        .as_ref()
        .context("accessibility tree")?;
    assert_eq!(
        tree.nodes
            .iter()
            .filter(|(_, node)| matches!(node.role(), Role::TextInput | Role::SpinButton))
            .count(),
        0
    );
    assert!(h.app.dialog.is_none());
    Ok(())
}

#[test]
fn target_modal_adds_a_target_and_closes_after_saving() -> Result<()> {
    for protocol in ["ICMP", "TCP", "HTTP"] {
        let mut h = Harness::new()?;
        h.click("Add target")?;
        assert!(matches!(h.app.dialog, Some(Dialog::AddTarget(_))));
        if protocol != "ICMP" {
            h.click("ICMP")?;
            h.click(protocol)?;
        }
        h.type_text(0, "local")?;
        h.type_text(
            1,
            if protocol == "HTTP" {
                "http://localhost/health"
            } else {
                "127.0.0.1"
            },
        )?;

        h.click("Add")?;

        assert!(h.app.dialog.is_none());
        assert_eq!(h.app.config.targets.len(), 1);
        assert_eq!(
            h.app.config.targets.first().context("target")?.name,
            "local"
        );
        assert_eq!(
            h.app
                .config
                .targets
                .first()
                .context("target")?
                .endpoint
                .label(),
            protocol
        );
        assert_eq!(
            Config::load(h.app.path.as_ref().context("path")?)?,
            h.app.config
        );
    }
    Ok(())
}

#[test]
fn escape_closes_a_protocol_popup_before_the_add_modal() -> Result<()> {
    let mut h = Harness::new()?;
    h.click("Add target")?;
    h.click("ICMP")?;

    h.escape();
    assert!(matches!(h.app.dialog, Some(Dialog::AddTarget(_))));
    h.escape();
    assert!(h.app.dialog.is_none());
    Ok(())
}

#[test]
fn invalid_target_keeps_the_modal_open_and_file_unchanged() -> Result<()> {
    let mut h = Harness::new()?;
    h.click("Add target")?;

    h.click("Add")?;

    assert!(matches!(h.app.dialog, Some(Dialog::AddTarget(_))));
    assert!(h.app.message.is_some());
    assert!(h.app.config.targets.is_empty());
    assert_eq!(
        Config::load(h.app.path.as_ref().context("path")?)?,
        Config::default()
    );
    h.click("Cancel")?;
    assert!(h.app.dialog.is_none());
    Ok(())
}

#[test]
fn cancelled_target_entry_is_empty_when_reopened() -> Result<()> {
    let mut h = Harness::new()?;
    h.click("Add target")?;
    h.type_text(0, "discarded")?;
    h.type_text(1, "127.0.0.1")?;

    h.click("Cancel")?;
    assert!(h.app.dialog.is_none());
    h.click("Add target")?;

    let tree = h
        .output
        .platform_output
        .accesskit_update
        .as_ref()
        .context("accessibility tree")?;
    let values: Vec<_> = tree
        .nodes
        .iter()
        .filter(|(_, node)| node.role() == Role::TextInput)
        .map(|(_, node)| node.value())
        .collect();
    assert_eq!(values, vec![Some(""), Some("")]);
    assert!(h.app.config.targets.is_empty());
    Ok(())
}

#[test]
fn settings_modal_saves_and_closes() -> Result<()> {
    let mut h = Harness::new()?;
    h.click("Settings")?;
    h.set_number("Request interval", 30.0)?;
    h.set_number("Beep duration", 1200.0)?;
    h.set_number("Volume", 55.0)?;

    h.click("Save settings")?;

    assert!(h.app.dialog.is_none());
    assert_eq!(h.app.config.interval_seconds, 30);
    assert_eq!(h.app.config.beep_duration_ms, 1200);
    assert_eq!(h.app.config.beep_volume_percent, 55);
    assert_eq!(
        Config::load(h.app.path.as_ref().context("path")?)?,
        h.app.config
    );
    Ok(())
}

#[test]
fn settings_save_failure_keeps_the_modal_open() -> Result<()> {
    let mut h = Harness::new()?;
    h.click("Settings")?;
    let original_path = h.app.path.clone().context("path")?;
    h.app.path = Some(original_path.join("missing").join("ping.yml"));
    h.set_number("Request interval", 30.0)?;

    h.click("Save settings")?;

    assert!(matches!(h.app.dialog, Some(Dialog::Settings(_))));
    assert_eq!(h.app.config.interval_seconds, 5);
    assert_eq!(h.number("Request interval")?.1, 30.0);
    assert!(h.app.message.is_some());
    assert_eq!(Config::load(&original_path)?.interval_seconds, 5);
    Ok(())
}

#[test]
fn dismissing_settings_discards_changes_and_blocks_background_actions() -> Result<()> {
    #[derive(Debug)]
    enum Close {
        Cancel,
        Escape,
        Backdrop,
        BackgroundButton,
    }
    for close in [
        Close::Cancel,
        Close::Escape,
        Close::Backdrop,
        Close::BackgroundButton,
    ] {
        let mut h = Harness::new()?;
        h.click("Settings")?;
        h.set_number("Request interval", 30.0)?;
        match close {
            Close::Cancel => h.click("Cancel")?,
            Close::Escape => h.escape(),
            Close::Backdrop => h.pointer_click(egui::pos2(2.0, 2.0)),
            Close::BackgroundButton => {
                let pos = h
                    .output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        Shape::Text(text) if text.galley.job.text == "Add target" => {
                            Some(text.pos + text.galley.size() * 0.5)
                        }
                        _ => None,
                    })
                    .context("background add button")?;
                h.pointer_click(pos);
            }
        }
        assert!(h.app.dialog.is_none(), "close method {close:?}");
        assert_eq!(h.app.config.interval_seconds, 5);
        assert!(h.app.config.targets.is_empty());
        assert_eq!(
            Config::load(h.app.path.as_ref().context("path")?)?.interval_seconds,
            5
        );
        h.click("Settings")?;
        assert_eq!(h.number("Request interval")?.1, 5.0);
    }
    Ok(())
}

#[test]
fn last_check_metadata_does_not_add_content_to_the_monitoring_view() -> Result<()> {
    let mut h = Harness::new()?;
    let mut config = h.app.config.clone();
    config.targets.push(crate::config::Target {
        name: "local".into(),
        endpoint: crate::config::Endpoint::Icmp {
            host: "127.0.0.1".into(),
        },
    });
    assert!(h.app.save(config));
    h.app.message = None;
    h.frame(Vec::new());
    h.frame(Vec::new());
    let geometry = |output: &egui::FullOutput| {
        output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                Shape::Text(text) => Some((text.pos, text.galley.size())),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let before = geometry(&h.output);
    h.app
        .monitor
        .as_mut()
        .context("monitor")?
        .states
        .first_mut()
        .context("state")?
        .checked_at = Some(std::time::SystemTime::UNIX_EPOCH);

    h.frame(Vec::new());

    assert_eq!(geometry(&h.output), before);
    Ok(())
}
