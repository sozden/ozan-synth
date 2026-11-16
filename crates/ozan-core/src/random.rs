//! Random sounds within sensible limits (the "dice"), and a level check that
//! keeps them from being much louder or quieter than the factory presets.

use std::sync::Arc;

use crate::arp::{SeqStep, SEQ_STEPS};
use crate::engine::{Engine, Event};
use crate::envelope::AdsrParams;
use crate::filter::FilterMode;
use crate::lfo::LfoWave;
use crate::modmatrix::{ModDest, ModSlot, ModSource};
use crate::oscillator::Waveform;
use crate::patch::{FilterRouting, NoteLength, Patch};
use crate::wavetable::{WavetableBank, WavetableKind};

/// Which part of the sound the dice changes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Part {
    #[default]
    All,
    /// Oscillators, noise, unison, FM and ring.
    Oscillators,
    /// Both filters and the filter envelope.
    Filter,
    /// The three envelopes.
    Envelopes,
    /// LFOs and the matrix.
    Modulation,
    Effects,
}

impl Part {
    pub const ALL: [Part; 6] = [Part::All, Part::Oscillators, Part::Filter, Part::Envelopes, Part::Modulation, Part::Effects];
}

/// xorshift64*: small and good enough for choosing settings.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        // Never zero; mix the seed so nearby seeds give unrelated sounds.
        let mut r = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        r.next();
        r
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// 0..1.
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }

    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }

    /// Even on a log scale (times, frequencies).
    fn log(&mut self, lo: f32, hi: f32) -> f32 {
        (lo.ln() + (hi.ln() - lo.ln()) * self.unit()).exp()
    }

    fn chance(&mut self, p: f32) -> bool {
        self.unit() < p
    }

    fn pick<T: Copy>(&mut self, options: &[T]) -> T {
        options[(self.next() % options.len() as u64) as usize]
    }
}

/// A new sound made from `base`: `part` is chosen afresh, the rest is kept. How
/// notes are played (voice mode, arpeggiator, bend) and the volume never change.
pub fn randomize(base: &Patch, part: Part, seed: u64) -> Patch {
    let mut r = Rng::new(seed);
    let mut p = *base;
    let all = part == Part::All;
    if all || part == Part::Oscillators {
        oscillators(&mut p, &mut r);
    }
    if all || part == Part::Filter {
        filters(&mut p, &mut r);
    }
    if all || part == Part::Envelopes {
        envelopes(&mut p, &mut r);
    } else if part == Part::Filter {
        p.filter_env = shape(&mut r);
    }
    if all || part == Part::Modulation {
        modulation(&mut p, &mut r);
    }
    if all || part == Part::Effects {
        effects(&mut p, &mut r);
    }
    p
}

fn oscillators(p: &mut Patch, r: &mut Rng) {
    use Waveform::*;
    for (k, o) in p.osc.iter_mut().enumerate() {
        o.wave = r.pick(&[Saw, Saw, Square, Square, Triangle, Sine, Wavetable]);
        o.table = r.pick(&WavetableKind::ALL);
        o.position = r.range(0.0, 1.0);
        if k == 0 {
            o.octave = r.pick(&[-1, 0, 0, 0, 1]);
            o.semitones = 0;
            o.fine = 0.0;
            o.level = r.range(0.55, 0.9);
        } else {
            o.octave = r.pick(&[-1, 0, 0, 0, 1]);
            o.semitones = r.pick(&[0, 0, 0, 0, 7, 5, -5, 12, 3]);
            o.fine = r.range(-12.0, 12.0);
            o.level = r.range(0.2, 0.8);
        }
    }
    p.noise = if r.chance(0.2) { r.range(0.02, 0.15) } else { 0.0 };
    p.unison.voices = r.pick(&[1, 1, 1, 2, 3, 5]);
    p.unison.detune = r.range(5.0, 40.0);
    p.unison.spread = r.range(0.3, 0.9);
    p.fm = if r.chance(0.25) { r.range(0.05, 0.5) } else { 0.0 };
    p.ring = if r.chance(0.12) { r.range(0.2, 0.6) } else { 0.0 };
}

