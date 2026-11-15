//! Polyphonic engine: voice allocation, events and mixing.

use crate::lfo::Lfo;
use crate::patch::{Patch, MOD_OCTAVES};
use crate::voice::{Modulation, Voice};

pub const MAX_VOICES: usize = 16;

/// Mix level per voice, leaves headroom for chords.
const VOICE_GAIN: f32 = 0.2;

/// Voices are rendered in blocks of this many samples; smoothed settings (volume,
/// cutoff) step once per block, about 0.7 ms.
const BLOCK: usize = 32;

/// Time constant of the LFO's smoothing, in seconds.
const LFO_SLEW: f32 = 0.004;

/// Something that happens to the engine, from MIDI or from the user interface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    NoteOn { note: u8, velocity: u8 },
    NoteOff { note: u8 },
    SustainPedal(bool),
    /// -1..1, scaled to ± the patch's bend range.
    PitchBend(f32),
    /// Mod wheel (CC 1), 0..1.
    ModWheel(f32),
    /// Channel pressure, 0..1; acts like the mod wheel.
    Aftertouch(f32),
    /// MIDI program change; handled by the application, not the engine.
    ProgramChange(u8),
    AllNotesOff,
    /// New sound settings.
    Patch(Patch),
}

pub struct Engine {
    sample_rate: f32,
    voices: [Voice; MAX_VOICES],
    patch: Patch,
    /// Smoothed volume and cutoff (log2 Hz), so turning a knob does not crackle.
    gain: f32,
    log_cutoff: f32,
    /// Pitch bend -1..1.
    bend: f32,
    wheel: f32,
    aftertouch: f32,
    lfo: Lfo,
    /// Smoothed LFO value, used for modulation and shown by the interface.
    lfo_value: f32,
    pedal: bool,
    counter: u64,
}

