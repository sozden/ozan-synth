//! The plugin's automatable parameters are relative to the chosen preset (an offset
//! or a factor), so a preset keeps its character and automation stays meaningful
//! when the preset changes. At their defaults they leave the preset untouched.

use ozan_core::Patch;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Macros {
    /// Level change, dB.
    pub volume_db: f32,
    /// Cutoff shift, octaves.
    pub cutoff_oct: f32,
    /// Added to resonance.
    pub resonance: f32,
    /// Added to the filter envelope amount.
    pub env_amount: f32,
    /// Factors on both envelopes' attack and release times.
    pub attack: f32,
    pub release: f32,
    /// Factor on the LFO rate.
    pub lfo_rate: f32,
    /// Added to both oscillators' wavetable position.
    pub position: f32,
    /// Added to the unison detune, cents.
    pub detune: f32,
    /// Added to the glide time, seconds.
    pub glide: f32,
    /// Factors on the effects' mix levels.
    pub chorus: f32,
    pub delay: f32,
    pub reverb: f32,
}

impl Default for Macros {
    fn default() -> Self {
        Self {
            volume_db: 0.0,
            cutoff_oct: 0.0,
            resonance: 0.0,
            env_amount: 0.0,
            attack: 1.0,
            release: 1.0,
            lfo_rate: 1.0,
            position: 0.0,
            detune: 0.0,
            glide: 0.0,
            chorus: 1.0,
            delay: 1.0,
            reverb: 1.0,
        }
    }
}

/// The preset with the macros applied.
pub fn apply(base: &Patch, m: &Macros) -> Patch {
    let mut p = *base;
    p.volume = (p.volume * 10f32.powf(m.volume_db / 20.0)).clamp(0.0, 1.0);
    p.filter.cutoff = (p.filter.cutoff * m.cutoff_oct.exp2()).clamp(20.0, 20_000.0);
    p.filter.resonance = (p.filter.resonance + m.resonance).clamp(0.0, 1.0);
    p.filter.env_amount = (p.filter.env_amount + m.env_amount).clamp(-1.0, 1.0);
    for env in [&mut p.amp_env, &mut p.filter_env] {
        env.attack = (env.attack * m.attack).clamp(0.0005, 10.0);
        env.release = (env.release * m.release).clamp(0.002, 15.0);
    }
    p.lfo.rate = (p.lfo.rate * m.lfo_rate).clamp(0.01, 40.0);
    for osc in &mut p.osc {
        osc.position = (osc.position + m.position).clamp(0.0, 1.0);
    }
    p.unison.detune = (p.unison.detune + m.detune).clamp(0.0, 100.0);
    p.play.glide = (p.play.glide + m.glide).clamp(0.0, 3.0);
    p.fx.chorus.mix = (p.fx.chorus.mix * m.chorus).clamp(0.0, 1.0);
    p.fx.delay.mix = (p.fx.delay.mix * m.delay).clamp(0.0, 1.0);
    p.fx.reverb.mix = (p.fx.reverb.mix * m.reverb).clamp(0.0, 1.0);
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_leave_every_preset_untouched() {
        for (name, patch) in ozan_core::presets::factory() {
            assert_eq!(apply(&patch, &Macros::default()), patch, "{name}");
        }
    }

    #[test]
    fn macros_move_the_preset_and_stay_in_range() {
        let base = Patch::default();
        let p = apply(&base, &Macros { cutoff_oct: 1.0, volume_db: -6.0, attack: 10.0, reverb: 2.0, ..Macros::default() });
        assert!((p.filter.cutoff - 2.0 * base.filter.cutoff).abs() < 1e-3);
        assert!((p.volume - base.volume * 0.501).abs() < 1e-3);
        assert!((p.amp_env.attack - base.amp_env.attack * 10.0).abs() < 1e-6);
        let extreme = apply(&base, &Macros { cutoff_oct: 10.0, resonance: 5.0, volume_db: 40.0, position: 9.0, ..Macros::default() });
        assert_eq!(extreme.filter.cutoff, 20_000.0);
        assert_eq!(extreme.filter.resonance, 1.0);
        assert_eq!(extreme.volume, 1.0);
        assert_eq!(extreme.osc[0].position, 1.0);
    }
}
