#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

pub mod app;
pub mod checker;
pub mod config;

pub const APP_NAME: &str = "PingChecker";

const INITIAL_WINDOW_SIZE: [f32; 2] = [900.0, 600.0];

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title(APP_NAME)
            .with_inner_size(INITIAL_WINDOW_SIZE)
            .with_min_inner_size([700.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native(
        APP_NAME,
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
