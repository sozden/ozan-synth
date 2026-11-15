//! Synth-style controls drawn with egui: envelope graph, level meter and an
//! on-screen piano keyboard.

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

use crate::theme;

/// Shape of the ADSR envelope, with a fixed-width sustain segment.
pub fn envelope_graph(ui: &mut egui::Ui, attack: f32, decay: f32, sustain: f32, release: f32, size: Vec2) {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 6.0, theme::t().screen);
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
    painter.add(egui::Shape::convex_polygon(fill, theme::t().accent.gamma_multiply(0.12), Stroke::NONE));
    painter.add(egui::Shape::line(points, Stroke::new(2.0, theme::t().accent)));
}

/// Output level that reads 0 VU, in dB below full scale.
const VU_REFERENCE_DB: f32 = -10.0;

/// Analogue VU meter: cream face, scale -20..+3 VU with a red zone, needle and a
/// clip light. `level` is the needle's (smoothed) amplitude, `peak` the recent peak,
/// both linear (1 = full scale).
pub fn vu_meter(ui: &mut egui::Ui, level: f32, peak: f32, size: Vec2) {
    let th = theme::t();
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 5.0, th.screen);
    let face = rect.shrink(3.0);
    painter.rect_filled(face, 3.0, th.vu_face);

    // Needle travel is proportional to voltage, as on a real VU meter.
    let full = 10f32.powf(3.0 / 20.0);
    let pos = |vu_db: f32| (10f32.powf(vu_db / 20.0) / full).clamp(0.0, 1.0);
    let pivot = Pos2::new(face.center().x, face.bottom() + face.height() * 0.35);
    let radius = face.height() * 0.95;
    let half = 44f32.to_radians();
    let point = |p: f32, r: f32| {
        let a = -half + 2.0 * half * p - std::f32::consts::FRAC_PI_2;
        pivot + Vec2::new(a.cos(), a.sin()) * r
    };
    let arc = |from: f32, to: f32, r: f32| -> Vec<Pos2> { (0..=30).map(|i| point(from + (to - from) * i as f32 / 30.0, r)).collect() };
    painter.add(egui::Shape::line(arc(0.0, pos(0.0), radius), Stroke::new(1.0, th.vu_ink)));
    painter.add(egui::Shape::line(arc(pos(0.0), 1.0, radius), Stroke::new(1.0, th.hot)));
    painter.add(egui::Shape::line(arc(pos(0.0), 1.0, radius - 2.0), Stroke::new(3.0, th.hot)));
    for (db, label) in [(-20.0, "20"), (-10.0, "10"), (-7.0, "7"), (-5.0, "5"), (-3.0, "3"), (-2.0, ""), (-1.0, ""), (0.0, "0"), (1.0, ""), (2.0, ""), (3.0, "+3")] {
        let p = pos(db);
        let color = if db > 0.0 { th.hot } else { th.vu_ink };
        painter.line_segment([point(p, radius), point(p, radius + 4.0)], Stroke::new(1.0, color));
        if !label.is_empty() {
            painter.text(point(p, radius + 10.0), egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(9.0), color);
        }
    }
    painter.text(
        Pos2::new(face.center().x, face.bottom() - 12.0),
        egui::Align2::CENTER_CENTER,
        "VU",
        egui::FontId::proportional(12.0),
        th.vu_ink,
    );
    // Needle.
    let db = 20.0 * level.max(1e-6).log10() - VU_REFERENCE_DB;
    let tip = point(pos(db), radius + 2.0);
    let base = point(pos(db), face.height() * 0.45);
    painter.line_segment([base, tip], Stroke::new(1.4, Color32::from_rgb(0x15, 0x12, 0x10)));
    // Clip light near full scale.
    let clip = peak > 0.89;
    let led = Pos2::new(face.right() - 9.0, face.top() + 9.0);
    painter.circle_filled(led, 3.5, if clip { th.hot } else { th.hot.gamma_multiply(0.18) });
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
        painter.rect_filled(r, egui::CornerRadius { nw: 0, ne: 0, sw: 4, se: 4 }, if lit_note(n) { theme::t().accent } else { theme::t().white_key });
        if n % 12 == 0 {
            painter.text(
                Pos2::new(r.center().x, r.bottom() - 10.0),
                egui::Align2::CENTER_CENTER,
                ozan_core::note_name(n),
                egui::FontId::proportional(10.0),
                Color32::from_gray(110),
            );
        }
    }
    for &(n, r) in &blacks {
        painter.rect_filled(r, egui::CornerRadius { nw: 0, ne: 0, sw: 3, se: 3 }, if lit_note(n) { theme::t().accent_dark } else { theme::t().black_key });
    }
    painter.rect_stroke(rect, 4.0, Stroke::new(1.0, theme::t().border), StrokeKind::Inside);

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


/// A labelled horizontal bar for a controller value (0..1).
pub fn mini_bar(ui: &mut egui::Ui, label: &str, value: f32) {
    let th = theme::t();
    ui.label(egui::RichText::new(label).size(11.0).color(th.muted));
    let (rect, _) = ui.allocate_exact_size(Vec2::new(56.0, 8.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, th.screen);
    let filled = Rect::from_min_size(rect.min, Vec2::new(rect.width() * value.clamp(0.0, 1.0), rect.height()));
    painter.rect_filled(filled, 2.0, th.accent);
}

/// On/off switch with a label; returns true when flipped.
pub fn toggle(ui: &mut egui::Ui, on: &mut bool, label: &str) -> bool {
    ui.horizontal(|ui| {
        let th = theme::t();
        let (rect, response) = ui.allocate_exact_size(Vec2::new(30.0, 16.0), Sense::click());
        let painter = ui.painter_at(rect);
        let t = ui.ctx().animate_bool(response.id, *on);
        let track = if *on { th.accent_dark } else { th.track };
        painter.rect_filled(rect, 8.0, track);
        let x = egui::lerp(rect.left() + 8.0..=rect.right() - 8.0, t);
        painter.circle_filled(Pos2::new(x, rect.center().y), 6.0, if *on { th.pointer } else { th.muted });
        let text = ui.add(egui::Label::new(egui::RichText::new(label).size(12.0).color(if *on { th.text } else { th.muted })).sense(Sense::click()));
        let clicked = response.clicked() || text.clicked();
        if clicked {
            *on = !*on;
        }
        clicked
    })
    .inner
}
