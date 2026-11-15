//! All settings of a sound. The interface edits a copy and sends the whole patch to
//! the engine when something changes; presets are saved patches.
//!
//! With the `serde` feature a patch can be written and read (e.g. as JSON). Fields
//! missing from an older preset take their default values.

use crate::envelope::AdsrParams;
use crate::filter::FilterMode;
use crate::lfo::LfoWave;
use crate::modmatrix::{ModSlot, SLOTS};
use crate::oscillator::Waveform;
use crate::wavetable::WavetableKind;

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(default))]
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
    /// Table read when `wave` is [`Waveform::Wavetable`].
    pub table: WavetableKind,
    /// Place in the table, 0..1.
    pub position: f32,
}

impl Default for OscParams {
    fn default() -> Self {
        Self { wave: Waveform::Saw, octave: 0, semitones: 0, fine: 0.0, level: 0.8, table: WavetableKind::Basic, position: 0.0 }
    }
}

impl OscParams {
    /// Pitch offset in semitones.
    pub fn offset(&self) -> f32 {
        self.octave as f32 * 12.0 + self.semitones as f32 + self.fine / 100.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

/// Cutoff range of the LFO and the mod wheel at full amount, in octaves.
pub const MOD_OCTAVES: f32 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(default))]
pub struct LfoParams {
    pub wave: LfoWave,
    /// Hz.
    pub rate: f32,
    /// Vibrato depth in semitones, 0..12.
    pub pitch: f32,
    /// Filter wobble, -1..1 (±[`MOD_OCTAVES`]).
    pub cutoff: f32,
    /// Tremolo depth, 0..1.
    pub amp: f32,
}

impl Default for LfoParams {
    fn default() -> Self {
        Self { wave: LfoWave::Sine, rate: 5.0, pitch: 0.0, cutoff: 0.0, amp: 0.0 }
    }
}

/// What the mod wheel (and aftertouch, whichever is higher) does.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(default))]
pub struct WheelParams {
    /// Extra vibrato from the LFO at full wheel, in semitones (0..2).
    pub vibrato: f32,
    /// Moves the cutoff at full wheel, -1..1 (±[`MOD_OCTAVES`]).
    pub cutoff: f32,
}

impl Default for WheelParams {
    fn default() -> Self {
        Self { vibrato: 0.5, cutoff: 0.0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(default))]
pub struct ChorusParams {
    pub on: bool,
    /// Sweep speed, Hz (0.1..5).
    pub rate: f32,
    /// Sweep width, 0..1 (up to 6 ms).
    pub depth: f32,
    /// 0..1; full is half dry, half chorus.
    pub mix: f32,
}

impl Default for ChorusParams {
    fn default() -> Self {
        Self { on: false, rate: 0.6, depth: 0.5, mix: 0.7 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(default))]
pub struct DelayParams {
    pub on: bool,
    /// Time between repeats, ms (20..2000).
    pub time_ms: f32,
    /// 0..0.95.
    pub feedback: f32,
    /// Low-pass in the repeats, Hz: lower makes each echo darker.
    pub tone: f32,
    /// Repeats alternate between left and right.
    pub ping_pong: bool,
    /// Level of the echoes, 0..1.
    pub mix: f32,
}

impl Default for DelayParams {
    fn default() -> Self {
        Self { on: false, time_ms: 375.0, feedback: 0.35, tone: 5000.0, ping_pong: true, mix: 0.3 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(default))]
pub struct ReverbParams {
    pub on: bool,
    /// Room size, 0..1.
    pub size: f32,
    /// Time to fade by 60 dB, seconds (0.3..10).
    pub decay: f32,
    /// 0 bright .. 1 dark.
    pub damping: f32,
    /// Gap before the reverb starts, ms (0..100).
    pub predelay_ms: f32,
    /// Level of the reverb, 0..1.
    pub mix: f32,
}

impl Default for ReverbParams {
    fn default() -> Self {
        Self { on: false, size: 0.5, decay: 1.8, damping: 0.4, predelay_ms: 10.0, mix: 0.3 }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum VoiceMode {
    /// Up to 16 notes at once.
    #[default]
    Poly,
    /// One note at a time; every new key restarts the envelopes.
    Mono,
    /// One note at a time; keys played while another is held only change the
    /// pitch (gliding if glide is on), envelopes run on.
    Legato,
}

impl VoiceMode {
    pub const ALL: [VoiceMode; 3] = [VoiceMode::Poly, VoiceMode::Mono, VoiceMode::Legato];
}

/// How notes are played.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(default))]
pub struct PlayParams {
    pub mode: VoiceMode,
    /// Portamento: time to slide most of the way to a new note, seconds (0..2).
    pub glide: f32,
    /// How much key velocity changes the level, 0 (not at all) .. 1 (fully).
    pub velocity: f32,
}

impl Default for PlayParams {
    fn default() -> Self {
        Self { mode: VoiceMode::Poly, glide: 0.0, velocity: 1.0 }
    }
}

/// Several detuned copies of each oscillator per note, spread across the stereo field.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(default))]
pub struct UnisonParams {
    /// Copies per oscillator, 1..7.
    pub voices: u8,
    /// Spread of the outermost copies, cents (0..100).
    pub detune: f32,
    /// Stereo width, 0..1.
    pub spread: f32,
}

impl Default for UnisonParams {
    fn default() -> Self {
        Self { voices: 1, detune: 20.0, spread: 0.6 }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(default))]
pub struct EffectsParams {
    pub chorus: ChorusParams,
    pub delay: DelayParams,
    pub reverb: ReverbParams,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(default))]
pub struct Patch {
    pub osc: [OscParams; 2],
    pub filter: FilterParams,
    pub filter_env: AdsrParams,
    pub amp_env: AdsrParams,
    pub lfo: LfoParams,
    pub wheel: WheelParams,
    /// Pitch-bend range in semitones each way (0..12).
    pub bend_range: f32,
    pub play: PlayParams,
    /// White noise level added to the oscillators, 0..1.
    pub noise: f32,
    pub unison: UnisonParams,
    pub mods: [ModSlot; SLOTS],
    pub fx: EffectsParams,
    /// 0..1.
    pub volume: f32,
}

impl Default for Patch {
    /// Two slightly detuned saws through a 24 dB low-pass that opens on each note.
    fn default() -> Self {
        Self {
            osc: [
                OscParams { wave: Waveform::Saw, level: 0.8, ..OscParams::default() },
                OscParams { wave: Waveform::Saw, fine: 7.0, level: 0.6, ..OscParams::default() },
            ],
            filter: FilterParams { mode: FilterMode::Lp24, cutoff: 800.0, resonance: 0.3, env_amount: 0.4, key_track: 0.5 },
            filter_env: AdsrParams { attack: 0.005, decay: 0.5, sustain: 0.3, release: 0.5 },
            amp_env: AdsrParams { attack: 0.005, decay: 0.3, sustain: 0.8, release: 0.4 },
            lfo: LfoParams::default(),
            wheel: WheelParams::default(),
            bend_range: 2.0,
            play: PlayParams::default(),
            noise: 0.0,
            unison: UnisonParams::default(),
            mods: [ModSlot::default(); SLOTS],
            fx: EffectsParams::default(),
            volume: 0.7,
        }
    }
}