fn filters(p: &mut Patch, r: &mut Rng) {
    use FilterMode::*;
    let f = &mut p.filter;
    f.mode = r.pick(&[Lp24, Lp24, Lp24, Lp12, Lp12, BandPass, HighPass, Notch]);
    f.cutoff = if matches!(f.mode, HighPass) { r.log(60.0, 600.0) } else { r.log(250.0, 6000.0) };
    f.resonance = r.range(0.0, 0.7);
    f.env_amount = r.range(-0.2, 0.8);
    f.key_track = r.range(0.0, 0.8);
    f.drive = if r.chance(0.3) { r.range(0.05, 0.5) } else { 0.0 };
    let f2 = &mut p.filter2;
    f2.routing = if r.chance(0.25) { r.pick(&[FilterRouting::Serial, FilterRouting::Parallel]) } else { FilterRouting::Off };
    f2.mode = r.pick(&[HighPass, HighPass, BandPass, Lp12, Notch]);
    f2.cutoff = r.log(100.0, 3000.0);
    f2.resonance = r.range(0.0, 0.6);
    f2.env_amount = r.range(-0.3, 0.5);
    f2.key_track = r.range(0.0, 0.6);
}

/// A filter or modulation envelope.
fn shape(r: &mut Rng) -> AdsrParams {
    AdsrParams { attack: r.log(0.001, 0.6), decay: r.log(0.05, 2.0), sustain: r.range(0.0, 0.8), release: r.log(0.05, 2.0) }
}

fn envelopes(p: &mut Patch, r: &mut Rng) {
    // The level envelope decides the character: pluck, pad or played.
    p.amp_env = match r.pick(&[0, 1, 2, 2]) {
        0 => AdsrParams { attack: r.log(0.001, 0.01), decay: r.log(0.1, 0.8), sustain: r.range(0.0, 0.3), release: r.log(0.1, 0.6) },
        1 => AdsrParams { attack: r.log(0.2, 1.5), decay: r.log(0.5, 2.0), sustain: r.range(0.6, 1.0), release: r.log(0.8, 3.0) },
        _ => AdsrParams { attack: r.log(0.001, 0.03), decay: r.log(0.2, 1.0), sustain: r.range(0.5, 1.0), release: r.log(0.1, 0.8) },
    };
    p.filter_env = shape(r);
    p.mod_env = shape(r);
}

fn modulation(p: &mut Patch, r: &mut Rng) {
    use NoteLength::*;
    let divisions = [Bar, Half, Quarter, QuarterDotted, Eighth, EighthDotted, Sixteenth];
    let l = &mut p.lfo;
    l.wave = r.pick(&LfoWave::ALL);
    l.rate = r.log(0.1, 8.0);
    l.sync = r.chance(0.3);
    l.division = r.pick(&divisions);
    l.pitch = if r.chance(0.3) { r.range(0.0, 0.3) } else { 0.0 };
    l.cutoff = if r.chance(0.4) { r.range(-0.4, 0.4) } else { 0.0 };
    l.amp = if r.chance(0.15) { r.range(0.0, 0.4) } else { 0.0 };
    let l2 = &mut p.lfo2;
    l2.wave = r.pick(&LfoWave::ALL);
    l2.rate = r.log(0.05, 4.0);
    l2.sync = r.chance(0.3);
    l2.division = r.pick(&divisions);

    // Up to three connections, each to a different place.
    use ModDest::*;
    let dests = [Cutoff, Cutoff, Resonance, Osc1Position, Osc2Position, Osc2Pitch, Osc1Level, Osc2Level, UnisonDetune, Pan, Cutoff2, Fm, Drive];
    p.mods = Default::default();
    let n = r.pick(&[0, 1, 1, 2, 2, 3]);
    let mut used = 0;
    for _ in 0..n * 3 {
        if used == n {
            break;
        }
        let source = r.pick(&ModSource::ALL[1..]);
        let dest = r.pick(&dests);
        if p.mods[..used].iter().any(|s| s.dest == dest) {
            continue;
        }
        // Pitch moves are kept small, or the sound goes out of tune.
        let amount = if dest == Osc2Pitch { r.pick(&[1.0 / 12.0, 7.0 / 12.0, 1.0]) * r.pick(&[-0.1, 0.1]) } else { r.range(-0.5, 0.5) };
        p.mods[used] = ModSlot::new(source, dest, amount);
        used += 1;
    }
}

