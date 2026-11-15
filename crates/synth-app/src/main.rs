//! Synth - an open-source software synthesizer, built up step by step.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod audio;
mod knob;
mod midi_in;
mod settings;
mod shared;
mod theme;
mod widgets;

use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Synth")
            .with_inner_size([1180.0, 610.0])
            .with_min_inner_size([1120.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native("Synth", options, Box::new(|cc| Ok(Box::new(app::SynthApp::new(cc)))))
}
