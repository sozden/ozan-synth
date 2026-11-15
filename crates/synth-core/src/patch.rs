//! All settings of a sound. The interface edits a copy and sends the whole patch to
//! the engine when something changes; later the same struct is saved as a preset.

use crate::envelope::AdsrParams;
use crate::filter::FilterMode;
use crate::oscillator::Waveform;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OscParams {
    pub wave: Waveform,
    /// -3..3 octaves.
    pub octave: i8,
    /// -12..12 semitones.
    pub semitones: i8,
    /// -50..50 cents.
    pub fine: f32,
    /// 0..1.
    pub level: f32,
}

impl OscParams {
    /// Pitch offset in semitones.
    pub fn offset(&self) -> f32 {
        self.octave as f32 * 12.0 + self.semitones as f32 + self.fine / 100.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FilterParams {
    pub mode: FilterMode,
    /// Hz.
    pub cutoff: f32,
    /// 0..1.
    pub resonance: f32,
    /// How far the filter envelope moves the cutoff, -1..1 (±[`ENV_OCTAVES`]).
    pub env_amount: f32,
    /// How much the cutoff follows the note, 0..1 (1 = one octave per octave).
    pub key_track: f32,
}

/// Cutoff range of the filter envelope at full amount, in octaves.
pub const ENV_OCTAVES: f32 = 7.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Patch {
    pub osc: [OscParams; 2],
    pub filter: FilterParams,
    pub filter_env: AdsrParams,
    pub amp_env: AdsrParams,
    /// 0..1.
    pub volume: f32,
}

impl Default for Patch {
    /// Two slightly detuned saws through a 24 dB low-pass that opens on each note.
    fn default() -> Self {
        Self {
            osc: [
                OscParams { wave: Waveform::Saw, octave: 0, semitones: 0, fine: 0.0, level: 0.8 },
                OscParams { wave: Waveform::Saw, octave: 0, semitones: 0, fine: 7.0, level: 0.6 },
            ],
            filter: FilterParams { mode: FilterMode::Lp24, cutoff: 800.0, resonance: 0.3, env_amount: 0.4, key_track: 0.5 },
            filter_env: AdsrParams { attack: 0.005, decay: 0.5, sustain: 0.3, release: 0.5 },
            amp_env: AdsrParams { attack: 0.005, decay: 0.3, sustain: 0.8, release: 0.4 },
            volume: 0.7,
        }
    }
}