fn effects(p: &mut Patch, r: &mut Rng) {
    use NoteLength::*;
    let fx = &mut p.fx;
    let c = &mut fx.chorus;
    c.on = r.chance(0.4);
    c.rate = r.log(0.2, 2.0);
    c.depth = r.range(0.2, 0.8);
    c.mix = r.range(0.3, 0.8);
    let d = &mut fx.delay;
    d.on = r.chance(0.4);
    d.sync = r.chance(0.6);
    d.division = r.pick(&[Eighth, EighthDotted, Quarter, Sixteenth, QuarterDotted]);
    d.time_ms = r.log(120.0, 700.0);
    d.feedback = r.range(0.15, 0.55);
    d.tone = r.log(1500.0, 8000.0);
    d.ping_pong = r.chance(0.5);
    d.mix = r.range(0.1, 0.35);
    let v = &mut fx.reverb;
    v.on = r.chance(0.6);
    v.size = r.range(0.3, 0.9);
    v.decay = r.log(0.8, 5.0);
    v.damping = r.range(0.2, 0.7);
    v.predelay_ms = r.range(0.0, 40.0);
    v.mix = r.range(0.1, 0.4);
}

/// A new sequencer pattern from the minor pentatonic scale around the key.
pub fn steps(seed: u64) -> [SeqStep; SEQ_STEPS] {
    let mut r = Rng::new(seed);
    let mut steps = [SeqStep::default(); SEQ_STEPS];
    for (i, s) in steps.iter_mut().enumerate() {
        // The first step of each beat plays more often, and more often the root.
        let downbeat = i % 4 == 0;
        s.on = r.chance(if downbeat { 0.95 } else { 0.7 });
        s.note = if downbeat && r.chance(0.5) { 0 } else { r.pick(&[-12, -5, 0, 0, 3, 5, 7, 10, 12, 12, 15]) };
        s.accent = r.chance(if downbeat { 0.5 } else { 0.15 });
        s.tie = r.chance(0.1);
    }
    steps
}

/// Loudness the factory presets are matched to, RMS while a note plays (-25 dB).
const TARGET_RMS: f32 = 0.056;
/// Short notes (plucks) are matched by their peak instead (-9 dB).
const TARGET_PEAK: f32 = 0.355;

/// A volume that puts `patch` near the factory presets' loudness: plays C3 for
/// a moment and measures it. Takes a few milliseconds.
pub fn balanced_volume(patch: &Patch, bank: Arc<WavetableBank>) -> f32 {
    const SR: f32 = 48_000.0;
    let mut engine = Engine::with_bank(SR, bank);
    let mut p = *patch;
    p.volume = 1.0;
    p.arp.on = false;
    engine.handle(Event::Patch(p));
    // Let the volume smoothing settle before the note.
    let mut out = vec![0.0f32; (0.05 * SR) as usize];
    engine.render(&mut out);
    engine.handle(Event::NoteOn { note: 48, velocity: 100 });
    let mut out = vec![0.0f32; (0.8 * SR) as usize];
    engine.render(&mut out);
    let rms = (out.iter().map(|v| v * v).sum::<f32>() / out.len() as f32).sqrt();
    let peak = out.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    if !rms.is_finite() || rms < 1e-6 {
        return patch.volume;
    }
    (TARGET_RMS / rms).min(TARGET_PEAK / peak.max(1e-6)).clamp(0.05, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_what_it_should_and_changes_the_rest() {
        let base = Patch::default();
        let a = randomize(&base, Part::All, 1);
        let b = randomize(&base, Part::All, 2);
        assert_ne!(a, b);
        assert_eq!(a, randomize(&base, Part::All, 1));
        assert_eq!((a.volume, a.play, a.arp, a.bend_range), (base.volume, base.play, base.arp, base.bend_range));
        let fx = randomize(&base, Part::Effects, 3);
        assert_eq!(Patch { fx: base.fx, ..fx }, base);
        let filter = randomize(&base, Part::Filter, 4);
        assert_eq!(Patch { filter: base.filter, filter2: base.filter2, filter_env: base.filter_env, ..filter }, base);
    }

    #[test]
    fn random_sounds_are_finite_and_balanced() {
        let bank = Arc::new(WavetableBank::new());
        for seed in 0..12 {
            let mut p = randomize(&Patch::default(), Part::All, seed);
            p.volume = balanced_volume(&p, bank.clone());
            let mut e = Engine::with_bank(48_000.0, bank.clone());
            e.handle(Event::Patch(p));
            e.handle(Event::NoteOn { note: 48, velocity: 100 });
            let mut out = vec![0.0f32; 24_000];
            e.render(&mut out);
            let peak = out.iter().fold(0.0f32, |m, v| m.max(v.abs()));
            assert!(peak.is_finite() && peak < 1.0, "seed {seed}: peak {peak}");
        }
    }
}
