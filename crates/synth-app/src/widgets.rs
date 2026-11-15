//! Synth-style controls drawn with egui: envelope graph, level meter and an
//! on-screen piano keyboard.

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

use crate::theme;

/// Shape of the ADSR envelope, with a fixed-width sustain segment.
pub fn envelope_graph(ui: &mut egui::Ui, attack: f32, decay: f32, sustain: f32, release: f32, size: Vec2) {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 6.0, theme::SCREEN);
    let r = rect.shrink(8.0);
    let hold = 0.35;
    let total = attack + decay + release + hold;
    let x = |t: f32| r.left() + t / total * r.width();
    let y = |l: f32| r.bottom() - l * r.height();
    let points = vec![
        Pos2::new(x(0.0), y(0.0)),
        Pos2::new(x(attack), y(1.0)),
        Pos2::new(x(attack + decay), y(sustain)),
        Pos2::new(x(attack + decay + hold), y(sustain)),
        Pos2::new(x(total), y(0.0)),
    ];
    let mut fill = points.clone();
    fill.push(Pos2::new(x(0.0), y(0.0)));
    painter.add(egui::Shape::convex_polygon(fill, theme::ACCENT.gamma_multiply(0.12), Stroke::NONE));
    painter.add(egui::Shape::line(points, Stroke::new(2.0, theme::ACCENT)));
}

/// Vertical level meter; `level` 0..1 (linear), drawn on a dB scale.
pub fn level_meter(ui: &mut egui::Ui, level: f32, size: Vec2) {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 3.0, theme::SCREEN);
    let db = 20.0 * level.max(1e-6).log10();
    let t = ((db + 48.0) / 48.0).clamp(0.0, 1.0);
    let bar = Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - t * rect.height()), rect.max);
    let color = if db > -1.0 { theme::HOT } else if db > -9.0 { theme::WARM } else { theme::ACCENT };
    painter.rect_filled(bar, 3.0, color);
}

fn is_black(note: u8) -> bool {
    matches!(note % 12, 1 | 3 | 6 | 8 | 10)
}

/// Piano keys from `low` to `high` (both should be white keys). Keys in `lit` are
/// drawn pressed. Mouse presses go to `mouse_note`; returns (note-off, note-on with
/// velocity) changes caused by the mouse.
pub fn piano(ui: &mut egui::Ui, low: u8, high: u8, lit: u128, mouse_note: &mut Option<u8>, height: f32) -> Vec<(u8, Option<u8>)> {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click_and_drag());
    let whites: Vec<u8> = (low..=high).filter(|&n| !is_black(n)).collect();
    let key_w = rect.width() / whites.len() as f32;
    let black_w = key_w * 0.6;
    let black_h = height * 0.62;

    let white_rect = |i: usize| Rect::from_min_size(Pos2::new(rect.left() + i as f32 * key_w, rect.top()), Vec2::new(key_w, height));
    // Black keys sit on the boundary after the white key below them.
    let blacks: Vec<(u8, Rect)> = whites
        .iter()
        .enumerate()
        .filter(|&(_, &n)| n < high && is_black(n + 1))
        .map(|(i, &n)| {
            let x = rect.left() + (i + 1) as f32 * key_w - black_w / 2.0;
            (n + 1, Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(black_w, black_h)))
        })
        .collect();

    let lit_note = |n: u8| lit & (1u128 << n) != 0 || *mouse_note == Some(n);
    let painter = ui.painter_at(rect);
    for (i, &n) in whites.iter().enumerate() {
        let r = white_rect(i).shrink2(Vec2::new(1.0, 0.0));
        painter.rect_filled(r, egui::CornerRadius { nw: 0, ne: 0, sw: 4, se: 4 }, if lit_note(n) { theme::ACCENT } else { theme::WHITE_KEY });
        if n % 12 == 0 {
            painter.text(
                Pos2::new(r.center().x, r.bottom() - 10.0),
                egui::Align2::CENTER_CENTER,
                synth_core::note_name(n),
                egui::FontId::proportional(10.0),
                Color32::from_gray(110),
            );
        }
    }
    for &(n, r) in &blacks {
        painter.rect_filled(r, egui::CornerRadius { nw: 0, ne: 0, sw: 3, se: 3 }, if lit_note(n) { theme::ACCENT_DARK } else { theme::BLACK_KEY });
    }
    painter.rect_stroke(rect, 4.0, Stroke::new(1.0, theme::BORDER), StrokeKind::Inside);

    // Mouse: the key under the pointer while the button is held; lower on the key
    // is louder, as on a real keyboard.
    let mut changes = Vec::new();
    let down = response.is_pointer_button_down_on();
    let target = if down {
        response.interact_pointer_pos().filter(|p| rect.contains(*p)).and_then(|p| {
            blacks
                .iter()
                .find(|(_, r)| r.contains(p))
                .map(|&(n, r)| (n, (p.y - r.top()) / r.height()))
                .or_else(|| {
                    let i = ((p.x - rect.left()) / key_w) as usize;
                    whites.get(i).map(|&n| (n, (p.y - rect.top()) / height))
                })
        })
    } else {
        None
    };
    let new_note = target.map(|(n, _)| n);
    if new_note != *mouse_note {
        if let Some(old) = mouse_note.take() {
            changes.push((old, None));
        }
        if let Some((n, depth)) = target {
            let velocity = (40.0 + depth.clamp(0.0, 1.0) * 87.0) as u8;
            changes.push((n, Some(velocity)));
            *mouse_note = Some(n);
        }
    }
    changes
}

/// A small round indicator light.
pub fn led(ui: &mut egui::Ui, color: Color32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), 5.0, color);
    response
}

