//! The Ozan logo, drawn from geometry (no font): monoline lowercase "ozan" with the
//! "z" as a sharp saw tooth in the accent colour. `assets/logo.svg` and the window
//! icon use the same proportions.

use std::f32::consts::PI;

use eframe::egui::{self, Color32, Pos2, Sense, Shape, Stroke, Vec2};

/// Proportions in units of the letter height.
const STROKE: f32 = 0.17;
const GAP: f32 = 0.28;
const ROUND: f32 = 1.0; // width of o and a
const NARROW: f32 = 0.8; // width of z and n

/// Width of the word for letter height 1.
pub const WIDTH: f32 = ROUND + GAP + NARROW + GAP + ROUND + GAP + NARROW;

/// Polyline with round ends and joins.
fn round_line(painter: &egui::Painter, points: &[Pos2], width: f32, color: Color32) {
    painter.add(Shape::line(points.to_vec(), Stroke::new(width, color)));
    for p in points {
        painter.circle_filled(*p, width / 2.0, color);
    }
}

/// Paints "ozan" with its top-left corner at `origin`, letters `h` high.
pub fn paint_wordmark(painter: &egui::Painter, origin: Pos2, h: f32, text: Color32, accent: Color32) {
    let w = STROKE * h;
    let half = w / 2.0;
    let r = h / 2.0 - half;
    let top = origin.y + half;
    let bottom = origin.y + h - half;
    let mid = origin.y + h / 2.0;
    let mut x = origin.x;

    // o
    painter.circle_stroke(Pos2::new(x + h / 2.0, mid), r, Stroke::new(w, text));
    x += ROUND * h + GAP * h;

    // z: a saw tooth
    let zr = x + NARROW * h - half;
    round_line(
        painter,
        &[Pos2::new(x + half, top), Pos2::new(zr, top), Pos2::new(x + half, bottom), Pos2::new(zr, bottom)],
        w,
        accent,
    );
    x += NARROW * h + GAP * h;

    // a: bowl and stem
    painter.circle_stroke(Pos2::new(x + h / 2.0, mid), r, Stroke::new(w, text));
    let stem = x + ROUND * h - half;
    round_line(painter, &[Pos2::new(stem, top + r * 0.2), Pos2::new(stem, bottom)], w, text);
    x += ROUND * h + GAP * h;

    // n: two stems and an arch
    let left = x + half;
    let right = x + NARROW * h - half;
    let ar = (right - left) / 2.0;
    let cy = top + ar;
    let mut points = vec![Pos2::new(left, bottom), Pos2::new(left, cy)];
    points.extend((1..24).map(|i| {
        let a = PI + PI * i as f32 / 24.0;
        Pos2::new(left + ar + ar * a.cos(), cy + ar * a.sin())
    }));
    points.push(Pos2::new(right, cy));
    points.push(Pos2::new(right, bottom));
    round_line(painter, &points, w, text);
}

/// The wordmark as a widget, letters `h` high.
pub fn wordmark(ui: &mut egui::Ui, h: f32, text: Color32, accent: Color32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(WIDTH * h + STROKE * h, h + STROKE * h), Sense::hover());
    let origin = rect.min + Vec2::splat(STROKE * h / 2.0);
    paint_wordmark(ui.painter(), origin, h, text, accent);
    response
}

/// Window and taskbar icon, made at start-up (`assets/icon.png`).
pub fn icon() -> egui::IconData {
    eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")).unwrap_or_default()
}
