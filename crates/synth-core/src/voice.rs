//! One sounding note: oscillator → amplitude envelope.

use crate::envelope::{Adsr, AdsrParams};
use crate::oscillator::SineOsc;

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
    osc: SineOsc,
    env: Adsr,
}

impl Voice {
    pub fn is_active(&self) -> bool {
        self.env.is_active()
    }

    /// Envelope level, used to pick the quietest voice to steal.
    pub fn level(&self) -> f32 {
        self.env.level()
    }

    pub fn start(&mut self, note: u8, velocity: f32, order: u64, retrigger: bool) {
        if !retrigger {
            self.env.kill();
            self.osc.reset();
        }
        self.note = note;
        self.velocity = velocity;
        self.held = true;
        self.sustained = false;
        self.started = order;
        self.env.gate_on();
    }

    pub fn release(&mut self, p: &AdsrParams, sample_rate: f32) {
        self.held = false;
        self.sustained = false;
        self.env.gate_off(p, sample_rate);
    }

    pub fn kill(&mut self) {
        self.held = false;
        self.sustained = false;
        self.env.kill();
    }

    /// Adds this voice's output to `out`. `bend` is in semitones.
    pub fn render_add(&mut self, out: &mut [f32], p: &AdsrParams, bend: f32, sample_rate: f32) {
        if !self.is_active() {
            return;
        }
        let freq = crate::note_to_hz(self.note as f32 + bend);
        for s in out {
            let env = self.env.next(p, sample_rate);
            // Squared envelope sounds more even to the ear than a linear one.
            *s += self.osc.next(freq, sample_rate) * env * env * self.velocity;
            if !self.env.is_active() {
                break;
            }
        }
    }
}