impl Engine {
    pub fn new(sample_rate: f32) -> Self {
        let patch = Patch::default();
        Self {
            sample_rate,
            voices: Default::default(),
            gain: patch.volume,
            log_cutoff: patch.filter.cutoff.log2(),
            patch,
            bend: 0.0,
            wheel: 0.0,
            aftertouch: 0.0,
            lfo: Lfo::default(),
            lfo_value: 0.0,
            pedal: false,
            counter: 0,
        }
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| v.is_active()).count()
    }

    /// Notes whose key is held (or kept by the pedal), one bit per MIDI note.
    pub fn held_notes(&self) -> u128 {
        self.voices
            .iter()
            .filter(|v| v.is_active() && (v.held || v.sustained))
            .fold(0, |mask, v| mask | 1u128 << (v.note & 0x7F))
    }

    pub fn handle(&mut self, event: Event) {
        match event {
            Event::NoteOn { note, velocity: 0 } => self.note_off(note),
            Event::NoteOn { note, velocity } => self.note_on(note, velocity as f32 / 127.0),
            Event::NoteOff { note } => self.note_off(note),
            Event::SustainPedal(down) => {
                self.pedal = down;
                if !down {
                    for v in self.voices.iter_mut().filter(|v| v.sustained) {
                        v.release(&self.patch, self.sample_rate);
                    }
                }
            }
            Event::PitchBend(b) => self.bend = b.clamp(-1.0, 1.0),
            Event::ModWheel(v) => self.wheel = v.clamp(0.0, 1.0),
            Event::Aftertouch(v) => self.aftertouch = v.clamp(0.0, 1.0),
            Event::ProgramChange(_) => {}
            Event::AllNotesOff => {
                self.pedal = false;
                for v in &mut self.voices {
                    v.kill();
                }
            }
            Event::Patch(p) => self.patch = p,
        }
    }

    fn note_on(&mut self, note: u8, velocity: f32) {
        self.counter += 1;
        // The same note again reuses its voice (no doubled, phasing notes).
        if let Some(v) = self.voices.iter_mut().find(|v| v.is_active() && v.note == note) {
            v.start(note, velocity, self.counter, true);
            return;
        }
        let index = self
            .voices
            .iter()
            .position(|v| !v.is_active())
            .or_else(|| self.steal())
            .unwrap_or(0);
        self.voices[index].start(note, velocity, self.counter, false);
    }

    /// Voice to take over when all are busy: the quietest released voice, otherwise
    /// the oldest one.
    fn steal(&self) -> Option<usize> {
        let released = self
            .voices
            .iter()
            .enumerate()
            .filter(|(_, v)| !v.held && !v.sustained)
            .min_by(|a, b| a.1.level().total_cmp(&b.1.level()));
        released
            .or_else(|| self.voices.iter().enumerate().min_by_key(|(_, v)| v.started))
            .map(|(i, _)| i)
    }

    fn note_off(&mut self, note: u8) {
        for v in self.voices.iter_mut().filter(|v| v.held && v.note == note) {
            if self.pedal {
                v.held = false;
                v.sustained = true;
            } else {
                v.release(&self.patch, self.sample_rate);
            }
        }
    }

    /// LFO value of the last block, -1..1.
    pub fn lfo_value(&self) -> f32 {
        self.lfo_value
    }

    /// Modulation for the next `samples`, from the LFO, wheel, aftertouch and bend.
    fn modulation(&mut self, samples: usize) -> Modulation {
        let p = &self.patch;
        let raw = self.lfo.advance(p.lfo.wave, p.lfo.rate, samples as f32 / self.sample_rate);
        // A few milliseconds of slew: square and sample & hold steps would
        // otherwise jump the cutoff or level at once, which clicks.
        let k = 1.0 - (-(samples as f32) / (LFO_SLEW * self.sample_rate)).exp();
        self.lfo_value += (raw - self.lfo_value) * k;
        let lfo = self.lfo_value;
        let wheel = self.wheel.max(self.aftertouch);
        Modulation {
            pitch: self.bend * p.bend_range + lfo * (p.lfo.pitch + wheel * p.wheel.vibrato),
            cutoff_octaves: (lfo * p.lfo.cutoff + wheel * p.wheel.cutoff) * MOD_OCTAVES,
            // Tremolo dips the level from full down to (1 - depth).
            amp: 1.0 - p.lfo.amp * 0.5 * (1.0 - lfo),
        }
    }

    /// Fills `out` (mono) with the next samples.
    pub fn render(&mut self, out: &mut [f32]) {
        out.fill(0.0);
        // About 10 ms smoothing.
        let k = 1.0 - (-(BLOCK as f32) / (0.01 * self.sample_rate)).exp();
        let target_cutoff = self.patch.filter.cutoff.max(1.0).log2();
        for block in out.chunks_mut(BLOCK) {
            self.gain += (self.patch.volume - self.gain) * k;
            self.log_cutoff += (target_cutoff - self.log_cutoff) * k;
            let cutoff = self.log_cutoff.exp2();
            let modulation = self.modulation(block.len());
            for v in &mut self.voices {
                v.render_add(block, &self.patch, &modulation, cutoff, self.sample_rate);
            }
            let gain = VOICE_GAIN * self.gain;
            for s in block {
                // Gentle saturation instead of hard clipping on loud chords.
                *s = (*s * gain).tanh();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    fn on(e: &mut Engine, note: u8) {
        e.handle(Event::NoteOn { note, velocity: 100 });
    }

    fn peak(e: &mut Engine, n: usize) -> f32 {
        let mut buf = vec![0.0; n];
        e.render(&mut buf);
        buf.iter().fold(0.0f32, |m, s| m.max(s.abs()))
    }

    #[test]
    fn silent_without_notes() {
        let mut e = Engine::new(SR);
        assert_eq!(peak(&mut e, 512), 0.0);
    }

    #[test]
    fn note_sounds_and_fades_out() {
        let mut e = Engine::new(SR);
        on(&mut e, 69);
        assert!(peak(&mut e, 4800) > 0.01);
        e.handle(Event::NoteOff { note: 69 });
        peak(&mut e, SR as usize); // default release is 0.4 s
        assert_eq!(e.active_voices(), 0);
        assert_eq!(peak(&mut e, 512), 0.0);
    }

    #[test]
    fn velocity_zero_is_note_off() {
        let mut e = Engine::new(SR);
        on(&mut e, 60);
        e.handle(Event::NoteOn { note: 60, velocity: 0 });
        assert_eq!(e.held_notes(), 0);
    }

    #[test]
    fn same_note_reuses_voice() {
        let mut e = Engine::new(SR);
        on(&mut e, 60);
        on(&mut e, 60);
        assert_eq!(e.active_voices(), 1);
    }

    #[test]
    fn steals_oldest_when_full() {
        let mut e = Engine::new(SR);
        for n in 0..MAX_VOICES as u8 {
            on(&mut e, 40 + n);
        }
        on(&mut e, 100);
        assert_eq!(e.active_voices(), MAX_VOICES);
        let held = e.held_notes();
        assert_eq!(held & 1 << 40, 0, "oldest note should be stolen");
        assert_ne!(held & 1 << 100, 0);
    }

    #[test]
    fn pedal_keeps_notes_until_lifted() {
        let mut e = Engine::new(SR);
        e.handle(Event::SustainPedal(true));
        on(&mut e, 64);
        e.handle(Event::NoteOff { note: 64 });
        peak(&mut e, SR as usize);
        assert_eq!(e.active_voices(), 1);
        e.handle(Event::SustainPedal(false));
        peak(&mut e, SR as usize);
        assert_eq!(e.active_voices(), 0);
    }

    #[test]
    fn full_chord_does_not_clip() {
        let mut e = Engine::new(SR);
        e.handle(Event::Patch(Patch { volume: 1.0, ..Patch::default() }));
        for n in 0..MAX_VOICES as u8 {
            e.handle(Event::NoteOn { note: 48 + n, velocity: 127 });
        }
        // Saturation keeps even 16 loud voices inside the -1..1 range.
        let p = peak(&mut e, 48_000);
        assert!(p.is_finite() && p < 1.0, "{p}");
    }
}

#[cfg(test)]
mod modulation_tests {
    use super::*;
    use crate::lfo::LfoWave;
    use crate::oscillator::Waveform;
    use crate::patch::{LfoParams, OscParams};

    const SR: f32 = 48_000.0;

    fn rising_crossings(x: &[f32]) -> usize {
        x.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count()
    }

    /// A plain sine through an open filter.
    fn sine_patch() -> Patch {
        let mut p = Patch::default();
        p.osc[0] = OscParams { wave: Waveform::Sine, octave: 0, semitones: 0, fine: 0.0, level: 1.0 };
        p.osc[1].level = 0.0;
        p.filter.cutoff = 20_000.0;
        p.filter.env_amount = 0.0;
        p.amp_env.sustain = 1.0;
        p
    }

    #[test]
    fn square_lfo_vibrato_jumps_an_octave() {
        let mut e = Engine::new(SR);
        let mut p = sine_patch();
        p.lfo = LfoParams { wave: LfoWave::Square, rate: 1.0, pitch: 12.0, cutoff: 0.0, amp: 0.0 };
        e.handle(Event::Patch(p));
        e.handle(Event::NoteOn { note: 57, velocity: 100 }); // 220 Hz
        let mut buf = vec![0.0; SR as usize];
        e.render(&mut buf);
        // First half of the LFO cycle is +1 (an octave up), the second half -1.
        let high = rising_crossings(&buf[4_800..19_200]) as f32 / 0.3;
        let low = rising_crossings(&buf[28_800..43_200]) as f32 / 0.3;
        assert!((high - 440.0).abs() < 10.0, "{high}");
        assert!((low - 110.0).abs() < 10.0, "{low}");
    }

    #[test]
    fn bend_uses_patch_range_and_wheel_adds_vibrato() {
        let mut e = Engine::new(SR);
        let mut p = sine_patch();
        p.bend_range = 12.0;
        e.handle(Event::Patch(p));
        e.handle(Event::PitchBend(1.0));
        e.handle(Event::NoteOn { note: 57, velocity: 100 });
        let mut buf = vec![0.0; 24_000];
        e.render(&mut buf);
        let f = rising_crossings(&buf[4_800..]) as f32 / 0.4;
        assert!((f - 440.0).abs() < 10.0, "{f}");

        // Without LFO depth the wheel alone adds the patch's vibrato.
        assert_eq!(e.modulation(32).pitch, 12.0);
        e.handle(Event::ModWheel(1.0));
        let pitches: Vec<f32> = (0..3000).map(|_| e.modulation(32).pitch).collect();
        let spread = pitches.iter().cloned().fold(f32::MIN, f32::max) - pitches.iter().cloned().fold(f32::MAX, f32::min);
        assert!((spread - 2.0 * p.wheel.vibrato).abs() < 0.05, "{spread}");
    }

    #[test]
    fn stepped_lfo_does_not_jump_at_once() {
        let mut e = Engine::new(SR);
        let p = Patch { lfo: LfoParams { wave: LfoWave::Square, rate: 2.0, pitch: 0.0, cutoff: 1.0, amp: 0.0 }, ..Patch::default() };
        e.handle(Event::Patch(p));
        let cut: Vec<f32> = (0..3000).map(|_| e.modulation(32).cutoff_octaves).collect();
        let biggest_step = cut.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0f32, f32::max);
        // The full swing is 2 × 4 octaves; one 32-sample block may move only part of it.
        assert!(biggest_step < 0.2 * 2.0 * MOD_OCTAVES, "{biggest_step}");
        assert!(cut.iter().cloned().fold(f32::MIN, f32::max) > 0.99 * MOD_OCTAVES);
    }

    #[test]
    fn tremolo_stays_between_full_and_depth() {
        let mut e = Engine::new(SR);
        let mut p = Patch::default();
        p.lfo.amp = 0.4;
        e.handle(Event::Patch(p));
        let amps: Vec<f32> = (0..3000).map(|_| e.modulation(32).amp).collect();
        let max = amps.iter().cloned().fold(f32::MIN, f32::max);
        let min = amps.iter().cloned().fold(f32::MAX, f32::min);
        assert!(max <= 1.0 && max > 0.99 && (min - 0.6).abs() < 0.01, "{min}..{max}");
    }
}
