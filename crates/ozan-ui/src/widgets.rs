//! Synth-style controls drawn with egui: envelope graph, level meter and an
//! on-screen piano keyboard.

use egui::{self, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

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

/// A rectangle shaded from `top` to `bottom`.
fn shade(painter: &egui::Painter, rect: Rect, top: Color32, bottom: Color32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), top);
    mesh.colored_vertex(rect.left_bottom(), bottom);
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 3, 2);
    painter.add(mesh);
}

/// Soft light spreading from `center` over `radius`, brightest in the middle.
fn glow(painter: &egui::Painter, center: Pos2, radius: Vec2, color: Color32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(center, color);
    let n = 32;
    for i in 0..=n {
        let a = std::f32::consts::PI * (1.0 + i as f32 / n as f32);
        mesh.colored_vertex(center + Vec2::new(a.cos() * radius.x, a.sin() * radius.y), Color32::TRANSPARENT);
    }
    for i in 0..n as u32 {
        mesh.add_triangle(0, i + 1, i + 2);
    }
    painter.add(mesh);
}

/// Analogue VU meter in its case: metal bezel, recessed backlit face, scale
/// -20..+3 VU with a red zone, needle with its shadow, pivot housing, glass and a
/// clip light. `level` is the needle's (smoothed) amplitude, `peak` the recent
/// peak, both linear (1 = full scale).
pub fn vu_meter(ui: &mut egui::Ui, level: f32, peak: f32, size: Vec2) {
    let th = theme::t();
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter_at(rect);

    // Bezel: lit from above, with a bright upper edge and a dark lower one.
    painter.rect_filled(rect, 9.0, th.track);
    shade(&painter, rect.shrink2(Vec2::new(4.0, 3.0)), Color32::from_white_alpha(34), Color32::from_black_alpha(70));
    painter.rect_stroke(rect, 9.0, Stroke::new(1.0, Color32::from_white_alpha(40)), StrokeKind::Inside);
    painter.line_segment([rect.left_bottom() + Vec2::new(8.0, -1.0), rect.right_bottom() + Vec2::new(-8.0, -1.0)], Stroke::new(1.5, Color32::from_black_alpha(140)));
    // The well the face sits in.
    let well = rect.shrink(6.0);
    painter.rect_filled(well, 6.0, Color32::from_rgb(0x0c, 0x0b, 0x0a));
    let face = well.shrink(4.0);
    // Face, a little darker at the top, lit warm from behind the housing.
    painter.rect_filled(face, 4.0, th.vu_face);
    shade(&painter, face.shrink2(Vec2::new(0.0, 3.0)), Color32::from_black_alpha(40), Color32::TRANSPARENT);
    glow(&painter, Pos2::new(face.center().x, face.bottom()), Vec2::new(face.width() * 0.65, face.height() * 1.1), Color32::from_rgba_unmultiplied(255, 190, 90, 70));

    // Needle travel is proportional to voltage, as on a real VU meter.
    let full = 10f32.powf(3.0 / 20.0);
    let pos = |vu_db: f32| (10f32.powf(vu_db / 20.0) / full).clamp(0.0, 1.0);
    let pivot = Pos2::new(face.center().x, face.bottom() + face.height() * 0.28);
    let radius = face.height() * 0.88;
    let half = 46f32.to_radians();
    let point = |p: f32, r: f32| {
        let a = -half + 2.0 * half * p - std::f32::consts::FRAC_PI_2;
        pivot + Vec2::new(a.cos(), a.sin()) * r
    };
    let arc = |from: f32, to: f32, r: f32| -> Vec<Pos2> { (0..=30).map(|i| point(from + (to - from) * i as f32 / 30.0, r)).collect() };
    painter.add(egui::Shape::line(arc(0.0, pos(0.0), radius), Stroke::new(1.2, th.vu_ink)));
    painter.add(egui::Shape::line(arc(pos(0.0), 1.0, radius), Stroke::new(1.2, th.hot)));
    painter.add(egui::Shape::line(arc(pos(0.0), 1.0, radius - 3.0), Stroke::new(5.0, th.hot)));
    for (db, label) in [(-20.0, "20"), (-10.0, "10"), (-7.0, "7"), (-5.0, "5"), (-3.0, "3"), (-2.0, ""), (-1.0, ""), (0.0, "0"), (1.0, ""), (2.0, ""), (3.0, "+3")] {
        let p = pos(db);
        let color = if db > 0.0 { th.hot } else { th.vu_ink };
        let long = !label.is_empty();
        painter.line_segment([point(p, radius), point(p, radius + if long { 6.0 } else { 4.0 })], Stroke::new(if long { 1.4 } else { 1.0 }, color));
        if long {
            painter.text(point(p, radius + 13.0), egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(10.0), color);
        }
    }
    painter.text(Pos2::new(face.center().x, face.bottom() - face.height() * 0.36), egui::Align2::CENTER_CENTER, "VU", egui::FontId::proportional(14.0), th.vu_ink);

    // Needle with its shadow on the face.
    let db = 20.0 * level.max(1e-6).log10() - VU_REFERENCE_DB;
    let p = pos(db);
    let tip = point(p, radius + 4.0);
    let base = point(p, face.height() * 0.3);
    painter.line_segment([base + Vec2::new(3.0, 2.5), tip + Vec2::new(3.0, 2.5)], Stroke::new(2.0, Color32::from_black_alpha(45)));
    painter.line_segment([base, tip], Stroke::new(1.6, Color32::from_rgb(0x15, 0x12, 0x10)));
    // Housing over the pivot.
    let housing = Rect::from_center_size(Pos2::new(face.center().x, face.bottom() - 6.0), Vec2::new(face.width() * 0.3, 16.0));
    painter.rect_filled(housing, 4.0, Color32::from_rgb(0x1c, 0x1a, 0x18));
    painter.line_segment([housing.left_top() + Vec2::new(4.0, 1.0), housing.right_top() + Vec2::new(-4.0, 1.0)], Stroke::new(1.0, Color32::from_white_alpha(40)));

    // Clip light with a glow when lit and a glint on its lens.
    let clip = peak > 0.89;
    let led = Pos2::new(face.right() - 10.0, face.top() + 10.0);
    if clip {
        painter.circle_filled(led, 8.0, th.hot.gamma_multiply(0.25));
    }
    painter.circle_filled(led, 4.0, if clip { th.hot } else { th.hot.gamma_multiply(0.25) });
    painter.circle_filled(led + Vec2::new(-1.2, -1.2), 1.2, Color32::from_white_alpha(150));

    // Glass: a slanted reflection across the top and a shadow under the rim.
    let mut mesh = egui::Mesh::default();
    let g = Color32::from_white_alpha(38);
    mesh.colored_vertex(face.left_top(), g);
    mesh.colored_vertex(face.right_top(), g);
    mesh.colored_vertex(Pos2::new(face.right(), face.top() + face.height() * 0.18), Color32::TRANSPARENT);
    mesh.colored_vertex(Pos2::new(face.left(), face.top() + face.height() * 0.42), Color32::TRANSPARENT);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(mesh);
    painter.rect_stroke(face, 4.0, Stroke::new(2.0, Color32::from_black_alpha(90)), StrokeKind::Inside);
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

/// One wavetable frame drawn on a small screen.
pub fn wave_display(ui: &mut egui::Ui, frame: &[f32], size: Vec2) {
    let th = theme::t();
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, th.screen);
    let r = rect.shrink2(Vec2::new(4.0, 5.0));
    painter.line_segment([Pos2::new(r.left(), r.center().y), Pos2::new(r.right(), r.center().y)], Stroke::new(0.5, th.track));
    let n = 96;
    let points: Vec<Pos2> = (0..=n)
        .map(|i| {
            let idx = (i * (frame.len() - 1)) / n;
            Pos2::new(r.left() + i as f32 / n as f32 * r.width(), r.center().y - frame[idx] * r.height() * 0.5)
        })
        .collect();
    painter.add(egui::Shape::line(points, Stroke::new(1.6, th.accent)));
}

