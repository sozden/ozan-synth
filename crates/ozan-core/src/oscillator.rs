//! Band-limited oscillators.
//!
//! A naive saw or square jumps between two values in one sample; the jump contains
//! harmonics above half the sample rate, which fold back as inharmonic "aliasing"
//! tones. PolyBLEP smooths each jump over the two samples around it, and PolyBLAMP
//! does the same for the corners of the triangle.

use std::f32::consts::TAU;

use crate::wavetable::Wavetable;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Waveform {
    Sine,
    #[default]
    Saw,
    Square,
    Triangle,
    /// Reads the oscillator's wavetable (see [`crate::wavetable`]).
    Wavetable,
}

impl Waveform {
    /// The classic shapes computed directly (all but [`Waveform::Wavetable`]).
    pub const ALL: [Waveform; 4] = [Waveform::Sine, Waveform::Saw, Waveform::Square, Waveform::Triangle];
}

/// Correction for a downward step of size 2 at phase 0; `t` is the phase (0..1),
/// `dt` the phase increment per sample.
#[inline]
fn poly_blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let t = t / dt;
        2.0 * t - t * t - 1.0
    } else if t > 1.0 - dt {
        let t = (t - 1.0) / dt;
        t * t + 2.0 * t + 1.0
    } else {
        0.0
    }
}

/// Integral of [`poly_blep`]: correction for a corner (change of slope) at phase 0,
/// per unit of slope change per sample.
#[inline]
fn poly_blamp(t: f32, dt: f32) -> f32 {
    if t < dt {
        let t = t / dt - 1.0;
        -t * t * t / 3.0
    } else if t > 1.0 - dt {
        let t = (t - 1.0) / dt + 1.0;
        t * t * t / 3.0
    } else {
        0.0
    }
}

#[inline]
fn wrap(t: f32) -> f32 {
    t - t.floor()
}

/// Oscillator with a phase accumulator in cycles (0..1).
#[derive(Clone, Debug, Default)]
pub struct Oscillator {
    phase: f32,
}

impl Oscillator {
    pub fn reset(&mut self, phase: f32) {
        self.phase = wrap(phase);
    }

    /// Next sample (about -1..1) of `wave` at `freq` Hz.
    #[inline]
    pub fn next(&mut self, wave: Waveform, freq: f32, sample_rate: f32) -> f32 {
        let dt = (freq / sample_rate).clamp(0.0, 0.5);
        let t = self.phase;
        let out = match wave {
            Waveform::Sine => (t * TAU).sin(),
            Waveform::Saw => 2.0 * t - 1.0 - poly_blep(t, dt),
            Waveform::Square => {
                let naive = if t < 0.5 { 1.0 } else { -1.0 };
                naive + poly_blep(t, dt) - poly_blep(wrap(t + 0.5), dt)
            }
            Waveform::Triangle => {
                // +1 at phase 0, -1 at 0.5; the slope (per cycle) turns from -4 to +4
                // at 0.5 and back at 0.
                let naive = 4.0 * (t - 0.5).abs() - 1.0;
                naive + 8.0 * dt * (poly_blamp(wrap(t + 0.5), dt) - poly_blamp(t, dt))
            }
            // Played through `next_table`.
            Waveform::Wavetable => 0.0,
        };
        self.phase = wrap(t + dt);
        out
    }

    /// Like [`Oscillator::next`], read `offset` cycles ahead of the phase (phase
    /// modulation, heard as FM). The phase itself still moves at `freq`.
    #[inline]
    pub fn next_pm(&mut self, wave: Waveform, freq: f32, sample_rate: f32, offset: f32) -> f32 {
        let saved = self.phase;
        self.phase = wrap(saved + offset);
        let out = self.next(wave, freq, sample_rate);
        self.phase = wrap(saved + (freq / sample_rate).clamp(0.0, 0.5));
        out
    }

    /// [`Oscillator::next_table`] read `offset` cycles ahead.
    #[inline]
    pub fn next_table_pm(&mut self, table: &Wavetable, position: f32, level: usize, dt: f32, offset: f32) -> f32 {
        let out = table.sample(wrap(self.phase + offset), position, level);
        self.phase = wrap(self.phase + dt);
        out
    }

