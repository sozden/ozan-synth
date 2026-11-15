//! Ozan - an open-source software synthesizer, built up step by step.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod audio;
mod i18n;
mod knob;
mod logo;
mod midi_in;
mod presets;
mod settings;
mod shared;
mod theme;
mod widgets;

use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Ozan")
            .with_app_id("ozan")
            .with_icon(logo::icon())
            .with_inner_size([1180.0, 940.0])
            .with_min_inner_size([1120.0, 700.0]),
        ..Default::default()
    };
    settings::migrate_old_folder();
    eframe::run_native("Ozan", options, Box::new(|cc| Ok(Box::new(app::OzanApp::new(cc)))))
}
