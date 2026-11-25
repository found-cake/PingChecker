use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily, FontId, TextStyle};

use crate::{
    checker::state::Health,
    config::{Appearance, Config},
};

pub const ALARM_ROW_DARK: Color32 = Color32::from_rgb(72, 24, 32);
pub const ALARM_ROW_LIGHT: Color32 = Color32::from_rgb(255, 235, 238);

pub fn initialize(ctx: &egui::Context, config: &mut Config) {
    let appearance = *config.theme.get_or_insert_with(|| {
        match ctx.system_theme().unwrap_or(egui::Theme::Dark) {
            egui::Theme::Light => Appearance::Light,
            egui::Theme::Dark => Appearance::Dark,
        }
    });
    apply(ctx, appearance);
}

pub fn apply(ctx: &egui::Context, appearance: Appearance) {
    ctx.set_theme(match appearance {
        Appearance::Light => egui::Theme::Light,
        Appearance::Dark => egui::Theme::Dark,
    });
}

pub fn setup(ctx: &egui::Context) -> bool {
    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.interact_size.y = 28.0;
    style
        .text_styles
        .insert(TextStyle::Body, FontId::proportional(15.0));
    style
        .text_styles
        .insert(TextStyle::Button, FontId::proportional(15.0));
    style
        .text_styles
        .insert(TextStyle::Heading, FontId::proportional(26.0));
    style
        .text_styles
        .insert(TextStyle::Small, FontId::proportional(12.0));
    // Share the dark layout's geometry and typography; switch only its palette.
    let light_visuals = ctx.style_of(egui::Theme::Light).visuals.clone();
    ctx.set_style_of(egui::Theme::Dark, style.clone());
    style.visuals = light_visuals;
    ctx.set_style_of(egui::Theme::Light, style);
    let candidates = [
        "/System/Library/Fonts/AppleSDGothicNeo.ttc",
        "C:\\Windows\\Fonts\\malgun.ttf",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/nanum/NanumGothic.ttf",
    ];
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            let mut fonts = FontDefinitions::default();
            fonts
                .font_data
                .insert("korean".into(), FontData::from_owned(bytes).into());
            for family in [FontFamily::Proportional, FontFamily::Monospace] {
                fonts
                    .families
                    .entry(family)
                    .or_default()
                    .push("korean".into());
            }
            ctx.set_fonts(fonts);
            return true;
        }
    }
    false
}

pub fn status_color(health: &Health, ui: &egui::Ui) -> Color32 {
    match (health, ui.visuals().dark_mode) {
        (Health::Up, true) => Color32::from_rgb(134, 239, 172),
        (Health::Up, false) => Color32::from_rgb(22, 101, 52),
        (Health::Down, true) => Color32::from_rgb(253, 164, 175),
        (Health::Down, false) => Color32::from_rgb(180, 35, 24),
        (Health::Pending, _) => ui.visuals().text_color(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_theme_is_only_the_initial_default_and_saved_choice_wins() {
        for (system, saved, expected) in [
            (Some(egui::Theme::Light), None, Appearance::Light),
            (Some(egui::Theme::Dark), None, Appearance::Dark),
            (None, None, Appearance::Dark),
            (
                Some(egui::Theme::Light),
                Some(Appearance::Dark),
                Appearance::Dark,
            ),
            (
                Some(egui::Theme::Dark),
                Some(Appearance::Light),
                Appearance::Light,
            ),
        ] {
            let ctx = egui::Context::default();
            let mut frame = ctx.run_pass(
                egui::RawInput {
                    system_theme: system,
                    ..Default::default()
                },
                |_| {},
            );
            frame.textures_delta.clear();
            let mut config = Config {
                theme: saved,
                ..Config::default()
            };
            initialize(&ctx, &mut config);
            assert_eq!(config.theme, Some(expected));
            let pinned = ctx.theme();
            let mut frame = ctx.run_pass(
                egui::RawInput {
                    system_theme: Some(egui::Theme::Light),
                    ..Default::default()
                },
                |_| {},
            );
            frame.textures_delta.clear();
            assert_eq!(ctx.theme(), pinned);
            let mut frame = ctx.run_pass(
                egui::RawInput {
                    system_theme: Some(egui::Theme::Dark),
                    ..Default::default()
                },
                |_| {},
            );
            frame.textures_delta.clear();
            assert_eq!(ctx.theme(), pinned);
        }
    }
}
