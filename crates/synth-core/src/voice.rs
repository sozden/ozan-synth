//! One sounding note: two oscillators → filter → amplifier, each envelope its own.

use crate::envelope::Adsr;
use crate::filter::Filter;
use crate::oscillator::Oscillator;
use crate::patch::{Patch, ENV_OCTAVES};

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
    osc: [Oscillator; 2],
    filter: Filter,
    filter_env: Adsr,
    amp_env: Adsr,
}

impl Voice {
    pub fn is_active(&self) -> bool {
        self.amp_env.is_active()
    }

    /// Amplitude envelope level, used to pick the quietest voice to steal.
    pub fn level(&self) -> f32 {
        self.amp_env.level()
    }

    pub fn start(&mut self, note: u8, velocity: f32, order: u64, retrigger: bool) {
        if !retrigger {
            self.amp_env.kill();
            self.filter_env.kill();
            self.filter.reset();
            // The second oscillator starts a quarter cycle later, so the two do not
            // begin exactly in phase (which would sound louder and flat).
            self.osc[0].reset(0.0);
            self.osc[1].reset(0.25);
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

    /// Adds this voice's output to `out`. `bend` is in semitones, `cutoff` the
    /// (smoothed) base cutoff in Hz.
    pub fn render_add(&mut self, out: &mut [f32], patch: &Patch, bend: f32, cutoff: f32, sample_rate: f32) {
        if !self.is_active() {
            return;
        }
        let note = self.note as f32 + bend;
        let [o1, o2] = &patch.osc;
        let f1 = crate::note_to_hz(note + o1.offset());
        let f2 = crate::note_to_hz(note + o2.offset());
        let f = &patch.filter;
        let tracked = cutoff * (f.key_track * (note - 60.0) / 12.0).exp2();
        let amount = f.env_amount * ENV_OCTAVES;
        for s in out {
            let mut x = 0.0;
            if o1.level > 0.0 {
                x += self.osc[0].next(o1.wave, f1, sample_rate) * o1.level;
            }
            if o2.level > 0.0 {
                x += self.osc[1].next(o2.wave, f2, sample_rate) * o2.level;
            }
            let fe = self.filter_env.next(&patch.filter_env, sample_rate);
            let fc = tracked * (amount * fe).exp2();
            let y = self.filter.process(x, f.mode, fc, f.resonance, sample_rate);
            let ae = self.amp_env.next(&patch.amp_env, sample_rate);
            // Squared envelope sounds more even to the ear than a linear one.
            *s += y * ae * ae * self.velocity;
            if !self.amp_env.is_active() {
                break;
            }
        }
    }
}
