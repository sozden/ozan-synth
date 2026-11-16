//! Factory presets: one example of each classic sound the engine can make.
//!
//! Volumes are matched by measurement (`ozan-lab render`) so that switching
//! presets does not jump in loudness: about -25 dB RMS while the test phrase
//! plays (short plucks a little lower, judged by their peaks).

use crate::arp::{ArpDivision, ArpMode, ArpParams};
use crate::envelope::AdsrParams;
use crate::filter::FilterMode;
use crate::lfo::LfoWave;
use crate::modmatrix::{ModDest, ModSlot, ModSource, SLOTS};
use crate::oscillator::Waveform;
use crate::wavetable::WavetableKind;
use crate::patch::{
    ChorusParams, DelayParams, EffectsParams, FilterParams, LfoParams, OscParams, Patch, PlayParams, ReverbParams, UnisonParams, VoiceMode,
    WheelParams,
};

fn osc(wave: Waveform, octave: i8, semitones: i8, fine: f32, level: f32) -> OscParams {
    OscParams { wave, octave, semitones, fine, level, ..OscParams::default() }
}

/// A wavetable oscillator.
fn wt(table: WavetableKind, position: f32, octave: i8, fine: f32, level: f32) -> OscParams {
    OscParams { wave: Waveform::Wavetable, octave, fine, level, table, position, ..OscParams::default() }
}

const OFF: OscParams = OscParams {
    wave: Waveform::Sine,
    octave: 0,
    semitones: 0,
    fine: 0.0,
    level: 0.0,
    table: WavetableKind::Basic,
    position: 0.0,
};

fn filter(mode: FilterMode, cutoff: f32, resonance: f32, env_amount: f32, key_track: f32) -> FilterParams {
    FilterParams { mode, cutoff, resonance, env_amount, key_track, drive: 0.0 }
}

fn adsr(attack: f32, decay: f32, sustain: f32, release: f32) -> AdsrParams {
    AdsrParams { attack, decay, sustain, release }
}

fn lfo(wave: LfoWave, rate: f32, pitch: f32, cutoff: f32, amp: f32) -> LfoParams {
    LfoParams { wave, rate, pitch, cutoff, amp }
}

fn chorus(rate: f32, depth: f32, mix: f32) -> Option<ChorusParams> {
    Some(ChorusParams { on: true, rate, depth, mix })
}

fn delay(time_ms: f32, feedback: f32, tone: f32, ping_pong: bool, mix: f32) -> Option<DelayParams> {
    Some(DelayParams { on: true, time_ms, feedback, tone, ping_pong, mix })
}

fn reverb(size: f32, decay: f32, damping: f32, predelay_ms: f32, mix: f32) -> Option<ReverbParams> {
    Some(ReverbParams { on: true, size, decay, damping, predelay_ms, mix })
}

/// Effects in use; the others stay off with their default settings.
fn fx(c: Option<ChorusParams>, d: Option<DelayParams>, r: Option<ReverbParams>) -> EffectsParams {
    let off = EffectsParams::default();
    EffectsParams { chorus: c.unwrap_or(off.chorus), delay: d.unwrap_or(off.delay), reverb: r.unwrap_or(off.reverb) }
}

fn unison(voices: u8, detune: f32, spread: f32) -> UnisonParams {
    UnisonParams { voices, detune, spread }
}

/// Matrix slots in use; the rest stay off.
fn mods(slots: &[ModSlot]) -> [ModSlot; SLOTS] {
    let mut all = [ModSlot::default(); SLOTS];
    all[..slots.len()].copy_from_slice(slots);
    all
}

fn play(mode: VoiceMode, glide: f32, velocity: f32) -> PlayParams {
    PlayParams { mode, glide, velocity }
}

fn arp(mode: ArpMode, division: ArpDivision, octaves: u8, gate: f32, bpm: f32) -> ArpParams {
    ArpParams { on: true, mode, division, octaves, gate, hold: false, bpm }
}

