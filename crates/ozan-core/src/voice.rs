//! One sounding note: two oscillators (each up to [`MAX_UNISON`] detuned copies)
//! → filter → amplifier, in stereo, with the modulation matrix applied per block.

use std::f32::consts::{FRAC_PI_4, SQRT_2};

use crate::envelope::Adsr;
use crate::filter::Filter;
use crate::modmatrix::{ModDest, ModSource};
use crate::oscillator::{Oscillator, Waveform};
use crate::patch::{Patch, ENV_OCTAVES};
use crate::wavetable::{Wavetable, WavetableBank};

/// Most copies per oscillator.
pub const MAX_UNISON: usize = 7;

/// Cutoff range of the matrix at full amount, in octaves.
const MATRIX_OCTAVES: f32 = 6.0;

/// Modulation shared by all voices for one block.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Modulation {
    /// Semitones added to the pitch (bend, vibrato).
    pub pitch: f32,
    /// Octaves added to the cutoff.
    pub cutoff_octaves: f32,
    /// Level factor (tremolo), 0..1.
    pub amp: f32,
    /// Matrix sources that are the same for every voice.
    pub lfo: f32,
    pub wheel: f32,
    pub aftertouch: f32,
}

impl Default for Modulation {
    fn default() -> Self {
        Self { pitch: 0.0, cutoff_octaves: 0.0, amp: 1.0, lfo: 0.0, wheel: 0.0, aftertouch: 0.0 }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Voice {
    /// MIDI note the voice was last started with.
    pub note: u8,
    /// The key is still pressed (gate on).
    pub held: bool,
    /// The key was released while the sustain pedal was down; released with the pedal.
    pub sustained: bool,
    /// Order in which voices were started, for stealing the oldest.
    pub started: u64,
    velocity: f32,
    /// Matrix "random" source, -1..1, new for each note.
    random: f32,
    osc: [[Oscillator; MAX_UNISON]; 2],
    /// Left and right; the right one runs only when the voice is not mono.
    filter: [Filter; 2],
    filter_env: Adsr,
    amp_env: Adsr,
    /// Wavetable positions and gain at the end of the previous block: the matrix
    /// changes them once per block, and jumping there would click, so each block
    /// ramps from these to its new values. `None` right after a note starts.
    ramp_from: Option<([f32; 2], f32)>,
}

/// Per-block settings of one oscillator copy.
#[derive(Clone, Copy, Default)]
struct CopySetup {
    freq: f32,
    level: usize,
}

impl Voice {
    pub fn is_active(&self) -> bool {
        self.amp_env.is_active()
    }

    /// Amplitude envelope level, used to pick the quietest voice to steal.
    pub fn level(&self) -> f32 {
        self.amp_env.level()
    }

    pub fn start(&mut self, note: u8, velocity: f32, order: u64, retrigger: bool, random: f32) {
        if !retrigger {
            self.amp_env.kill();
            self.filter_env.kill();
            for f in &mut self.filter {
                f.reset();
            }
            // Copies start at different points of the cycle, so they do not begin in
            // phase (louder, then a slow sweep). Copy 0 keeps the classic phases.
            for (k, row) in self.osc.iter_mut().enumerate() {
                for (i, o) in row.iter_mut().enumerate() {
                    o.reset(0.25 * k as f32 + 0.618_034 * i as f32);
                }
            }
            self.random = random;
            self.ramp_from = None;
        }
        self.note = note;
        self.velocity = velocity;
        self.held = true;
        self.sustained = false;
        self.started = order;
        self.amp_env.gate_on();
        self.filter_env.gate_on();
    }

    pub fn release(&mut self, patch: &Patch, sample_rate: f32) {
        self.held = false;
        self.sustained = false;
        self.amp_env.gate_off(&patch.amp_env, sample_rate);
        self.filter_env.gate_off(&patch.filter_env, sample_rate);
    }

    pub fn kill(&mut self) {
        self.held = false;
        self.sustained = false;
        self.amp_env.kill();
        self.filter_env.kill();
    }

    /// Sum of the matrix slots for each destination.
    fn matrix(&self, patch: &Patch, m: &Modulation) -> [f32; ModDest::COUNT] {
        let mut d = [0.0; ModDest::COUNT];
        for slot in &patch.mods {
            let v = match slot.source {
                ModSource::Off => continue,
                ModSource::Lfo => m.lfo,
                ModSource::FilterEnv => self.filter_env.level(),
                ModSource::AmpEnv => self.amp_env.level(),
                ModSource::Velocity => self.velocity,
                ModSource::Note => (self.note as f32 - 60.0) / 60.0,
                ModSource::Wheel => m.wheel,
                ModSource::Aftertouch => m.aftertouch,
                ModSource::Random => self.random,
            };
            d[slot.dest.index()] += slot.amount * v;
        }
        d
    }

    /// Adds this voice's output to `left` and `right`. `cutoff` is the (smoothed)
    /// base cutoff in Hz.
    #[allow(clippy::too_many_arguments)]
    pub fn render_add(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        patch: &Patch,
        m: &Modulation,
        cutoff: f32,
        sample_rate: f32,
        bank: &WavetableBank,
    ) {
        if !self.is_active() {
            return;
        }
        let d = self.matrix(patch, m);
        let at = |dest: ModDest| d[dest.index()];

        let u = &patch.unison;
        let n = u.voices.clamp(1, MAX_UNISON as u8) as usize;
        let detune = (u.detune + at(ModDest::UnisonDetune) * 50.0).max(0.0);
        let pan_shift = at(ModDest::Pan);
        let note = self.note as f32 + m.pitch + at(ModDest::Pitch) * 12.0;
        let osc_pitch = [at(ModDest::Osc1Pitch), at(ModDest::Osc2Pitch)];
        let levels = [
            (patch.osc[0].level + at(ModDest::Osc1Level)).clamp(0.0, 1.0),
            (patch.osc[1].level + at(ModDest::Osc2Level)).clamp(0.0, 1.0),
        ];
        let positions = [
            (patch.osc[0].position + at(ModDest::Osc1Position)).clamp(0.0, 1.0),
            (patch.osc[1].position + at(ModDest::Osc2Position)).clamp(0.0, 1.0),
        ];
        let tables: [&Wavetable; 2] = [bank.get(patch.osc[0].table), bank.get(patch.osc[1].table)];

        // Per copy: pitch and pan. Pan gains are 1 in the middle, so a single
        // centred copy sounds exactly as it did before unison existed.
        let mut copies = [[CopySetup::default(); MAX_UNISON]; 2];
        let mut gl = [0.0f32; MAX_UNISON];
        let mut gr = [0.0f32; MAX_UNISON];
        for i in 0..n {
            let spread_pos = if n > 1 { 2.0 * i as f32 / (n - 1) as f32 - 1.0 } else { 0.0 };
            for (k, row) in copies.iter_mut().enumerate() {
                let freq = crate::note_to_hz(note + patch.osc[k].offset() + osc_pitch[k] * 12.0 + spread_pos * detune / 100.0);
                row[i] = CopySetup { freq, level: Wavetable::level_for(freq / sample_rate) };
            }
            let pan = (spread_pos * u.spread.clamp(0.0, 1.0) + pan_shift).clamp(-1.0, 1.0);
            let a = (pan + 1.0) * FRAC_PI_4;
            gl[i] = a.cos() * SQRT_2;
            gr[i] = a.sin() * SQRT_2;
        }
        let stereo = gl[..n].iter().zip(&gr[..n]).any(|(l, r)| (l - r).abs() > 1e-6);
        // Equal loudness whatever the number of copies (they add up like noise).
        let norm = 1.0 / (n as f32).sqrt();

        let f = &patch.filter;
        let resonance = (f.resonance + at(ModDest::Resonance)).clamp(0.0, 1.0);
        let tracked = cutoff * (f.key_track * (self.note as f32 - 60.0) / 12.0 + m.cutoff_octaves + at(ModDest::Cutoff) * MATRIX_OCTAVES).exp2();
        let amount = f.env_amount * ENV_OCTAVES;
        let gain = self.velocity * m.amp * (1.0 + at(ModDest::Volume)).clamp(0.0, 2.0);

        let (start_pos, start_gain) = self.ramp_from.unwrap_or((positions, gain));
        self.ramp_from = Some((positions, gain));
        let steps = left.len().max(1) as f32;

        for (s, (l_out, r_out)) in left.iter_mut().zip(right.iter_mut()).enumerate() {
            let t = (s + 1) as f32 / steps;
            let pos = [start_pos[0] + (positions[0] - start_pos[0]) * t, start_pos[1] + (positions[1] - start_pos[1]) * t];
            let gain = start_gain + (gain - start_gain) * t;
            let (mut l, mut r) = (0.0, 0.0);
            for i in 0..n {
                let mut x = 0.0;
                for k in 0..2 {
                    if levels[k] <= 0.0 {
                        continue;
                    }
                    let o = &patch.osc[k];
                    let c = copies[k][i];
                    let v = if o.wave == Waveform::Wavetable {
                        self.osc[k][i].next_table(tables[k], pos[k], c.level, c.freq / sample_rate)
                    } else {
                        self.osc[k][i].next(o.wave, c.freq, sample_rate)
                    };
                    x += v * levels[k];
                }
                l += x * gl[i];
                r += x * gr[i];
            }
            let fe = self.filter_env.next(&patch.filter_env, sample_rate);
            let (g, k) = Filter::coefficients(tracked * (amount * fe).exp2(), resonance, sample_rate);
            let yl = self.filter[0].process_with(l * norm, f.mode, g, k);
            let yr = if stereo { self.filter[1].process_with(r * norm, f.mode, g, k) } else { yl };
            let ae = self.amp_env.next(&patch.amp_env, sample_rate);
            // Squared envelope sounds more even to the ear than a linear one.
            let e = ae * ae * gain;
            *l_out += yl * e;
            *r_out += yr * e;
            if !self.amp_env.is_active() {
                break;
            }
        }
    }
}
