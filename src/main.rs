#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

pub mod app;
pub mod checker;
pub mod config;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title(env!("CARGO_PKG_NAME"))
            .with_inner_size([1100.0, 720.0])
            .with_min_inner_size([700.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native(
        env!("CARGO_PKG_NAME"),
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
