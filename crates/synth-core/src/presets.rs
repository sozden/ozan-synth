//! Factory presets: one example of each classic sound the engine can make.

use crate::envelope::AdsrParams;
use crate::filter::FilterMode;
use crate::lfo::LfoWave;
use crate::oscillator::Waveform;
use crate::patch::{FilterParams, LfoParams, OscParams, Patch, WheelParams};

fn osc(wave: Waveform, octave: i8, semitones: i8, fine: f32, level: f32) -> OscParams {
    OscParams { wave, octave, semitones, fine, level }
}

const OFF: OscParams = OscParams { wave: Waveform::Sine, octave: 0, semitones: 0, fine: 0.0, level: 0.0 };

fn filter(mode: FilterMode, cutoff: f32, resonance: f32, env_amount: f32, key_track: f32) -> FilterParams {
    FilterParams { mode, cutoff, resonance, env_amount, key_track }
}

fn adsr(attack: f32, decay: f32, sustain: f32, release: f32) -> AdsrParams {
    AdsrParams { attack, decay, sustain, release }
}

fn lfo(wave: LfoWave, rate: f32, pitch: f32, cutoff: f32, amp: f32) -> LfoParams {
    LfoParams { wave, rate, pitch, cutoff, amp }
}

/// Built-in presets, in the order they appear in the list.
pub fn factory() -> Vec<(&'static str, Patch)> {
    use FilterMode::*;
    use Waveform::*;
    let d = Patch::default();
    vec![
        ("Başlangıç", d),
        (
            "Acid bas",
            Patch {
                osc: [osc(Square, -1, 0, 0.0, 1.0), OFF],
                filter: filter(Lp24, 180.0, 0.82, 0.55, 0.3),
                filter_env: adsr(0.002, 0.18, 0.0, 0.1),
                amp_env: adsr(0.002, 0.3, 0.9, 0.08),
                ..d
            },
        ),
        (
            "Kalın bas",
            Patch {
                osc: [osc(Saw, -1, 0, 0.0, 0.8), osc(Square, -2, 0, 0.0, 0.6)],
                filter: filter(Lp24, 300.0, 0.2, 0.35, 0.3),
                filter_env: adsr(0.003, 0.35, 0.2, 0.15),
                amp_env: adsr(0.003, 0.2, 1.0, 0.1),
                ..d
            },
        ),
        (
            "Wobble bas",
            Patch {
                osc: [osc(Saw, -1, 0, 0.0, 0.8), osc(Square, -1, 0, 10.0, 0.6)],
                filter: filter(Lp24, 220.0, 0.6, 0.1, 0.2),
                amp_env: adsr(0.005, 0.2, 1.0, 0.15),
                lfo: lfo(LfoWave::Sine, 3.0, 0.0, 0.6, 0.0),
                ..d
            },
        ),
        (
            "Yumuşak pad",
            Patch {
                osc: [osc(Saw, 0, 0, 0.0, 0.7), osc(Saw, 0, 0, 12.0, 0.7)],
                filter: filter(Lp12, 1200.0, 0.15, 0.2, 0.3),
                filter_env: adsr(1.2, 2.0, 0.6, 2.0),
                amp_env: adsr(0.8, 1.0, 0.9, 2.5),
                lfo: lfo(LfoWave::Triangle, 0.3, 0.0, 0.15, 0.0),
                ..d
            },
        ),
        (
            "Pirinç",
            Patch {
                osc: [osc(Saw, 0, 0, 0.0, 0.8), osc(Saw, 0, 0, -8.0, 0.7)],
                filter: filter(Lp24, 500.0, 0.1, 0.55, 0.5),
                filter_env: adsr(0.08, 0.6, 0.45, 0.3),
                amp_env: adsr(0.04, 0.2, 0.9, 0.25),
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
                ..d
            },
        ),
        (
            "Vibrato lead",
            Patch {
                osc: [osc(Saw, 0, 0, 0.0, 0.8), osc(Square, 0, 7, 0.0, 0.4)],
                filter: filter(Lp24, 1500.0, 0.3, 0.3, 0.5),
                amp_env: adsr(0.01, 0.2, 1.0, 0.2),
                lfo: lfo(LfoWave::Sine, 5.5, 0.15, 0.0, 0.0),
                wheel: WheelParams { vibrato: 1.0, cutoff: 0.2 },
                ..d
            },
        ),
        (
            "Tremolo org",
            Patch {
                osc: [osc(Sine, 0, 0, 0.0, 0.8), osc(Sine, 1, 0, 0.0, 0.5)],
                filter: filter(Lp12, 8000.0, 0.0, 0.0, 0.0),
                amp_env: adsr(0.005, 0.1, 1.0, 0.08),
                lfo: lfo(LfoWave::Sine, 6.0, 0.0, 0.0, 0.4),
                ..d
            },
        ),
        (
            "S&H bloop",
            Patch {
                osc: [osc(Square, 0, 0, 0.0, 0.9), OFF],
                filter: filter(Lp24, 600.0, 0.75, 0.2, 0.5),
                amp_env: adsr(0.005, 0.3, 1.0, 0.3),
                lfo: lfo(LfoWave::SampleHold, 8.0, 0.0, 0.5, 0.0),
                ..d
            },
        ),
        (
            "Saf sinüs",
            Patch {
                osc: [osc(Sine, 0, 0, 0.0, 1.0), OFF],
                filter: filter(Lp12, 20_000.0, 0.0, 0.0, 0.0),
                amp_env: adsr(0.01, 0.3, 0.7, 0.4),
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
