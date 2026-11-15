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
}

impl FilterMode {
    pub const ALL: [FilterMode; 4] = [FilterMode::Lp12, FilterMode::Lp24, FilterMode::BandPass, FilterMode::HighPass];
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
        let fc = cutoff.clamp(20.0, sample_rate * 0.45);
        let g = (PI * fc / sample_rate).tan();
        let k = 2.0 - 1.95 * resonance.clamp(0.0, 1.0);
        match mode {
            FilterMode::Lp12 => self.a.process(x, g, k).low,
            FilterMode::Lp24 => {
                // First stage without resonance (Butterworth), second with it.
                let first = self.a.process(x, g, std::f32::consts::SQRT_2).low;
                self.b.process(first, g, k).low
            }
            FilterMode::BandPass => self.a.process(x, g, k).band,
            FilterMode::HighPass => self.a.process(x, g, k).high,
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