/// The last couple of seconds of `value` (-1..1) scrolling across a small screen.
pub fn lcd_trace(ui: &mut egui::Ui, id: &str, value: f32, size: Vec2) {
    let th = theme::t();
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let id = ui.id().with(id);
    const LEN: usize = 150;
    let mut history = ui.data_mut(|d| d.get_temp::<Vec<f32>>(id)).unwrap_or_default();
    history.push(value.clamp(-1.0, 1.0));
    if history.len() > LEN {
        history.drain(..history.len() - LEN);
    }
    let painter = ui.painter_at(rect.expand(2.0));
    crate::lcd::glass(&painter, rect);
    let inner = rect.shrink2(Vec2::new(6.0, 8.0));
    painter.line_segment([inner.left_center(), inner.right_center()], Stroke::new(1.0, th.lcd_ink.gamma_multiply(0.15)));
    let step = inner.width() / (LEN - 1) as f32;
    let start = LEN - history.len();
    let points: Vec<Pos2> = history
        .iter()
        .enumerate()
        .map(|(i, v)| Pos2::new(inner.left() + (start + i) as f32 * step, inner.center().y - v * inner.height() * 0.5))
        .collect();
    if let Some(&last) = points.last() {
        painter.add(egui::Shape::line(points, Stroke::new(1.8, th.lcd_ink)));
        painter.circle_filled(last, 3.0, th.lcd_ink);
    }
    ui.data_mut(|d| d.insert_temp(id, history));
}

