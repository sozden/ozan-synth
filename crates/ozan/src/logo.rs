//! The OZAN Synth logo, drawn from geometry (no font for the letters): monoline
//! capitals on a 100-unit cap height, O a circle, Z a saw tooth in the accent colour,
//! A an open peak (Λ, an envelope's attack), N. `assets/make_logo.py` writes the SVG
//! files from the same proportions.

use eframe::egui::{self, Color32, Pos2, Sense, Shape, Stroke, Vec2};

const STROKE: f32 = 14.0;
const GAP: f32 = 26.0;
const O: f32 = 100.0;
const Z: f32 = 76.0;
const A: f32 = 84.0;
const N: f32 = 76.0;

/// Width of the word in cap-height units (cap height = 100).
const WORD: f32 = O + Z + A + N + 3.0 * GAP;

/// Polyline with round ends and joins.
fn round_line(painter: &egui::Painter, points: &[Pos2], width: f32, color: Color32) {
    painter.add(Shape::line(points.to_vec(), Stroke::new(width, color)));
    for p in points {
        painter.circle_filled(*p, width / 2.0, color);
    }
}

/// Paints "OZAN" with its top-left corner at `origin`, capitals `h` high.
pub fn paint_word(painter: &egui::Painter, origin: Pos2, h: f32, text: Color32, accent: Color32) {
    let k = h / 100.0;
    let w = STROKE * k;
    let half = w / 2.0;
    let top = origin.y + half;
    let bottom = origin.y + h - half;
    let mut x = origin.x;

    painter.circle_stroke(Pos2::new(x + O * k / 2.0, origin.y + h / 2.0), O * k / 2.0 - half, Stroke::new(w, text));
    x += (O + GAP) * k;

    let (l, r) = (x + half, x + Z * k - half);
    round_line(painter, &[Pos2::new(l, top), Pos2::new(r, top), Pos2::new(l, bottom), Pos2::new(r, bottom)], w, accent);
    x += (Z + GAP) * k;

    let (l, r) = (x + half, x + A * k - half);
    round_line(painter, &[Pos2::new(l, bottom), Pos2::new((l + r) / 2.0, top), Pos2::new(r, bottom)], w, text);
    x += (A + GAP) * k;

    let (l, r) = (x + half, x + N * k - half);
    round_line(painter, &[Pos2::new(l, bottom), Pos2::new(l, top), Pos2::new(r, bottom), Pos2::new(r, top)], w, text);
}

/// The header lockup: OZAN in capitals `h` high, then SYNTH small and letter-spaced.
pub fn lockup(ui: &mut egui::Ui, h: f32, text: Color32, accent: Color32, muted: Color32) -> egui::Response {
    let k = h / 100.0;
    let word = Vec2::new(WORD * k + STROKE * k, h + STROKE * k);
    let label = ui.painter().layout_no_wrap(
        "S Y N T H".to_string(),
        egui::FontId::proportional((h * 0.42).max(9.0)),
        muted,
    );
    let size = Vec2::new(word.x + h * 0.45 + label.size().x, word.y);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    paint_word(ui.painter(), rect.min + Vec2::splat(STROKE * k / 2.0), h, text, accent);
    // SYNTH sits on the baseline of the capitals.
    let pos = Pos2::new(rect.min.x + word.x + h * 0.45, rect.min.y + STROKE * k / 2.0 + h - label.size().y * 0.85);
    ui.painter().galley(pos, label, muted);
    response
}

/// Window and taskbar icon (`assets/icon.png`).
pub fn icon() -> egui::IconData {
    eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")).unwrap_or_default()
}
