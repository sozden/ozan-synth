//! OZAN Synth - an open-source software synthesizer, built up step by step.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod audio;
mod midi_in;
mod shared;

use eframe::egui;

/// Window and taskbar icon.
fn icon() -> egui::IconData {
    eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")).unwrap_or_default()
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("OZAN Synth")
            .with_app_id("ozan")
            .with_icon(icon())
            .with_inner_size([1660.0, 956.0])
            .with_min_inner_size([1560.0, 700.0]),
        ..Default::default()
    };
    ozan_ui::settings::migrate_old_folder();
    eframe::run_native("OZAN Synth", options, Box::new(|cc| Ok(Box::new(app::OzanApp::new(cc)))))
}
