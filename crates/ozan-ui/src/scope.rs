//! Oscilloscope and spectrum of the output.
//!
//! The audio thread pushes samples into a [`ScopeTap`] (lock-free, a ring of atomics);
//! the window takes a snapshot each frame and draws either the waveform, started on a
//! rising zero crossing so it stands still, or the spectrum on a log frequency scale.

use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use egui::{self, Color32, Pos2, Sense, Shape, Stroke, Vec2};
use ozan_core::fft::{fft, Complex};

use crate::theme;

/// Samples kept: enough for a 2048-point spectrum.
pub const LEN: usize = 4096;
const FFT_LEN: usize = 2048;
/// Samples shown as a waveform (about 20 ms at 48 kHz).
const SHOWN: usize = 1024;

pub struct ScopeTap {
    ring: Box<[AtomicU32]>,
    pos: AtomicUsize,
    sample_rate: AtomicU32,
}

impl Default for ScopeTap {
    fn default() -> Self {
        Self { ring: (0..LEN).map(|_| AtomicU32::new(0)).collect(), pos: AtomicUsize::new(0), sample_rate: AtomicU32::new(48_000) }
    }
}

impl ScopeTap {
    /// Audio thread: one sample (mono).
    #[inline]
    pub fn push(&self, v: f32) {
        let p = self.pos.load(Ordering::Relaxed);
        self.ring[p % LEN].store(v.to_bits(), Ordering::Relaxed);
        self.pos.store(p.wrapping_add(1), Ordering::Release);
    }

    pub fn set_sample_rate(&self, rate: f32) {
        self.sample_rate.store(rate as u32, Ordering::Relaxed);
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate.load(Ordering::Relaxed) as f32
    }

    /// The last [`LEN`] samples, oldest first.
    pub fn snapshot(&self, out: &mut Vec<f32>) {
        let end = self.pos.load(Ordering::Acquire);
        out.clear();
        out.extend((0..LEN).map(|i| f32::from_bits(self.ring[end.wrapping_add(i) % LEN].load(Ordering::Relaxed))));
    }
}

/// Start of the shown window: the last rising zero crossing that still leaves
/// [`SHOWN`] samples after it, so a steady tone draws in the same place each frame.
pub fn trigger(x: &[f32]) -> usize {
    let latest = x.len().saturating_sub(SHOWN);
    (1..=latest).rev().take(latest.min(2048)).find(|&i| x[i - 1] < 0.0 && x[i] >= 0.0).unwrap_or(latest)
}

/// Level in dB for each FFT bin up to half the sample rate (Hann window), for the
/// last [`FFT_LEN`] samples.
pub fn spectrum_db(x: &[f32]) -> Vec<f32> {
    let tail = &x[x.len().saturating_sub(FFT_LEN)..];
    let n = tail.len().next_power_of_two().max(2);
    let mut buf: Vec<Complex> = (0..n)
        .map(|i| {
            let v = tail.get(i).copied().unwrap_or(0.0) as f64;
            let w = 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / n as f64).cos();
            Complex::new(v * w, 0.0)
        })
        .collect();
    fft(&mut buf, false);
    // A full-scale sine reads 0 dB (the Hann window halves the amplitude).
    buf[..n / 2].iter().map(|c| (20.0 * (c.norm() * 4.0 / n as f64).max(1e-9).log10()) as f32).collect()
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Wave,
    Spectrum,
}

