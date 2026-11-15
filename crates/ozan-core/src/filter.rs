//! Resonant filter: a "zero-delay feedback" state-variable filter (after Andrew
//! Simper, Cytomic). It stays stable while its cutoff is swept quickly, which an
//! envelope does to it all the time.

use std::f32::consts::PI;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FilterMode {
    /// Low-pass, 12 dB per octave.
    Lp12,
    /// Low-pass, 24 dB per octave (two stages).
    #[default]
    Lp24,
    BandPass,
    HighPass,
    /// Cuts a band around the cutoff, passes the rest (phaser-like when swept).
    Notch,
}

impl FilterMode {
    pub const ALL: [FilterMode; 5] = [FilterMode::Lp12, FilterMode::Lp24, FilterMode::BandPass, FilterMode::HighPass, FilterMode::Notch];
}

/// Soft saturation before the filter: `amount` 0 leaves the signal untouched, 1
/// drives it about ten times into a tanh curve. Full scale stays full scale.
#[inline]
pub fn drive(x: f32, amount: f32) -> f32 {
    if amount <= 0.0 {
        return x;
    }
    let g = 1.0 + 9.0 * amount * amount;
    (g * x).tanh() / g.tanh()
}

/// Removes the offset that saturating an uneven wave (saw, pulse) creates: a
/// one-pole high-pass at 10 Hz.
#[derive(Clone, Debug, Default)]
pub struct DcBlock {
    x1: f32,
    y1: f32,
}

impl DcBlock {
    #[inline]
    pub fn process(&mut self, x: f32, sample_rate: f32) -> f32 {
        let r = 1.0 - std::f32::consts::TAU * 10.0 / sample_rate;
        let y = x - self.x1 + r * self.y1;
        self.x1 = x;
        self.y1 = y;
        y
    }
}

/// One 12 dB state-variable stage.
#[derive(Clone, Debug, Default)]
struct Svf {
    ic1: f32,
    ic2: f32,
}

struct Outputs {
    low: f32,
    band: f32,
    high: f32,
}

