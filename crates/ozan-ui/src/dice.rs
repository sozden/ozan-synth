//! The dice: a new random sound in one click, or one part of it from the
//! right-click menu. The level is matched to the factory presets.

use std::sync::Arc;

use egui::{self, Sense, Vec2};
use ozan_core::random::{self, Part};
use ozan_core::wavetable::WavetableBank;
use ozan_core::Patch;

use crate::i18n::s;
use crate::theme;

pub fn part_name(part: Part) -> &'static str {
    match part {
        Part::All => s().part_all,
        Part::Oscillators => s().part_osc,
        Part::Filter => s().part_filter,
        Part::Envelopes => s().part_env,
        Part::Modulation => s().part_mod,
        Part::Effects => s().part_fx,
    }
}

/// A fresh seed for each throw.
pub fn seed() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64)
}

/// The die button. Returns true when `patch` was changed.
pub fn button(ui: &mut egui::Ui, patch: &mut Patch, bank: &Arc<WavetableBank>) -> bool {
    let th = theme::t();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(30.0, 24.0), Sense::click());
    let painter = ui.painter_at(rect.expand(2.0));
    let face = egui::Rect::from_center_size(rect.center(), Vec2::splat(21.0));
    painter.rect_filled(face.translate(Vec2::new(1.0, 1.5)), 5.0, egui::Color32::from_black_alpha(90));
    painter.rect_filled(face, 5.0, if response.hovered() { th.accent_dark } else { th.knob });
    painter.rect_stroke(face, 5.0, egui::Stroke::new(1.0, th.track), egui::StrokeKind::Inside);
    // Five pips.
    let c = face.center();
    for (dx, dy) in [(0.0, 0.0), (-5.5, -5.5), (5.5, -5.5), (-5.5, 5.5), (5.5, 5.5)] {
        painter.circle_filled(c + Vec2::new(dx, dy), 2.0, th.text);
    }

    let mut chosen = None;
    let response = response.on_hover_text(s().dice_hint);
    if response.clicked() {
        chosen = Some(Part::All);
    }
    response.context_menu(|ui| {
        ui.label(egui::RichText::new(s().dice_menu).color(th.muted));
        for part in Part::ALL {
            if ui.button(part_name(part)).clicked() {
                chosen = Some(part);
                ui.close_menu();
            }
        }
    });
    let Some(part) = chosen else { return false };
    let mut p = random::randomize(patch, part, seed());
    p.volume = random::balanced_volume(&p, bank.clone());
    *patch = p;
    crate::lcd::touch(s().dice, part_name(part).to_string(), 1.0, false, ui.input(|i| i.time));
    true
}