/// A pitch or mod wheel in its slot. `value` is -1..1 when `spring` (pitch: it
/// returns to the middle when let go), else 0..1. Drag up and down. Returns true
/// when the value changed.
pub fn wheel(ui: &mut egui::Ui, label: &str, value: &mut f32, spring: bool, size: Vec2) -> bool {
    let th = theme::t();
    let (rect, response) = ui.allocate_exact_size(size, Sense::drag());
    let slot = Rect::from_min_size(rect.min, Vec2::new(size.x, size.y - 18.0));
    let before = *value;
    let (lo, hi) = if spring { (-1.0, 1.0) } else { (0.0, 1.0) };
    if response.dragged() {
        *value = (*value - response.drag_delta().y / (slot.height() * 0.45) * (hi - lo) / 2.0).clamp(lo, hi);
    }
    if spring && response.drag_stopped() {
        *value = 0.0;
    }
    let painter = ui.painter_at(rect);
    painter.rect_filled(slot, 6.0, th.screen);
    painter.rect_stroke(slot, 6.0, Stroke::new(1.0, th.track), StrokeKind::Inside);
    let body = slot.shrink(5.0);
    painter.rect_filled(body, 4.0, th.skirt);
    // Ridges that roll with the wheel, darker towards the ends (the curve of the drum).
    let roll = *value * 40.0;
    let pitch = 9.0;
    let mut y = body.top() + roll.rem_euclid(pitch);
    while y < body.bottom() {
        let t = ((y - body.center().y) / (body.height() / 2.0)).abs();
        painter.line_segment([Pos2::new(body.left(), y), Pos2::new(body.right(), y)], Stroke::new(1.6, th.bg.gamma_multiply(0.5 + 0.5 * t)));
        y += pitch;
    }
    for (band, alpha) in [(0.0, 120u8), (0.12, 60)] {
        let h = body.height() * (0.1 + band);
        painter.rect_filled(Rect::from_min_size(body.min, Vec2::new(body.width(), h * 0.5)), 0.0, Color32::from_black_alpha(alpha));
        painter.rect_filled(Rect::from_min_max(Pos2::new(body.left(), body.bottom() - h * 0.5), body.max), 0.0, Color32::from_black_alpha(alpha));
    }
    // Position mark.
    let travel = body.height() * 0.42;
    let my = if spring { body.center().y - *value * travel } else { body.center().y + travel - *value * 2.0 * travel };
    let mark = if spring { th.pointer } else { th.accent };
    painter.rect_filled(Rect::from_center_size(Pos2::new(body.center().x, my), Vec2::new(body.width(), 4.0)), 1.0, mark);
    let active = response.hovered() || response.dragged();
    painter.text(
        Pos2::new(rect.center().x, rect.bottom() - 7.0),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(11.0),
        if active { th.accent } else { th.muted },
    );
    *value != before
}
