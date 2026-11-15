//! Rotary knob and button rows for choosing waveform and filter type.

use std::f32::consts::PI;

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use synth_core::{FilterMode, Waveform};

use crate::theme;

/// How a knob maps its 0..1 travel to a value.
#[derive(Clone, Copy)]
enum Curve {
    Linear,
    /// Equal travel for equal ratios; for times and frequencies. `min` must be > 0.
    Log,
}

/// A rotary control: drag up/down to turn (Shift for fine steps), mouse wheel,
/// double-click for the default value.
pub struct Knob<'a> {
    label: &'a str,
    value: &'a mut f32,
    min: f32,
    max: f32,
    default: f32,
    curve: Curve,
    /// Rounds to whole numbers (octaves, semitones).
    integer: bool,
    /// The arc starts from the middle (for amounts that go both ways).
    bipolar: bool,
    format: fn(f32) -> String,
}

fn plain(v: f32) -> String {
    format!("{v:.2}")
}

impl<'a> Knob<'a> {
    pub fn new(label: &'a str, value: &'a mut f32, min: f32, max: f32) -> Self {
        let default = *value;
        Self { label, value, min, max, default, curve: Curve::Linear, integer: false, bipolar: false, format: plain }
    }

    pub fn default_value(mut self, v: f32) -> Self {
        self.default = v;
        self
    }

    pub fn log(mut self) -> Self {
        self.curve = Curve::Log;
        self
    }

    pub fn integer(mut self) -> Self {
        self.integer = true;
        self
    }

    pub fn bipolar(mut self) -> Self {
        self.bipolar = true;
        self
    }

    pub fn format(mut self, f: fn(f32) -> String) -> Self {
        self.format = f;
        self
    }

    fn value_to_norm(&self, v: f32) -> f32 {
        let t = match self.curve {
            Curve::Linear => (v - self.min) / (self.max - self.min),
            Curve::Log => (v / self.min).ln() / (self.max / self.min).ln(),
        };
        t.clamp(0.0, 1.0)
    }

    fn norm_to_value(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        let v = match self.curve {
            Curve::Linear => self.min + t * (self.max - self.min),
            Curve::Log => self.min * (self.max / self.min).powf(t),
        };
        if self.integer { v.round() } else { v }
    }

    /// Returns true when the value changed.
    pub fn show(self, ui: &mut egui::Ui) -> bool {
        let (rect, response) = ui.allocate_exact_size(Vec2::new(60.0, 92.0), Sense::click_and_drag());
        let id = response.id;
        let before = *self.value;
        if response.dragged() {
            // The unrounded position is kept while dragging, or an integer knob
            // would never reach the next step with small moves.
            let start = ui.data(|d| d.get_temp::<f32>(id)).unwrap_or_else(|| self.value_to_norm(*self.value));
            let fine = ui.input(|i| i.modifiers.shift);
            let norm = (start - response.drag_delta().y / if fine { 800.0 } else { 160.0 }).clamp(0.0, 1.0);
            ui.data_mut(|d| d.insert_temp(id, norm));
            *self.value = self.norm_to_value(norm);
        } else {
            ui.data_mut(|d| d.remove::<f32>(id));
        }
        if response.double_clicked() {
            *self.value = self.default;
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                let step = if self.integer { scroll.signum() / (self.max - self.min) } else { scroll / 600.0 };
                *self.value = self.norm_to_value(self.value_to_norm(*self.value) + step);
            }
        }
        let norm = self.value_to_norm(*self.value);

        let painter = ui.painter_at(rect);
        let center = Pos2::new(rect.center().x, rect.top() + 28.0);
        let radius = 21.0;
        let start = 0.75 * PI;
        let sweep = 1.5 * PI;
        let arc = |from: f32, to: f32| -> Vec<Pos2> {
            (0..=40)
                .map(|i| {
                    let a = from + (to - from) * i as f32 / 40.0;
                    center + Vec2::new(a.cos(), a.sin()) * radius
                })
                .collect()
        };
        let active = response.hovered() || response.dragged();
        painter.add(egui::Shape::line(arc(start, start + sweep), Stroke::new(4.0, theme::TRACK)));
        let from = if self.bipolar { 0.5 } else { 0.0 };
        if (norm - from).abs() > 0.001 {
            painter.add(egui::Shape::line(arc(start + sweep * from, start + sweep * norm), Stroke::new(4.0, theme::ACCENT)));
        }
        painter.circle_filled(center, radius - 6.0, if active { theme::KNOB_HOVER } else { theme::KNOB });
        let a = start + sweep * norm;
        let dir = Vec2::new(a.cos(), a.sin());
        painter.line_segment([center + dir * 5.0, center + dir * (radius - 8.0)], Stroke::new(2.5, theme::TEXT));
        painter.text(Pos2::new(center.x, rect.top() + 62.0), egui::Align2::CENTER_CENTER, self.label, egui::FontId::proportional(13.0), theme::TEXT);
        painter.text(
            Pos2::new(center.x, rect.top() + 80.0),
            egui::Align2::CENTER_CENTER,
            (self.format)(*self.value),
            egui::FontId::monospace(11.0),
            if active { theme::ACCENT } else { theme::MUTED },
        );
        *self.value != before
    }
}

