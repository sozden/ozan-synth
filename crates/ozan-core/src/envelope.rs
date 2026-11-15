//! ADSR envelope with linear segments.

/// Times in seconds, sustain as a level 0..1.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AdsrParams {
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
}

impl Default for AdsrParams {
    fn default() -> Self {
        Self { attack: 0.01, decay: 0.3, sustain: 0.7, release: 0.4 }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Stage {
    #[default]
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

#[derive(Clone, Debug, Default)]
pub struct Adsr {
    stage: Stage,
    level: f32,
    release_step: f32,
}

/// Number of samples a segment of `seconds` lasts, at least one.
fn samples(seconds: f32, sample_rate: f32) -> f32 {
    (seconds * sample_rate).max(1.0)
}

impl Adsr {
    /// Starts (or restarts) the attack from the current level, so a retriggered
    /// voice does not click.
    pub fn gate_on(&mut self) {
        self.stage = Stage::Attack;
    }

    /// Starts the release; it takes `release` seconds from whatever level it is at.
    pub fn gate_off(&mut self, p: &AdsrParams, sample_rate: f32) {
        if self.stage != Stage::Idle {
            self.stage = Stage::Release;
            self.release_step = self.level / samples(p.release, sample_rate);
        }
    }

    /// Ends the note at once (used before a voice is reused).
    pub fn kill(&mut self) {
        self.stage = Stage::Idle;
        self.level = 0.0;
    }

    pub fn stage(&self) -> Stage {
        self.stage
    }

    pub fn is_active(&self) -> bool {
        self.stage != Stage::Idle
    }

    pub fn level(&self) -> f32 {
        self.level
    }

    #[inline]
    pub fn next(&mut self, p: &AdsrParams, sample_rate: f32) -> f32 {
        match self.stage {
            Stage::Idle => self.level = 0.0,
            Stage::Attack => {
                self.level += 1.0 / samples(p.attack, sample_rate);
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.stage = Stage::Decay;
                }
            }
            Stage::Decay => {
                self.level -= (1.0 - p.sustain) / samples(p.decay, sample_rate);
                if self.level <= p.sustain {
                    self.level = p.sustain;
                    self.stage = Stage::Sustain;
                }
            }
            // Follows the knob while the key is held.
            Stage::Sustain => self.level = p.sustain,
            Stage::Release => {
                self.level -= self.release_step;
                if self.level <= 0.0 {
                    self.level = 0.0;
                    self.stage = Stage::Idle;
                }
            }
        }
        self.level
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 1000.0;

    fn run(env: &mut Adsr, p: &AdsrParams, n: usize) -> f32 {
        (0..n).map(|_| env.next(p, SR)).last().unwrap_or(env.level())
    }

    #[test]
    fn full_cycle() {
        let p = AdsrParams { attack: 0.1, decay: 0.1, sustain: 0.5, release: 0.2 };
        let mut env = Adsr::default();
        assert!(!env.is_active());
        env.gate_on();
        assert!((run(&mut env, &p, 100) - 1.0).abs() < 1e-3);
        // A sample or two of rounding slack at the end of the decay.
        assert!((run(&mut env, &p, 103) - 0.5).abs() < 1e-3);
        assert_eq!(env.stage(), Stage::Sustain);
        env.gate_off(&p, SR);
        run(&mut env, &p, 150);
        assert!(env.is_active());
        run(&mut env, &p, 51);
        assert!(!env.is_active());
        assert_eq!(env.level(), 0.0);
    }

    #[test]
    fn release_during_attack_takes_release_time() {
        let p = AdsrParams { attack: 1.0, decay: 0.1, sustain: 0.5, release: 0.1 };
        let mut env = Adsr::default();
        env.gate_on();
        run(&mut env, &p, 300);
        env.gate_off(&p, SR);
        run(&mut env, &p, 101);
        assert!(!env.is_active());
    }

    #[test]
    fn retrigger_starts_from_current_level() {
        let p = AdsrParams { attack: 0.1, decay: 0.1, sustain: 0.5, release: 1.0 };
        let mut env = Adsr::default();
        env.gate_on();
        run(&mut env, &p, 300);
        env.gate_off(&p, SR);
        run(&mut env, &p, 100);
        let before = env.level();
        env.gate_on();
        assert!(env.next(&p, SR) > before);
    }
}
