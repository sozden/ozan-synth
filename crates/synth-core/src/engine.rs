//! Polyphonic engine: voice allocation, events and mixing.

use crate::patch::Patch;
use crate::voice::Voice;

pub const MAX_VOICES: usize = 16;

/// Pitch-bend range in semitones (each way).
const BEND_RANGE: f32 = 2.0;

/// Mix level per voice, leaves headroom for chords.
const VOICE_GAIN: f32 = 0.2;

/// Voices are rendered in blocks of this many samples; smoothed settings (volume,
/// cutoff) step once per block, about 0.7 ms.
const BLOCK: usize = 32;

/// Something that happens to the engine, from MIDI or from the user interface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    NoteOn { note: u8, velocity: u8 },
    NoteOff { note: u8 },
    SustainPedal(bool),
    /// -1..1, scaled to ±[`BEND_RANGE`] semitones.
    PitchBend(f32),
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
    bend: f32,
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
            Event::PitchBend(b) => self.bend = b.clamp(-1.0, 1.0) * BEND_RANGE,
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
            for v in &mut self.voices {
                v.render_add(block, &self.patch, self.bend, cutoff, self.sample_rate);
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
