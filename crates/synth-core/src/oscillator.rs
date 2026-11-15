//! Oscillators. Stage 1 has a sine only; band-limited saw, square and triangle
//! (PolyBLEP) come in stage 2.

use std::f32::consts::TAU;

/// Sine oscillator with a phase accumulator in cycles (0..1).
#[derive(Clone, Debug, Default)]
pub struct SineOsc {
    phase: f32,
}

impl SineOsc {
    pub fn reset(&mut self) {
        self.phase = 0.0;
    }

    /// Next sample for `freq` Hz at `sample_rate`.
    #[inline]
    pub fn next(&mut self, freq: f32, sample_rate: f32) -> f32 {
        let out = (self.phase * TAU).sin();
        self.phase += freq / sample_rate;
        self.phase -= self.phase.floor();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frequency_from_zero_crossings() {
        let sr = 48_000.0;
        let mut osc = SineOsc::default();
        let samples: Vec<f32> = (0..48_000).map(|_| osc.next(440.0, sr)).collect();
        let rising = samples.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
        assert!((439..=441).contains(&rising), "{rising}");
        assert!(samples.iter().all(|s| s.abs() <= 1.0));
    }
}