/// Small picture of a waveform inside `rect`.
fn paint_wave(painter: &egui::Painter, rect: Rect, wave: Waveform, color: Color32) {
    let y = |v: f32| rect.center().y - v * rect.height() * 0.5;
    let x = |t: f32| rect.left() + t * rect.width();
    let p = |t: f32, v: f32| Pos2::new(x(t), y(v));
    let points: Vec<Pos2> = match wave {
        Waveform::Sine => (0..=32).map(|i| i as f32 / 32.0).map(|t| p(t, (t * 2.0 * PI).sin())).collect(),
        Waveform::Saw => vec![p(0.0, -1.0), p(0.5, 1.0), p(0.5, -1.0), p(1.0, 1.0)],
        Waveform::Square => vec![p(0.0, 1.0), p(0.5, 1.0), p(0.5, -1.0), p(1.0, -1.0)],
        Waveform::Triangle => vec![p(0.0, 0.0), p(0.25, 1.0), p(0.75, -1.0), p(1.0, 0.0)],
    };
    painter.add(egui::Shape::line(points, Stroke::new(1.8, color)));
}

/// A row of toggle buttons; returns true when the choice changed.
fn choice_row<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    value: &mut T,
    options: &[T],
    size: Vec2,
    paint: impl Fn(&egui::Painter, Rect, T, Color32),
    hover: impl Fn(T) -> &'static str,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for &option in options {
            let (rect, response) = ui.allocate_exact_size(size, Sense::click());
            let selected = *value == option;
            let painter = ui.painter_at(rect);
            let fill = if selected { theme::ACCENT_DARK } else if response.hovered() { theme::KNOB_HOVER } else { theme::KNOB };
            painter.rect_filled(rect, 4.0, fill);
            paint(&painter, rect, option, if selected { theme::TEXT } else { theme::MUTED });
            if response.on_hover_text(hover(option)).clicked() && !selected {
                *value = option;
                changed = true;
            }
        }
    });
    changed
}

pub fn wave_selector(ui: &mut egui::Ui, wave: &mut Waveform) -> bool {
    choice_row(
        ui,
        wave,
        &Waveform::ALL,
        Vec2::new(40.0, 26.0),
        |p, r, w, c| paint_wave(p, r.shrink2(Vec2::new(9.0, 7.0)), w, c),
        |w| match w {
            Waveform::Sine => "Sinüs",
            Waveform::Saw => "Testere",
            Waveform::Square => "Kare",
            Waveform::Triangle => "Üçgen",
        },
    )
}

pub fn filter_mode_selector(ui: &mut egui::Ui, mode: &mut FilterMode) -> bool {
    fn short(m: FilterMode) -> &'static str {
        match m {
            FilterMode::Lp12 => "LP12",
            FilterMode::Lp24 => "LP24",
            FilterMode::BandPass => "BP",
            FilterMode::HighPass => "HP",
        }
    }
    choice_row(
        ui,
        mode,
        &FilterMode::ALL,
        Vec2::new(46.0, 26.0),
        |p, r, m, c| {
            p.text(r.center(), egui::Align2::CENTER_CENTER, short(m), egui::FontId::proportional(12.0), c);
        },
        |m| match m {
            FilterMode::Lp12 => "Alçak geçiren, 12 dB/oktav",
            FilterMode::Lp24 => "Alçak geçiren, 24 dB/oktav",
            FilterMode::BandPass => "Bant geçiren",
            FilterMode::HighPass => "Yüksek geçiren",
        },
    )
}
