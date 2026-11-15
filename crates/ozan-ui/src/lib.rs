//! Interface of OZAN Synth shared by the standalone program and the plugin: theme,
//! knobs and meters, logo, translations, settings and the sound panel.

pub mod i18n;
pub mod knob;
pub mod learn;
pub mod logo;
pub mod panel;
pub mod preset_bar;
pub mod presets;
pub mod scope;
pub mod settings;
pub mod theme;
pub mod widgets;

pub use egui;

/// Wooden end cheeks left and right, when the theme has them. Call before the
/// other panels so they run the full height.
pub fn wood_cheeks(ctx: &egui::Context) {
    if !theme::t().wood {
        return;
    }
    for left in [true, false] {
        let panel = if left { egui::SidePanel::left("wood_left") } else { egui::SidePanel::right("wood_right") };
        panel.exact_width(26.0).resizable(false).show_separator_line(false).frame(egui::Frame::new()).show(ctx, |ui| {
            theme::paint_wood(ui.painter(), ui.max_rect());
        });
    }
}

/// Inner margin of the main areas, in the theme's background colour.
pub fn pad() -> egui::Frame {
    egui::Frame::new().fill(theme::t().bg).inner_margin(egui::Margin::same(14))
}
