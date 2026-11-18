use super::*;
use crate::config::Appearance;
use anyhow::{Context, Result};

use super::ui_test_harness::Harness;

#[test]
fn successful_yaml_reload_updates_the_open_settings_form() -> Result<()> {
    let mut h = Harness::new()?;
    h.click("Settings")?;
    let saved = Config {
        theme: Some(Appearance::Light),
        interval_seconds: 23,
        ..h.app.config.clone()
    };
    saved.save(h.app.path.as_ref().context("path")?)?;

    h.click("Reload YAML")?;

    assert_eq!(h.number("Request interval")?.1, 23.0);
    assert_eq!(h.app.config, saved);
    assert_eq!(h.ctx.theme(), egui::Theme::Light);
    assert!(matches!(h.app.dialog, Some(Dialog::Settings(_))));
    Ok(())
}

#[test]
fn first_render_uses_the_system_theme_then_pins_the_choice() -> Result<()> {
    let mut h = Harness::new()?;
    h.app.config.theme = None;
    h.ctx.set_theme(egui::ThemePreference::System);

    for system in [egui::Theme::Light, egui::Theme::Dark] {
        let _frame = h.ctx.run(
            egui::RawInput {
                system_theme: Some(system),
                ..Default::default()
            },
            |ctx| h.app.ui(ctx),
        );
        assert_eq!(h.app.config.theme, Some(Appearance::Light));
        assert_eq!(h.ctx.theme(), egui::Theme::Light);
    }
    Ok(())
}

#[test]
fn settings_open_reads_saved_values_without_applying_monitoring_settings() -> Result<()> {
    let mut h = Harness::new()?;
    let original = h.app.config.clone();
    let saved = Config {
        theme: Some(Appearance::Light),
        interval_seconds: 19,
        timeout_seconds: 7,
        failure_threshold: 9,
        beep_duration_ms: 1700,
        beep_volume_percent: 61,
        ..h.app.config.clone()
    };
    saved.save(h.app.path.as_ref().context("path")?)?;

    h.click("Settings")?;

    for (label, expected) in [
        ("Request interval", 19.0),
        ("Timeout", 7.0),
        ("Failure threshold", 9.0),
        ("Beep duration", 1700.0),
        ("Volume", 61.0),
    ] {
        assert_eq!(h.number(label)?.1, expected, "{label}");
    }
    assert_eq!(h.ctx.theme(), egui::Theme::Light);
    assert_eq!(h.app.config, original);
    assert_eq!(Config::load(h.app.path.as_ref().context("path")?)?, saved);
    Ok(())
}

#[test]
fn failed_yaml_reload_keeps_the_current_preview_and_active_settings() -> Result<()> {
    let mut h = Harness::new()?;
    h.click("Settings")?;
    h.click("Light")?;
    let original = h.app.config.clone();
    let path = h.app.path.clone().context("path")?;
    std::fs::write(&path, "invalid: [")?;

    h.click("Reload YAML")?;

    assert_eq!(h.ctx.theme(), egui::Theme::Light);
    assert_eq!(h.app.config, original);
    assert!(!h.app.writable);
    assert_eq!(std::fs::read_to_string(path)?, "invalid: [");
    Ok(())
}

#[test]
fn edited_controls_and_theme_survive_save_and_reopen() -> Result<()> {
    let mut h = Harness::new()?;
    h.click("Settings")?;
    for (label, value) in [
        ("Request interval", 17.0),
        ("Timeout", 6.0),
        ("Failure threshold", 4.0),
        ("Beep duration", 1300.0),
        ("Volume", 63.0),
    ] {
        h.set_number(label, value)?;
    }
    h.click("Light")?;

    h.click("Save settings")?;
    h.click("Settings")?;

    for (label, expected) in [
        ("Request interval", 17.0),
        ("Timeout", 6.0),
        ("Failure threshold", 4.0),
        ("Beep duration", 1300.0),
        ("Volume", 63.0),
    ] {
        assert_eq!(h.number(label)?.1, expected, "{label}");
    }
    let saved = Config::load(h.app.path.as_ref().context("path")?)?;
    assert_eq!(saved.theme, Some(Appearance::Light));
    let restarted = egui::Context::default();
    let _frame = restarted.run(
        egui::RawInput {
            system_theme: Some(egui::Theme::Dark),
            ..Default::default()
        },
        |_| {},
    );
    let mut saved = saved;
    theme::initialize(&restarted, &mut saved);
    assert_eq!(restarted.theme(), egui::Theme::Light);
    Ok(())
}

#[test]
fn cancel_restores_the_active_theme_and_keeps_saved_settings() -> Result<()> {
    let mut h = Harness::new()?;
    h.click("Settings")?;
    h.click("Light")?;
    assert_eq!(h.ctx.theme(), egui::Theme::Light);

    h.click("Cancel")?;

    assert_eq!(h.ctx.theme(), egui::Theme::Dark);
    assert_eq!(
        Config::load(h.app.path.as_ref().context("path")?)?.theme,
        None
    );
    Ok(())
}

#[test]
fn invalid_saved_settings_remain_untouched_and_reopening_recovers() -> Result<()> {
    let mut h = Harness::new()?;
    let original = h.app.config.clone();
    let path = h.app.path.clone().context("path")?;
    std::fs::write(&path, "invalid: [")?;

    h.click("Settings")?;

    assert!(!h.app.writable);
    assert_eq!(h.app.config, original);
    assert_eq!(h.number("Request interval")?.1, 5.0);
    assert!(h.app.message.is_some());
    assert_eq!(std::fs::read_to_string(&path)?, "invalid: [");
    h.click("Cancel")?;
    original.save(&path)?;
    h.click("Settings")?;
    assert!(h.app.writable);
    assert_eq!(h.number("Request interval")?.1, 5.0);
    Ok(())
}
