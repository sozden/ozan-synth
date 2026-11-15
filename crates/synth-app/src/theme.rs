//! Colours of the panel.

use eframe::egui::{self, Color32};

pub const BG: Color32 = Color32::from_rgb(0x15, 0x16, 0x1b);
pub const CARD: Color32 = Color32::from_rgb(0x1f, 0x21, 0x29);
pub const SCREEN: Color32 = Color32::from_rgb(0x10, 0x11, 0x15);
pub const BORDER: Color32 = Color32::from_rgb(0x34, 0x37, 0x44);
pub const TRACK: Color32 = Color32::from_rgb(0x34, 0x37, 0x44);
pub const KNOB: Color32 = Color32::from_rgb(0x2c, 0x2f, 0x3a);
pub const KNOB_HOVER: Color32 = Color32::from_rgb(0x38, 0x3c, 0x4a);
pub const TEXT: Color32 = Color32::from_rgb(0xe6, 0xe6, 0xec);
pub const MUTED: Color32 = Color32::from_rgb(0x8c, 0x8f, 0x9c);
pub const ACCENT: Color32 = Color32::from_rgb(0x8b, 0x7c, 0xf6);
pub const ACCENT_DARK: Color32 = Color32::from_rgb(0x5b, 0x4c, 0xc6);
pub const WARM: Color32 = Color32::from_rgb(0xef, 0x9f, 0x27);
pub const HOT: Color32 = Color32::from_rgb(0xe2, 0x4b, 0x4a);
pub const GOOD: Color32 = Color32::from_rgb(0x3d, 0xc9, 0x8a);
pub const WHITE_KEY: Color32 = Color32::from_rgb(0xec, 0xec, 0xf0);
pub const BLACK_KEY: Color32 = Color32::from_rgb(0x1a, 0x1b, 0x20);

pub fn apply(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BG;
    visuals.window_fill = CARD;
    visuals.selection.bg_fill = ACCENT_DARK;
    visuals.widgets.hovered.bg_stroke.color = ACCENT;
    ctx.set_visuals(visuals);
}

/// A titled section of the panel.
pub fn section<R>(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .fill(CARD)
        .stroke(egui::Stroke::new(1.0, BORDER))
        .corner_radius(8.0)
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                ui.label(egui::RichText::new(title).size(12.0).color(MUTED).strong());
                add(ui)
            })
            .inner
        })
        .inner
}