impl Svf {
    /// `g` = tan(π·fc/fs), `k` = 1/Q.
    #[inline]
    fn process(&mut self, v0: f32, g: f32, k: f32) -> Outputs {
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;
        let v3 = v0 - self.ic2;
        let v1 = a1 * self.ic1 + a2 * v3;
        let v2 = self.ic2 + a2 * self.ic1 + a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        Outputs { low: v2, band: v1, high: v0 - k * v1 - v2 }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Filter {
    a: Svf,
    b: Svf,
}

impl Filter {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Filters one sample. `cutoff` in Hz, `resonance` 0..1 (near 1 it rings and
    /// whistles at the cutoff).
    #[inline]
    pub fn process(&mut self, x: f32, mode: FilterMode, cutoff: f32, resonance: f32, sample_rate: f32) -> f32 {
        let (g, k) = Self::coefficients(cutoff, resonance, sample_rate);
        self.process_with(x, mode, g, k)
    }

    /// `(g, k)` for [`Filter::process_with`]; worth sharing between the two filters
    /// of a stereo voice.
    #[inline]
    pub fn coefficients(cutoff: f32, resonance: f32, sample_rate: f32) -> (f32, f32) {
        let fc = cutoff.clamp(20.0, sample_rate * 0.45);
        ((PI * fc / sample_rate).tan(), 2.0 - 1.95 * resonance.clamp(0.0, 1.0))
    }

    #[inline]
    pub fn process_with(&mut self, x: f32, mode: FilterMode, g: f32, k: f32) -> f32 {
        match mode {
            FilterMode::Lp12 => self.a.process(x, g, k).low,
            FilterMode::Lp24 => {
                // First stage without resonance (Butterworth), second with it.
                let first = self.a.process(x, g, std::f32::consts::SQRT_2).low;
                self.b.process(first, g, k).low
            }
            FilterMode::BandPass => self.a.process(x, g, k).band,
            FilterMode::HighPass => self.a.process(x, g, k).high,
            FilterMode::Notch => {
                let o = self.a.process(x, g, k);
                o.low + o.high
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    /// Output level (RMS ratio) of a sine at `freq` after settling.
    fn gain(mode: FilterMode, cutoff: f32, resonance: f32, freq: f32) -> f32 {
        let mut f = Filter::default();
        let n = 48_000;
        let (mut sin, mut sout) = (0.0f32, 0.0f32);
        for i in 0..n {
            let x = (std::f32::consts::TAU * freq * i as f32 / SR).sin();
            let y = f.process(x, mode, cutoff, resonance, SR);
            if i > n / 2 {
                sin += x * x;
                sout += y * y;
            }
        }
        (sout / sin).sqrt()
    }

    #[test]
    fn low_pass_passes_lows_and_cuts_highs() {
        assert!((gain(FilterMode::Lp12, 1000.0, 0.0, 100.0) - 1.0).abs() < 0.05);
        assert!(gain(FilterMode::Lp12, 1000.0, 0.0, 8000.0) < 0.03);
        // Two stages cut much more.
        assert!(gain(FilterMode::Lp24, 1000.0, 0.0, 8000.0) < 0.001);
    }

    #[test]
    fn high_pass_and_band_pass() {
        assert!(gain(FilterMode::HighPass, 1000.0, 0.0, 100.0) < 0.03);
        assert!((gain(FilterMode::HighPass, 1000.0, 0.0, 10_000.0) - 1.0).abs() < 0.05);
        let centre = gain(FilterMode::BandPass, 1000.0, 0.5, 1000.0);
        assert!(centre > gain(FilterMode::BandPass, 1000.0, 0.5, 100.0) * 5.0);
        assert!(centre > gain(FilterMode::BandPass, 1000.0, 0.5, 10_000.0) * 5.0);
    }

    #[test]
    fn notch_cuts_the_cutoff_and_passes_the_rest() {
        assert!(gain(FilterMode::Notch, 1000.0, 0.0, 1000.0) < 0.05);
        assert!((gain(FilterMode::Notch, 1000.0, 0.0, 100.0) - 1.0).abs() < 0.05);
        assert!((gain(FilterMode::Notch, 1000.0, 0.0, 10_000.0) - 1.0).abs() < 0.05);
    }

    #[test]
    fn drive_is_transparent_at_zero_and_adds_odd_harmonics() {
        let n = 4800;
        let sine: Vec<f32> = (0..n).map(|i| (std::f32::consts::TAU * 1000.0 * i as f32 / SR).sin() * 0.8).collect();
        assert!(sine.iter().all(|&x| drive(x, 0.0) == x));
        // Third harmonic (3 kHz) energy, by correlation over whole cycles.
        let third = |y: &[f32]| {
            let (mut re, mut im) = (0.0f32, 0.0f32);
            for (i, v) in y.iter().enumerate() {
                let a = std::f32::consts::TAU * 3000.0 * i as f32 / SR;
                re += v * a.cos();
                im += v * a.sin();
            }
            (re * re + im * im).sqrt() / y.len() as f32
        };
        let clean = third(&sine);
        let driven: Vec<f32> = sine.iter().map(|&x| drive(x, 1.0)).collect();
        assert!(third(&driven) > 100.0 * clean.max(1e-6), "{} vs {}", third(&driven), clean);
        assert!(driven.iter().all(|v| v.abs() <= 1.0));
        assert!((drive(1.0, 0.7) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn resonance_boosts_the_cutoff() {
        assert!(gain(FilterMode::Lp24, 1000.0, 0.9, 1000.0) > 3.0 * gain(FilterMode::Lp24, 1000.0, 0.0, 1000.0));
    }

    #[test]
    fn stable_under_fast_sweeps_at_full_resonance() {
        let mut f = Filter::default();
        let mut seed = 1u32;
        for i in 0..200_000 {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let noise = (seed >> 9) as f32 / (1u32 << 23) as f32 - 0.5;
            // Cutoff jumps between 30 Hz and 20 kHz every 50 samples.
            let cutoff = if (i / 50) % 2 == 0 { 30.0 } else { 20_000.0 };
            let y = f.process(noise, FilterMode::Lp24, cutoff, 1.0, SR);
            assert!(y.is_finite() && y.abs() < 100.0, "sample {i}: {y}");
        }
    }
}