/// Built-in presets, in the order they appear in the list.
pub fn factory() -> Vec<(&'static str, Patch)> {
    use FilterMode::*;
    use Waveform::*;
    let d = Patch::default();
    vec![
        ("Init", Patch { volume: 0.88, ..d }),
        (
            "Acid Bass",
            Patch {
                osc: [osc(Square, -1, 0, 0.0, 1.0), OFF],
                filter: filter(Lp24, 180.0, 0.82, 0.55, 0.3),
                filter_env: adsr(0.002, 0.18, 0.0, 0.1),
                amp_env: adsr(0.002, 0.3, 0.9, 0.08),
                volume: 0.31,
                ..d
            },
        ),
        (
            "Fat Bass",
            Patch {
                osc: [osc(Saw, -1, 0, 0.0, 0.8), osc(Square, -2, 0, 0.0, 0.6)],
                filter: filter(Lp24, 300.0, 0.2, 0.35, 0.3),
                filter_env: adsr(0.003, 0.35, 0.2, 0.15),
                amp_env: adsr(0.003, 0.2, 1.0, 0.1),
                volume: 0.49,
                ..d
            },
        ),
        (
            "Wobble Bass",
            Patch {
                osc: [osc(Saw, -1, 0, 0.0, 0.8), osc(Square, -1, 0, 10.0, 0.6)],
                filter: filter(Lp24, 220.0, 0.6, 0.1, 0.2),
                amp_env: adsr(0.005, 0.2, 1.0, 0.15),
                lfo: lfo(LfoWave::Sine, 3.0, 0.0, 0.6, 0.0),
                volume: 0.5,
                ..d
            },
        ),
        (
            "Soft Pad",
            Patch {
                osc: [osc(Saw, 0, 0, 0.0, 0.7), osc(Saw, 0, 0, 12.0, 0.7)],
                filter: filter(Lp12, 1200.0, 0.15, 0.2, 0.3),
                filter_env: adsr(1.2, 2.0, 0.6, 2.0),
                amp_env: adsr(0.8, 1.0, 0.9, 2.5),
                lfo: lfo(LfoWave::Triangle, 0.3, 0.0, 0.15, 0.0),
                fx: fx(chorus(0.5, 0.7, 0.8), None, reverb(0.8, 4.5, 0.5, 20.0, 0.45)),
                volume: 0.95,
                ..d
            },
        ),
        (
            "Brass",
            Patch {
                osc: [osc(Saw, 0, 0, 0.0, 0.8), osc(Saw, 0, 0, -8.0, 0.7)],
                filter: filter(Lp24, 500.0, 0.1, 0.55, 0.5),
                filter_env: adsr(0.08, 0.6, 0.45, 0.3),
                amp_env: adsr(0.04, 0.2, 0.9, 0.25),
                fx: fx(None, None, reverb(0.6, 2.2, 0.4, 15.0, 0.25)),
                volume: 0.64,
                ..d
            },
        ),
        (
            "Pluck",
            Patch {
                osc: [osc(Square, 0, 0, 0.0, 0.8), osc(Saw, 1, 0, 0.0, 0.4)],
                filter: filter(Lp24, 250.0, 0.35, 0.7, 0.8),
                filter_env: adsr(0.001, 0.25, 0.0, 0.25),
                amp_env: adsr(0.001, 0.6, 0.0, 0.4),
                fx: fx(None, delay(300.0, 0.4, 4000.0, true, 0.35), reverb(0.5, 1.5, 0.4, 5.0, 0.2)),
                volume: 1.0,
                ..d
            },
        ),
        (
            "Vibrato Lead",
            Patch {
                osc: [osc(Saw, 0, 0, 0.0, 0.8), osc(Square, 0, 7, 0.0, 0.4)],
                filter: filter(Lp24, 1500.0, 0.3, 0.3, 0.5),
                amp_env: adsr(0.01, 0.2, 1.0, 0.2),
                lfo: lfo(LfoWave::Sine, 5.5, 0.15, 0.0, 0.0),
                wheel: WheelParams { vibrato: 1.0, cutoff: 0.2 },
                fx: fx(None, delay(420.0, 0.3, 3500.0, false, 0.25), reverb(0.6, 2.5, 0.4, 20.0, 0.3)),
                volume: 0.49,
                ..d
            },
        ),
        (
            "Tremolo Organ",
            Patch {
                osc: [osc(Sine, 0, 0, 0.0, 0.8), osc(Sine, 1, 0, 0.0, 0.5)],
                filter: filter(Lp12, 8000.0, 0.0, 0.0, 0.0),
                amp_env: adsr(0.005, 0.1, 1.0, 0.08),
                lfo: lfo(LfoWave::Sine, 6.0, 0.0, 0.0, 0.4),
                fx: fx(chorus(0.8, 0.4, 0.5), None, reverb(0.5, 1.6, 0.5, 10.0, 0.2)),
                volume: 0.79,
                ..d
            },
        ),
        (
            "S&H Bloop",
            Patch {
                osc: [osc(Square, 0, 0, 0.0, 0.9), OFF],
                filter: filter(Lp24, 600.0, 0.75, 0.2, 0.5),
                amp_env: adsr(0.005, 0.3, 1.0, 0.3),
                lfo: lfo(LfoWave::SampleHold, 8.0, 0.0, 0.5, 0.0),
                fx: fx(None, delay(250.0, 0.5, 5000.0, true, 0.35), None),
                volume: 0.32,
                ..d
            },
        ),
        (
            "Distant Bell",
            Patch {
                osc: [osc(Sine, 0, 0, 0.0, 0.8), osc(Triangle, 2, 0, 0.0, 0.4)],
                filter: filter(Lp12, 6000.0, 0.0, 0.0, 0.3),
                amp_env: adsr(0.001, 1.5, 0.0, 1.5),
                fx: fx(None, delay(450.0, 0.45, 3000.0, true, 0.3), reverb(0.9, 6.0, 0.3, 30.0, 0.5)),
                volume: 0.53,
                ..d
            },
        ),
        (
            "Supersaw",
            Patch {
                osc: [osc(Saw, 0, 0, 0.0, 0.9), OFF],
                unison: unison(7, 25.0, 0.8),
                filter: filter(Lp24, 3500.0, 0.1, 0.2, 0.4),
                filter_env: adsr(0.005, 0.5, 0.4, 0.4),
                amp_env: adsr(0.01, 0.3, 0.9, 0.5),
                fx: fx(None, None, reverb(0.5, 2.5, 0.4, 15.0, 0.25)),
                volume: 0.75,
                ..d
            },
        ),
        (
            "Vowel Pad",
            Patch {
                osc: [wt(WavetableKind::Vowels, 0.0, 0, 0.0, 1.0), wt(WavetableKind::Vowels, 0.5, 0, 8.0, 0.8)],
                unison: unison(3, 12.0, 0.6),
                filter: filter(Lp12, 5000.0, 0.0, 0.0, 0.2),
                amp_env: adsr(0.6, 1.0, 0.9, 2.0),
                lfo: lfo(LfoWave::Triangle, 0.15, 0.0, 0.0, 0.0),
                mods: mods(&[
                    ModSlot::new(ModSource::Lfo, ModDest::Osc1Position, 0.5),
                    ModSlot::new(ModSource::Lfo, ModDest::Osc2Position, -0.4),
                ]),
                fx: fx(chorus(0.4, 0.6, 0.6), None, reverb(0.8, 4.0, 0.5, 25.0, 0.4)),
                volume: 0.9,
                ..d
            },
        ),
        (
            "PWM Strings",
            Patch {
                osc: [wt(WavetableKind::Pulse, 0.3, 0, -6.0, 0.7), wt(WavetableKind::Pulse, 0.3, 0, 6.0, 0.7)],
                filter: filter(Lp24, 2500.0, 0.1, 0.15, 0.5),
                amp_env: adsr(0.25, 0.5, 0.85, 0.8),
                lfo: lfo(LfoWave::Sine, 0.5, 0.0, 0.0, 0.0),
                mods: mods(&[
                    ModSlot::new(ModSource::Lfo, ModDest::Osc1Position, 0.35),
                    ModSlot::new(ModSource::Lfo, ModDest::Osc2Position, -0.35),
                ]),
                fx: fx(chorus(0.5, 0.5, 0.5), None, reverb(0.6, 2.2, 0.4, 15.0, 0.25)),
                volume: 0.7,
                ..d
            },
        ),
        (
            "Sync Lead",
            Patch {
                osc: [wt(WavetableKind::Sync, 0.1, 0, 0.0, 0.9), OFF],
                filter: filter(Lp24, 4000.0, 0.2, 0.2, 0.5),
                filter_env: adsr(0.002, 0.6, 0.1, 0.3),
                amp_env: adsr(0.005, 0.2, 1.0, 0.3),
                mods: mods(&[
                    ModSlot::new(ModSource::FilterEnv, ModDest::Osc1Position, 0.8),
                    ModSlot::new(ModSource::Wheel, ModDest::Osc1Position, 0.5),
                    ModSlot::new(ModSource::Velocity, ModDest::Cutoff, 0.3),
                ]),
                fx: fx(None, delay(350.0, 0.3, 4000.0, true, 0.2), None),
                volume: 0.75,
                ..d
            },
        ),
        (
            "Harmonic Keys",
            Patch {
                osc: [wt(WavetableKind::Harmonics, 0.2, 0, 0.0, 1.0), OFF],
                filter: filter(Lp12, 6000.0, 0.0, 0.0, 0.0),
                amp_env: adsr(0.002, 1.2, 0.3, 0.6),
                mods: mods(&[
                    ModSlot::new(ModSource::Velocity, ModDest::Osc1Position, 0.7),
                    ModSlot::new(ModSource::Velocity, ModDest::Volume, 0.4),
                    ModSlot::new(ModSource::Note, ModDest::Cutoff, 0.3),
                ]),
                fx: fx(None, None, reverb(0.5, 1.8, 0.4, 10.0, 0.2)),
                volume: 0.93,
                ..d
            },
        ),
        (
            "Glide Lead",
            Patch {
                osc: [osc(Saw, 0, 0, 0.0, 0.8), osc(Square, -1, 0, 5.0, 0.5)],
                play: play(VoiceMode::Legato, 0.12, 0.8),
                filter: filter(Lp24, 1800.0, 0.35, 0.35, 0.5),
                filter_env: adsr(0.002, 0.5, 0.4, 0.3),
                amp_env: adsr(0.005, 0.2, 1.0, 0.2),
                wheel: WheelParams { vibrato: 0.6, cutoff: 0.2 },
                fx: fx(None, delay(300.0, 0.3, 3500.0, true, 0.2), reverb(0.5, 1.8, 0.4, 10.0, 0.2)),
                volume: 0.55,
                ..d
            },
        ),
        (
            "Mono Bass",
            Patch {
                osc: [osc(Saw, -1, 0, 0.0, 0.8), osc(Square, -2, 0, 0.0, 0.6)],
                play: play(VoiceMode::Mono, 0.05, 0.6),
                filter: filter(Lp24, 250.0, 0.4, 0.55, 0.3),
                filter_env: adsr(0.001, 0.25, 0.1, 0.1),
                amp_env: adsr(0.002, 0.2, 1.0, 0.1),
                volume: 0.53,
                ..d
            },
        ),
        (
            "Wind",
            Patch {
                osc: [OFF, OFF],
                noise: 1.0,
                filter: filter(Lp12, 1400.0, 0.6, 0.0, 0.5),
                amp_env: adsr(0.6, 1.0, 1.0, 2.0),
                lfo: lfo(LfoWave::Triangle, 0.2, 0.0, 0.4, 0.0),
                fx: fx(chorus(0.3, 0.6, 0.5), None, reverb(0.9, 5.0, 0.4, 30.0, 0.5)),
                volume: 1.0,
                ..d
            },
        ),
        (
            "Arp Pluck",
            Patch {
                osc: [osc(Square, 0, 0, 0.0, 0.8), osc(Saw, 1, 0, 0.0, 0.4)],
                filter: filter(Lp24, 300.0, 0.4, 0.65, 0.8),
                filter_env: adsr(0.001, 0.2, 0.0, 0.2),
                amp_env: adsr(0.001, 0.3, 0.0, 0.2),
                arp: arp(ArpMode::Up, ArpDivision::Sixteenth, 2, 0.4, 124.0),
                // A dotted eighth at 124 BPM.
                fx: fx(None, delay(363.0, 0.4, 3500.0, true, 0.3), reverb(0.5, 1.5, 0.4, 5.0, 0.15)),
                volume: 0.68,
                ..d
            },
        ),
        (
            "Random Bells",
            Patch {
                osc: [osc(Sine, 0, 0, 0.0, 0.8), osc(Triangle, 2, 0, 0.0, 0.4)],
                filter: filter(Lp12, 6000.0, 0.0, 0.0, 0.3),
                amp_env: adsr(0.001, 0.9, 0.0, 0.9),
                arp: arp(ArpMode::Random, ArpDivision::Eighth, 2, 0.8, 100.0),
                fx: fx(None, delay(450.0, 0.35, 3000.0, true, 0.25), reverb(0.9, 5.0, 0.3, 30.0, 0.45)),
                volume: 0.52,
                ..d
            },
        ),
        (
            "Evolving Pad",
            Patch {
                osc: [wt(WavetableKind::Harmonics, 0.0, 0, 0.0, 0.9), wt(WavetableKind::Vowels, 0.3, 0, 7.0, 0.6)],
                unison: unison(3, 15.0, 0.5),
                filter: filter(Lp12, 3000.0, 0.1, 0.0, 0.3),
                amp_env: adsr(1.0, 1.0, 0.9, 3.0),
                mod_env: adsr(3.0, 2.0, 0.6, 3.0),
                lfo2: lfo(LfoWave::Sine, 0.13, 0.0, 0.0, 0.0),
                mods: mods(&[
                    ModSlot::new(ModSource::ModEnv, ModDest::Osc1Position, 0.9),
                    ModSlot::new(ModSource::Lfo2, ModDest::Pan, 0.6),
                    ModSlot::new(ModSource::Lfo2, ModDest::Osc2Position, 0.3),
                ]),
                fx: fx(chorus(0.3, 0.5, 0.5), None, reverb(0.85, 5.0, 0.45, 25.0, 0.45)),
                volume: 0.94,
                ..d
            },
        ),
        (
            "Gritty Bass",
            Patch {
                osc: [osc(Saw, -1, 0, 0.0, 0.8), osc(Square, -1, 0, -6.0, 0.6)],
                filter: FilterParams { drive: 0.6, ..filter(Lp24, 400.0, 0.5, 0.45, 0.4) },
                filter_env: adsr(0.002, 0.3, 0.2, 0.15),
                amp_env: adsr(0.002, 0.2, 1.0, 0.12),
                mods: mods(&[ModSlot::new(ModSource::Velocity, ModDest::Drive, 0.4)]),
                volume: 0.38,
                ..d
            },
        ),
        (
            "Notch Sweep",
            Patch {
                osc: [osc(Saw, 0, 0, 0.0, 0.7), osc(Saw, 0, 0, 9.0, 0.7)],
                unison: unison(3, 14.0, 0.7),
                filter: filter(Notch, 1200.0, 0.3, 0.0, 0.2),
                amp_env: adsr(0.4, 1.0, 0.9, 1.8),
                lfo2: lfo(LfoWave::Triangle, 0.25, 0.0, 0.0, 0.0),
                mods: mods(&[ModSlot::new(ModSource::Lfo2, ModDest::Cutoff, 0.45)]),
                fx: fx(chorus(0.4, 0.5, 0.5), None, reverb(0.7, 3.0, 0.4, 20.0, 0.35)),
                volume: 0.94,
                ..d
            },
        ),
        (
            "Pure Sine",
            Patch {
                osc: [osc(Sine, 0, 0, 0.0, 1.0), OFF],
                filter: filter(Lp12, 20_000.0, 0.0, 0.0, 0.0),
                amp_env: adsr(0.01, 0.3, 0.7, 0.4),
                volume: 0.75,
                ..d
            },
        ),
        // Added after the first 25, so earlier preset numbers keep their sound.
        (
            "Sub Bass",
            Patch {
                osc: [osc(Sine, -1, 0, 0.0, 1.0), osc(Triangle, -2, 0, 0.0, 0.35)],
                play: play(VoiceMode::Legato, 0.04, 0.5),
                filter: filter(Lp12, 500.0, 0.0, 0.0, 0.2),
                amp_env: adsr(0.004, 0.2, 1.0, 0.12),
                volume: 0.55,
                ..d
            },
        ),
        (
            "Reese Bass",
            Patch {
                osc: [osc(Saw, -1, 0, 0.0, 0.8), osc(Saw, -1, 0, 18.0, 0.8)],
                unison: unison(3, 22.0, 0.4),
                filter: filter(Lp24, 650.0, 0.25, 0.15, 0.3),
                amp_env: adsr(0.005, 0.2, 1.0, 0.15),
                lfo2: lfo(LfoWave::Sine, 0.2, 0.0, 0.0, 0.0),
                mods: mods(&[ModSlot::new(ModSource::Lfo2, ModDest::Cutoff, 0.12)]),
                volume: 0.63,
                ..d
            },
        ),
        (
            "Pluck Bass",
            Patch {
                osc: [osc(Square, -1, 0, 0.0, 0.8), osc(Saw, -1, 0, 5.0, 0.6)],
                filter: filter(Lp24, 220.0, 0.35, 0.6, 0.4),
                filter_env: adsr(0.001, 0.16, 0.0, 0.1),
                amp_env: adsr(0.001, 0.45, 0.0, 0.12),
                volume: 0.66,
                ..d
            },
        ),
        (
            "Rubber Bass",
            Patch {
                osc: [osc(Sine, -1, 0, 0.0, 1.0), osc(Square, -1, 0, 0.0, 0.35)],
                play: play(VoiceMode::Mono, 0.08, 0.6),
                filter: filter(Lp24, 300.0, 0.55, 0.4, 0.3),
                filter_env: adsr(0.002, 0.3, 0.1, 0.1),
                amp_env: adsr(0.002, 0.3, 0.8, 0.1),
                volume: 0.62,
                ..d
            },
        ),
        (
            "Hoover",
            Patch {
                osc: [osc(Saw, 0, 0, 0.0, 0.8), osc(Saw, -1, 0, 0.0, 0.6)],
                unison: unison(7, 45.0, 0.9),
                play: play(VoiceMode::Legato, 0.15, 0.5),
                filter: filter(Lp12, 2500.0, 0.15, 0.0, 0.3),
                amp_env: adsr(0.01, 0.3, 0.9, 0.3),
                mod_env: adsr(0.001, 0.25, 0.0, 0.2),
                mods: mods(&[ModSlot::new(ModSource::ModEnv, ModDest::Pitch, -0.12)]),
                volume: 0.81,
                ..d
            },
        ),
        (
            "Square Lead",
            Patch {
                osc: [osc(Square, 0, 0, 0.0, 0.8), osc(Square, 1, 0, 5.0, 0.35)],
                play: play(VoiceMode::Legato, 0.03, 0.6),
                filter: filter(Lp24, 3000.0, 0.2, 0.2, 0.5),
                filter_env: adsr(0.005, 0.4, 0.5, 0.3),
                amp_env: adsr(0.005, 0.2, 0.9, 0.2),
                fx: fx(None, delay(330.0, 0.3, 4000.0, true, 0.2), None),
                volume: 0.49,
                ..d
            },
        ),
        (
            "Saw Lead",
            Patch {
                osc: [osc(Saw, 0, 0, 0.0, 0.8), osc(Saw, 0, 0, 9.0, 0.8)],
                play: play(VoiceMode::Legato, 0.06, 0.6),
                filter: filter(Lp24, 4000.0, 0.25, 0.15, 0.5),
                amp_env: adsr(0.005, 0.2, 0.9, 0.25),
                fx: fx(None, delay(375.0, 0.35, 4500.0, true, 0.22), reverb(0.5, 1.6, 0.4, 10.0, 0.18)),
                volume: 0.64,
                ..d
            },
        ),
        (
            "Chip Lead",
            Patch {
                osc: [osc(Square, 0, 0, 0.0, 0.9), OFF],
                filter: filter(Lp12, 20_000.0, 0.0, 0.0, 0.0),
                amp_env: adsr(0.001, 0.1, 0.8, 0.05),
                lfo: lfo(LfoWave::Triangle, 6.0, 0.15, 0.0, 0.0),
                volume: 0.54,
                ..d
            },
        ),
        (
            "Flute",
            Patch {
                osc: [osc(Sine, 0, 0, 0.0, 0.9), osc(Triangle, 1, 0, 0.0, 0.12)],
                noise: 0.08,
                filter: filter(Lp12, 2500.0, 0.1, 0.1, 0.6),
                amp_env: adsr(0.08, 0.2, 0.85, 0.25),
                lfo: lfo(LfoWave::Sine, 5.0, 0.08, 0.0, 0.05),
                fx: fx(None, None, reverb(0.6, 2.2, 0.5, 15.0, 0.28)),
                volume: 0.64,
                ..d
            },
        ),
        (
            "Warm Pad",
            Patch {
                osc: [osc(Triangle, 0, 0, 0.0, 1.0), osc(Saw, 0, 0, 7.0, 0.6)],
                filter: filter(Lp12, 900.0, 0.1, 0.15, 0.3),
                filter_env: adsr(1.5, 2.0, 0.7, 2.0),
                amp_env: adsr(1.0, 1.0, 0.9, 2.5),
                fx: fx(chorus(0.4, 0.6, 0.7), None, reverb(0.8, 4.0, 0.55, 20.0, 0.4)),
                volume: 0.91,
                ..d
            },
        ),
        (
            "Glass Pad",
            Patch {
                osc: [wt(WavetableKind::Harmonics, 0.7, 0, 0.0, 0.7), osc(Sine, 1, 0, 4.0, 0.4)],
                filter: filter(Lp12, 6000.0, 0.1, 0.0, 0.3),
                amp_env: adsr(0.6, 1.0, 0.85, 3.0),
                lfo2: lfo(LfoWave::Sine, 0.18, 0.0, 0.0, 0.0),
                mods: mods(&[ModSlot::new(ModSource::Lfo2, ModDest::Osc1Position, 0.25)]),
                fx: fx(chorus(0.3, 0.4, 0.4), delay(500.0, 0.35, 5000.0, true, 0.2), reverb(0.9, 5.5, 0.35, 30.0, 0.5)),
                volume: 0.56,
                ..d
            },
        ),
        (
            "Dark Strings",
            Patch {
                osc: [osc(Saw, 0, 0, -9.0, 0.95), osc(Saw, 0, 0, 9.0, 0.95)],
                unison: unison(3, 12.0, 0.7),
                filter: filter(Lp24, 1500.0, 0.1, 0.1, 0.4),
                amp_env: adsr(0.5, 1.0, 0.9, 1.5),
                lfo: lfo(LfoWave::Sine, 4.5, 0.05, 0.0, 0.0),
                fx: fx(chorus(0.5, 0.5, 0.6), None, reverb(0.75, 3.0, 0.5, 20.0, 0.35)),
                volume: 0.75,
                ..d
            },
        ),
        (
            "Choir",
            Patch {
                osc: [wt(WavetableKind::Vowels, 0.35, 0, 0.0, 0.8), wt(WavetableKind::Vowels, 0.55, 0, 6.0, 0.6)],
                unison: unison(5, 18.0, 0.8),
                filter: filter(Lp12, 4000.0, 0.0, 0.0, 0.3),
                amp_env: adsr(0.5, 1.0, 0.9, 2.0),
                lfo2: lfo(LfoWave::Sine, 0.1, 0.0, 0.0, 0.0),
                mods: mods(&[ModSlot::new(ModSource::Lfo2, ModDest::Osc1Position, 0.15), ModSlot::new(ModSource::Lfo2, ModDest::Osc2Position, -0.15)]),
                fx: fx(chorus(0.3, 0.5, 0.5), None, reverb(0.85, 4.5, 0.5, 25.0, 0.45)),
                volume: 0.96,
                ..d
            },
        ),
        (
            "E-Piano",
            Patch {
                osc: [osc(Sine, 0, 0, 0.0, 0.9), osc(Triangle, 1, 0, 0.0, 0.25)],
                filter: filter(Lp12, 2500.0, 0.0, 0.35, 0.5),
                filter_env: adsr(0.001, 0.6, 0.2, 0.4),
                amp_env: adsr(0.002, 1.6, 0.25, 0.5),
                lfo: lfo(LfoWave::Sine, 4.0, 0.0, 0.0, 0.15),
                mods: mods(&[ModSlot::new(ModSource::Velocity, ModDest::Cutoff, 0.3)]),
                fx: fx(chorus(0.6, 0.4, 0.5), None, reverb(0.5, 1.5, 0.5, 10.0, 0.18)),
                volume: 1.0,
                ..d
            },
        ),
        (
            "Clav",
            Patch {
                osc: [wt(WavetableKind::Pulse, 0.75, 0, 0.0, 0.9), OFF],
                filter: filter(Lp12, 2000.0, 0.3, 0.45, 0.6),
                filter_env: adsr(0.001, 0.25, 0.2, 0.1),
                amp_env: adsr(0.001, 0.5, 0.3, 0.08),
                volume: 0.8,
                ..d
            },
        ),
        (
            "Drawbar Organ",
            Patch {
                osc: [wt(WavetableKind::Harmonics, 1.0, 0, 0.0, 0.7), osc(Sine, -1, 0, 0.0, 0.5)],
                filter: filter(Lp12, 8000.0, 0.0, 0.0, 0.0),
                amp_env: adsr(0.005, 0.1, 1.0, 0.05),
                lfo: lfo(LfoWave::Sine, 6.5, 0.0, 0.0, 0.08),
                fx: fx(chorus(1.2, 0.5, 0.6), None, reverb(0.4, 1.2, 0.5, 5.0, 0.15)),
                volume: 0.93,
                ..d
            },
        ),
        (
            "Marimba",
            Patch {
                osc: [osc(Sine, 0, 0, 0.0, 0.9), osc(Sine, 2, 0, 0.0, 0.25)],
                filter: filter(Lp12, 5000.0, 0.0, 0.0, 0.3),
                amp_env: adsr(0.001, 0.45, 0.0, 0.35),
                fx: fx(None, None, reverb(0.4, 1.2, 0.5, 5.0, 0.15)),
                volume: 0.85,
                ..d
            },
        ),
        (
            "Kalimba",
            Patch {
                osc: [osc(Sine, 0, 0, 0.0, 0.9), osc(Triangle, 2, 0, 0.0, 0.2)],
                filter: filter(Lp12, 4000.0, 0.0, 0.0, 0.3),
                amp_env: adsr(0.001, 0.8, 0.0, 0.6),
                fx: fx(None, delay(280.0, 0.3, 3000.0, true, 0.22), reverb(0.6, 2.0, 0.4, 10.0, 0.25)),
                volume: 0.85,
                ..d
            },
        ),
        (
            "Brass Stab",
            Patch {
                osc: [osc(Saw, 0, 0, 0.0, 0.8), osc(Saw, 0, 0, 6.0, 0.8)],
                filter: filter(Lp24, 600.0, 0.15, 0.7, 0.4),
                filter_env: adsr(0.02, 0.25, 0.1, 0.2),
                amp_env: adsr(0.005, 0.35, 0.0, 0.2),
                fx: fx(None, None, reverb(0.6, 1.8, 0.4, 10.0, 0.2)),
                volume: 0.75,
                ..d
            },
        ),
        (
            "Gated Pad",
            Patch {
                osc: [osc(Saw, 0, 0, 0.0, 0.7), osc(Saw, 0, 0, 7.0, 0.7)],
                unison: unison(5, 25.0, 0.8),
                filter: filter(Lp12, 2500.0, 0.1, 0.0, 0.3),
                amp_env: adsr(0.05, 0.5, 1.0, 0.6),
                lfo: lfo(LfoWave::Square, 8.0, 0.0, 0.0, 1.0),
                fx: fx(None, delay(250.0, 0.3, 4000.0, true, 0.2), reverb(0.7, 2.5, 0.4, 15.0, 0.3)),
                volume: 0.89,
                ..d
            },
        ),
        (
            "Laser",
            Patch {
                osc: [osc(Saw, 1, 0, 0.0, 0.8), osc(Square, 1, 0, 10.0, 0.5)],
                filter: filter(Lp12, 6000.0, 0.3, 0.0, 0.3),
                amp_env: adsr(0.001, 0.35, 0.0, 0.2),
                mod_env: adsr(0.001, 0.3, 0.0, 0.2),
                mods: mods(&[ModSlot::new(ModSource::ModEnv, ModDest::Pitch, 1.0)]),
                fx: fx(None, delay(220.0, 0.45, 3000.0, true, 0.3), None),
                volume: 0.6,
                ..d
            },
        ),
        (
            "Ocean",
            Patch {
                osc: [OFF, OFF],
                noise: 1.0,
                filter: filter(Lp12, 1000.0, 0.2, 0.0, 0.0),
                amp_env: adsr(1.5, 1.0, 1.0, 3.0),
                lfo: lfo(LfoWave::Sine, 0.12, 0.0, 0.7, 0.25),
                fx: fx(chorus(0.2, 0.6, 0.5), None, reverb(0.9, 6.0, 0.5, 40.0, 0.5)),
                volume: 1.0,
                ..d
            },
        ),
        (
            "Arp Bass",
            Patch {
                osc: [osc(Saw, -1, 0, 0.0, 0.8), osc(Square, -1, 0, 7.0, 0.5)],
                filter: filter(Lp24, 300.0, 0.45, 0.5, 0.4),
                filter_env: adsr(0.001, 0.15, 0.1, 0.1),
                amp_env: adsr(0.001, 0.2, 0.5, 0.08),
                arp: arp(ArpMode::Up, ArpDivision::Sixteenth, 1, 0.6, 122.0),
                volume: 0.76,
                ..d
            },
        ),
        (
            "Arp Sparkle",
            Patch {
                osc: [osc(Triangle, 1, 0, 0.0, 0.8), osc(Sine, 2, 0, 0.0, 0.3)],
                filter: filter(Lp12, 6000.0, 0.0, 0.0, 0.3),
                amp_env: adsr(0.001, 0.25, 0.0, 0.25),
                arp: arp(ArpMode::UpDown, ArpDivision::Sixteenth, 3, 0.5, 118.0),
                fx: fx(None, delay(381.0, 0.4, 4000.0, true, 0.3), reverb(0.8, 3.5, 0.4, 20.0, 0.35)),
                volume: 1.0,
                ..d
            },
        ),
        (
            "Dub Chord",
            Patch {
                osc: [osc(Saw, 0, 0, 0.0, 0.7), osc(Saw, 0, 7, 0.0, 0.6)],
                filter: filter(Lp24, 700.0, 0.3, 0.45, 0.4),
                filter_env: adsr(0.001, 0.2, 0.0, 0.15),
                amp_env: adsr(0.001, 0.3, 0.0, 0.2),
                fx: fx(None, delay(375.0, 0.6, 2200.0, true, 0.4), reverb(0.7, 3.0, 0.5, 15.0, 0.3)),
                volume: 0.9,
                ..d
            },
        ),
        (
            "Talking Bass",
            Patch {
                osc: [wt(WavetableKind::Vowels, 0.0, -1, 0.0, 0.9), osc(Sine, -2, 0, 0.0, 0.4)],
                filter: filter(Lp24, 2000.0, 0.2, 0.0, 0.3),
                amp_env: adsr(0.005, 0.2, 1.0, 0.15),
                lfo: lfo(LfoWave::Triangle, 2.0, 0.0, 0.0, 0.0),
                mods: mods(&[ModSlot::new(ModSource::Lfo, ModDest::Osc1Position, 0.5)]),
                volume: 0.73,
                ..d
            },
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Engine, Event};

    #[test]
    fn names_are_unique() {
        let list = factory();
        let mut names: Vec<_> = list.iter().map(|(n, _)| *n).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), list.len());
    }

    #[test]
    fn every_preset_sounds_and_stays_in_range() {
        for (name, patch) in factory() {
            let mut e = Engine::new(48_000.0);
            e.handle(Event::Patch(patch));
            for note in [36, 48, 60, 72] {
                e.handle(Event::NoteOn { note, velocity: 110 });
            }
            let mut buf = vec![0.0; 48_000];
            e.render(&mut buf);
            let peak = buf.iter().fold(0.0f32, |m, v| m.max(v.abs()));
            assert!(peak.is_finite() && peak < 1.0, "{name}: {peak}");
            assert!(peak > 0.01, "{name} is silent");
        }
    }
}