    /// Next sample from a wavetable; `level` comes from [`Wavetable::level_for`].
    #[inline]
    pub fn next_table(&mut self, table: &Wavetable, position: f32, level: usize, dt: f32) -> f32 {
        let out = table.sample(self.phase, position, level);
        self.phase = wrap(self.phase + dt);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    fn render(wave: Waveform, freq: f32, n: usize) -> Vec<f32> {
        let mut osc = Oscillator::default();
        (0..n).map(|_| osc.next(wave, freq, SR)).collect()
    }

    fn naive(wave: Waveform, freq: f32, n: usize) -> Vec<f32> {
        let mut t = 0.0f32;
        (0..n)
            .map(|_| {
                let v = match wave {
                    Waveform::Saw => 2.0 * t - 1.0,
                    Waveform::Square => if t < 0.5 { 1.0 } else { -1.0 },
                    Waveform::Triangle => 4.0 * (t - 0.5).abs() - 1.0,
                    Waveform::Sine => (t * TAU).sin(),
                    Waveform::Wavetable => 0.0,
                };
                t = wrap(t + freq / SR);
                v
            })
            .collect()
    }

    /// Share of the signal's power that is not at a harmonic of `f0`. With 4800
    /// samples the bins are 10 Hz apart; `f0` must be a multiple of 10 Hz.
    fn alias_share(x: &[f32], f0: f32) -> f64 {
        let n = x.len();
        let step = (f0 / (SR / n as f32)).round() as usize;
        let (mut harmonic, mut total) = (0.0f64, 0.0f64);
        for k in 1..n / 2 {
            let w = std::f64::consts::TAU * k as f64 / n as f64;
            let (mut re, mut im) = (0.0f64, 0.0f64);
            for (i, &v) in x.iter().enumerate() {
                let a = w * i as f64;
                re += v as f64 * a.cos();
                im -= v as f64 * a.sin();
            }
            let p = re * re + im * im;
            total += p;
            if k % step == 0 {
                harmonic += p;
            }
        }
        1.0 - harmonic / total
    }

    #[test]
    fn frequency_from_zero_crossings() {
        for wave in Waveform::ALL {
            let x = render(wave, 440.0, 48_000);
            let rising = x.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
            assert!((439..=441).contains(&rising), "{wave:?}: {rising}");
        }
    }

    #[test]
    fn bounded_and_no_dc() {
        for wave in Waveform::ALL {
            let x = render(wave, 1234.0, 48_000);
            let peak = x.iter().fold(0.0f32, |m, v| m.max(v.abs()));
            let mean = x.iter().sum::<f32>() / x.len() as f32;
            assert!(peak <= 1.2, "{wave:?} peak {peak}");
            assert!(mean.abs() < 0.01, "{wave:?} mean {mean}");
        }
    }

    #[test]
    fn wavetable_plays_in_tune_without_aliasing() {
        use crate::wavetable::{Wavetable, WavetableBank, WavetableKind};
        let bank = WavetableBank::new();
        // The end of the basic table is a square; compare with the naive square.
        let table = bank.get(WavetableKind::Basic);
        let play = |freq: f32, n: usize| {
            let mut osc = Oscillator::default();
            let level = Wavetable::level_for(freq / SR);
            (0..n).map(|_| osc.next_table(table, 1.0, level, freq / SR)).collect::<Vec<f32>>()
        };
        let x = play(440.0, 48_000);
        let rising = x.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
        assert!((439..=441).contains(&rising), "{rising}");
        let f0 = 2_830.0;
        let ours = alias_share(&play(f0, 4800), f0);
        let theirs = alias_share(&naive(Waveform::Square, f0, 4800), f0);
        assert!(ours < theirs * 0.1, "{ours:.5} vs naive {theirs:.5}");
    }

    #[test]
    fn less_aliasing_than_naive() {
        // A high note, where aliasing is easy to hear.
        let f0 = 2_830.0;
        for wave in [Waveform::Saw, Waveform::Square, Waveform::Triangle] {
            let ours = alias_share(&render(wave, f0, 4800), f0);
            let theirs = alias_share(&naive(wave, f0, 4800), f0);
            assert!(ours < theirs * 0.5, "{wave:?}: {ours:.5} vs naive {theirs:.5}");
        }
    }
}
