//! Rotary knob and button rows for choosing waveform and filter type.

use std::f32::consts::PI;

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use ozan_core::{FilterMode, LfoWave, VoiceMode, Waveform};

use crate::i18n::s;
use crate::theme::{self, KnobStyle};

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
        let th = theme::t();
        let active = response.hovered() || response.dragged();
        let a = start + sweep * norm;
        let dir = Vec2::new(a.cos(), a.sin());
        match th.knob_style {
            KnobStyle::Modern => {
                painter.add(egui::Shape::line(arc(start, start + sweep), Stroke::new(4.0, th.track)));
                let from = if self.bipolar { 0.5 } else { 0.0 };
                if (norm - from).abs() > 0.001 {
                    painter.add(egui::Shape::line(arc(start + sweep * from, start + sweep * norm), Stroke::new(4.0, th.accent)));
                }
                painter.circle_filled(center, radius - 6.0, if active { th.knob_hover } else { th.knob });
                painter.line_segment([center + dir * 5.0, center + dir * (radius - 8.0)], Stroke::new(2.5, th.pointer));
            }
            KnobStyle::Skirted => {
                // Printed scale: 11 ticks, the ends and the middle longer.
                for i in 0..=10 {
                    let ta = start + sweep * i as f32 / 10.0;
                    let td = Vec2::new(ta.cos(), ta.sin());
                    let long = i == 0 || i == 10 || (self.bipolar && i == 5);
                    let outer = radius + if long { 1.5 } else { 0.0 };
                    painter.line_segment([center + td * (radius - 3.0), center + td * outer], Stroke::new(if long { 1.6 } else { 1.0 }, th.text));
                }
                // Skirt with a white line, then the black cap with a highlight ring.
                painter.circle_filled(center, radius - 4.0, th.skirt);
                painter.line_segment([center + dir * (radius - 11.0), center + dir * (radius - 4.5)], Stroke::new(2.5, th.pointer));
                painter.circle_filled(center, radius - 11.0, if active { th.knob_hover } else { th.knob });
                painter.circle_stroke(center, radius - 11.0, Stroke::new(1.0, Color32::from_white_alpha(28)));
                painter.line_segment([center + dir * 3.0, center + dir * (radius - 12.5)], Stroke::new(1.6, th.pointer.gamma_multiply(0.8)));
            }
        }
        painter.text(Pos2::new(center.x, rect.top() + 62.0), egui::Align2::CENTER_CENTER, self.label, egui::FontId::proportional(13.0), th.text);
        painter.text(
            Pos2::new(center.x, rect.top() + 80.0),
            egui::Align2::CENTER_CENTER,
            (self.format)(*self.value),
            egui::FontId::monospace(11.0),
            if active { th.accent } else { th.muted },
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
        Waveform::Wavetable => {
            // Three waves stacked in depth, like frames of a table.
            for (j, a) in [(0, 0.35f32), (1, 0.6), (2, 1.0)] {
                let o = Vec2::new(-3.0 + 3.0 * j as f32, 3.0 - 3.0 * j as f32);
                let pts: Vec<Pos2> = (0..=24)
                    .map(|i| i as f32 / 24.0)
                    .map(|t| p(t, (t * 2.0 * PI).sin() * (1.0 - 0.3 * j as f32) + 0.25 * (t * 6.0 * PI).sin() * j as f32 / 2.0) + o)
                    .collect();
                painter.add(egui::Shape::line(pts, Stroke::new(1.3, color.gamma_multiply(a))));
            }
            return;
        }
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
            let fill = if selected { theme::t().accent_dark } else if response.hovered() { theme::t().knob_hover } else { theme::t().knob };
            painter.rect_filled(rect, 4.0, fill);
            paint(&painter, rect, option, if selected { theme::t().text } else { theme::t().muted });
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
        &[Waveform::Sine, Waveform::Saw, Waveform::Square, Waveform::Triangle, Waveform::Wavetable],
        Vec2::new(36.0, 26.0),
        |p, r, w, c| paint_wave(p, r.shrink2(Vec2::new(9.0, 7.0)), w, c),
        |w| match w {
            Waveform::Sine => s().sine,
            Waveform::Saw => s().saw,
            Waveform::Square => s().square,
            Waveform::Triangle => s().triangle,
            Waveform::Wavetable => s().wavetable,
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
            FilterMode::Lp12 => s().lp12,
            FilterMode::Lp24 => s().lp24,
            FilterMode::BandPass => s().band_pass,
            FilterMode::HighPass => s().high_pass,
        },
    )
}

pub fn lfo_wave_selector(ui: &mut egui::Ui, wave: &mut LfoWave) -> bool {
    choice_row(
        ui,
        wave,
        &LfoWave::ALL,
        Vec2::new(34.0, 26.0),
        |painter, r, w, color| {
            let r = r.shrink2(Vec2::new(8.0, 7.0));
            let p = |t: f32, v: f32| Pos2::new(r.left() + t * r.width(), r.center().y - v * r.height() * 0.5);
            let points: Vec<Pos2> = match w {
                LfoWave::Sine => return paint_wave(painter, r, Waveform::Sine, color),
                LfoWave::Triangle => return paint_wave(painter, r, Waveform::Triangle, color),
                LfoWave::Square => return paint_wave(painter, r, Waveform::Square, color),
                LfoWave::Saw => vec![p(0.0, -1.0), p(1.0, 1.0), p(1.0, -1.0)],
                LfoWave::SampleHold => vec![
                    p(0.0, 0.2), p(0.25, 0.2), p(0.25, -0.8), p(0.5, -0.8), p(0.5, 1.0), p(0.75, 1.0), p(0.75, -0.3), p(1.0, -0.3),
                ],
            };
            painter.add(egui::Shape::line(points, Stroke::new(1.8, color)));
        },
        |w| match w {
            LfoWave::Sine => s().sine,
            LfoWave::Triangle => s().triangle,
            LfoWave::Saw => s().ramp,
            LfoWave::Square => s().square,
            LfoWave::SampleHold => s().sample_hold,
        },
    )
}

pub fn voice_mode_selector(ui: &mut egui::Ui, mode: &mut VoiceMode) -> bool {
    fn label(m: VoiceMode) -> &'static str {
        match m {
            VoiceMode::Poly => s().poly,
            VoiceMode::Mono => s().mono,
            VoiceMode::Legato => s().legato,
        }
    }
    choice_row(
        ui,
        mode,
        &VoiceMode::ALL,
        Vec2::new(54.0, 26.0),
        |p, r, m, c| {
            p.text(r.center(), egui::Align2::CENTER_CENTER, label(m), egui::FontId::proportional(12.0), c);
        },
        |m| match m {
            VoiceMode::Poly => s().poly_hint,
            VoiceMode::Mono => s().mono_hint,
            VoiceMode::Legato => s().legato_hint,
        },
    )
}