/// The scope screen; `mode` is kept by the caller.
pub fn draw(ui: &mut egui::Ui, x: &[f32], sample_rate: f32, mode: Mode, size: Vec2) {
    let th = theme::t();
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 5.0, th.screen);
    let r = rect.shrink2(Vec2::new(6.0, 6.0));
    let grid = Stroke::new(0.5, th.track);
    let glow = th.accent;
    match mode {
        Mode::Wave => {
            for k in 1..4 {
                let y = r.top() + r.height() * k as f32 / 4.0;
                painter.line_segment([Pos2::new(r.left(), y), Pos2::new(r.right(), y)], grid);
            }
            for k in 1..8 {
                let xg = r.left() + r.width() * k as f32 / 8.0;
                painter.line_segment([Pos2::new(xg, r.top()), Pos2::new(xg, r.bottom())], grid);
            }
            if x.len() < SHOWN {
                return;
            }
            let start = trigger(x);
            let shown = &x[start..start + SHOWN];
            let peak = shown.iter().fold(0.0f32, |m, v| m.max(v.abs()));
            // Quiet signals are scaled up (up to ×8) so the shape stays readable.
            let gain = if peak > 1e-4 { (0.9 / peak).min(8.0) } else { 1.0 };
            let points: Vec<Pos2> = shown
                .iter()
                .enumerate()
                .step_by(2)
                .map(|(i, v)| Pos2::new(r.left() + i as f32 / SHOWN as f32 * r.width(), r.center().y - (v * gain).clamp(-1.0, 1.0) * r.height() * 0.5))
                .collect();
            painter.add(Shape::line(points.clone(), Stroke::new(3.0, glow.gamma_multiply(0.25))));
            painter.add(Shape::line(points, Stroke::new(1.4, glow)));
        }
        Mode::Spectrum => {
            let (lo, hi) = (20.0f32.log10(), (sample_rate / 2.0).min(20_000.0).log10());
            let fx = |f: f32| r.left() + (f.max(20.0).log10() - lo) / (hi - lo) * r.width();
            for f in [100.0, 1000.0, 10_000.0] {
                let xg = fx(f);
                painter.line_segment([Pos2::new(xg, r.top()), Pos2::new(xg, r.bottom())], grid);
                let label = if f >= 1000.0 { format!("{}k", f / 1000.0) } else { format!("{f}") };
                painter.text(Pos2::new(xg + 3.0, r.bottom() - 2.0), egui::Align2::LEFT_BOTTOM, label, egui::FontId::proportional(10.0), th.muted);
            }
            if x.len() < FFT_LEN {
                return;
            }
            let db = spectrum_db(x);
            let bin_hz = sample_rate / (2.0 * db.len() as f32);
            // -90..0 dB, one point per screen pixel column (loudest bin in it).
            let columns = r.width().max(1.0) as usize;
            let mut points = Vec::with_capacity(columns + 2);
            points.push(Pos2::new(r.left(), r.bottom()));
            for c in 0..columns {
                let f0 = 10f32.powf(lo + (hi - lo) * c as f32 / columns as f32);
                let f1 = 10f32.powf(lo + (hi - lo) * (c + 1) as f32 / columns as f32);
                let (b0, b1) = ((f0 / bin_hz) as usize, ((f1 / bin_hz) as usize).max((f0 / bin_hz) as usize + 1));
                let level = db[b0.min(db.len() - 1)..b1.min(db.len())].iter().fold(-120.0f32, |m, v| m.max(*v));
                let t = ((level + 90.0) / 90.0).clamp(0.0, 1.0);
                points.push(Pos2::new(r.left() + c as f32, r.bottom() - t * r.height()));
            }
            points.push(Pos2::new(r.right(), r.bottom()));
            // egui fills only convex polygons correctly, so the area under the curve
            // is one thin vertical line per pixel column.
            let curve = &points[1..points.len() - 1];
            let shade = Stroke::new(1.0, glow.gamma_multiply(0.18));
            for p in curve {
                painter.line_segment([*p, Pos2::new(p.x, r.bottom())], shade);
            }
            painter.add(Shape::line(curve.to_vec(), Stroke::new(1.3, glow)));
        }
    }
    painter.rect_stroke(rect, 5.0, Stroke::new(1.0, Color32::from_black_alpha(80)), egui::StrokeKind::Inside);
}

/// Wave / spectrum buttons over the screen; the choice is kept in egui's memory.
pub fn section(ui: &mut egui::Ui, x: &[f32], sample_rate: f32, size: Vec2) {
    let id = ui.id().with("scope_mode");
    let mut mode = ui.data(|d| d.get_temp::<Mode>(id)).unwrap_or_default();
    ui.horizontal(|ui| {
        for (m, label) in [(Mode::Wave, crate::i18n::s().wave), (Mode::Spectrum, crate::i18n::s().spectrum)] {
            if ui.selectable_label(mode == m, label).clicked() {
                mode = m;
            }
        }
    });
    ui.data_mut(|d| d.insert_temp(id, mode));
    draw(ui, x, sample_rate, mode, size);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq: f32, n: usize, sr: f32) -> Vec<f32> {
        (0..n).map(|i| (std::f32::consts::TAU * freq * i as f32 / sr).sin()).collect()
    }

    #[test]
    fn tap_keeps_order() {
        let tap = ScopeTap::default();
        for i in 0..(LEN + 10) {
            tap.push(i as f32);
        }
        let mut out = Vec::new();
        tap.snapshot(&mut out);
        assert_eq!(out.len(), LEN);
        assert_eq!(out[0], 10.0);
        assert_eq!(out[LEN - 1], (LEN + 9) as f32);
    }

    #[test]
    fn trigger_lands_on_a_rising_zero_crossing() {
        let x = sine(440.0, LEN, 48_000.0);
        let t = trigger(&x);
        assert!(t <= LEN - SHOWN);
        assert!(x[t - 1] < 0.0 && x[t] >= 0.0);
    }

    #[test]
    fn spectrum_peaks_at_the_tone() {
        let sr = 48_000.0;
        let x = sine(1000.0, LEN, sr);
        let db = spectrum_db(&x);
        let (peak_bin, peak) = db.iter().enumerate().fold((0, f32::MIN), |m, (i, v)| if *v > m.1 { (i, *v) } else { m });
        let hz = peak_bin as f32 * sr / FFT_LEN as f32;
        assert!((hz - 1000.0).abs() < 30.0, "{hz}");
        assert!(peak.abs() < 1.5, "full-scale sine should read about 0 dB: {peak}");
    }
}
