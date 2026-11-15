//! Low-frequency oscillator, shared by all voices (as on classic polysynths), for
//! vibrato, filter wobble and tremolo.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LfoWave {
    #[default]
    Sine,
    Triangle,
    /// Rising ramp.
    Saw,
    Square,
    /// A new random level once per cycle.
    SampleHold,
}

impl LfoWave {
    pub const ALL: [LfoWave; 5] = [LfoWave::Sine, LfoWave::Triangle, LfoWave::Saw, LfoWave::Square, LfoWave::SampleHold];
}

#[derive(Clone, Debug)]
pub struct Lfo {
    phase: f32,
    held: f32,
    seed: u32,
}

impl Default for Lfo {
    fn default() -> Self {
        Self { phase: 0.0, held: 0.0, seed: 0x1234_5678 }
    }
}

impl Lfo {
    fn random(&mut self) -> f32 {
        // xorshift32: plenty for a modulation source.
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed >> 8) as f32 / (1u32 << 23) as f32 - 1.0
    }

    /// Value (-1..1) at the current phase, then moves on by `seconds` at `rate` Hz.
    pub fn advance(&mut self, wave: LfoWave, rate: f32, seconds: f32) -> f32 {
        let t = self.phase;
        let out = match wave {
            LfoWave::Sine => (t * std::f32::consts::TAU).sin(),
            LfoWave::Triangle => 1.0 - 4.0 * (t - 0.25).rem_euclid(1.0).min(1.0 - (t - 0.25).rem_euclid(1.0)),
            LfoWave::Saw => 2.0 * t - 1.0,
            LfoWave::Square => if t < 0.5 { 1.0 } else { -1.0 },
            LfoWave::SampleHold => self.held,
        };
        self.phase += rate.max(0.0) * seconds;
        if self.phase >= 1.0 {
            self.phase -= self.phase.floor();
            self.held = self.random();
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(wave: LfoWave, rate: f32, n: usize, step: f32) -> Vec<f32> {
        let mut lfo = Lfo::default();
        (0..n).map(|_| lfo.advance(wave, rate, step)).collect()
    }

    #[test]
    fn ranges_and_shapes() {
        for wave in LfoWave::ALL {
            let v = run(wave, 3.0, 10_000, 0.001);
            assert!(v.iter().all(|x| (-1.0..=1.0).contains(x)), "{wave:?}");
        }
        // Triangle and sine start at 0 going up, peak a quarter cycle later.
        let tri = run(LfoWave::Triangle, 1.0, 1000, 0.001);
        assert!(tri[0].abs() < 1e-3 && (tri[250] - 1.0).abs() < 1e-2 && (tri[750] + 1.0).abs() < 1e-2);
    }

    #[test]
    fn rate_in_hertz() {
        // 5 Hz over 2 seconds: 10 rising zero crossings of the sine.
        let v = run(LfoWave::Sine, 5.0, 2000, 0.001);
        let rising = v.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
        assert!((9..=10).contains(&rising), "{rising}");
    }

    #[test]
    fn sample_hold_changes_once_per_cycle() {
        let v = run(LfoWave::SampleHold, 10.0, 1000, 0.001);
        let changes = v.windows(2).filter(|w| w[0] != w[1]).count();
        assert!((9..=10).contains(&changes), "{changes}");
    }
}
