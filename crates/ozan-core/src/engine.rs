//! Polyphonic engine: voice allocation, events and mixing.

use crate::arp::{Arp, ArpAction};
use crate::effects::Effects;
use crate::lfo::Lfo;
use crate::patch::{Patch, MOD_OCTAVES};
use crate::patch::VoiceMode;
use crate::voice::{Modulation, Start, Voice};
use crate::wavetable::WavetableBank;

use std::sync::Arc;

pub const MAX_VOICES: usize = 16;

/// Mix level per voice, leaves headroom for chords.
const VOICE_GAIN: f32 = 0.2;

/// Voices are rendered in blocks of this many samples; smoothed settings (volume,
/// cutoff) step once per block, about 0.7 ms.
const BLOCK: usize = 32;

/// Time constant of the LFO's smoothing, in seconds.
const LFO_SLEW: f32 = 0.004;

/// Something that happens to the engine, from MIDI or from the user interface.
///
/// The patch travels inline although it makes every event larger: boxing it would
/// mean freeing memory on the audio thread when the old patch is replaced.
#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
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
    /// Cuts reverb and echo tails at once (the panel's panic button).
    ClearEffects,
    /// The host's tempo in beats per minute, for the arpeggiator (the patch's own
    /// tempo is used until one arrives).
    Tempo(f32),
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
    effects: Effects,
    /// Smoothed volume and cutoff (log2 Hz), so turning a knob does not crackle.
    gain: f32,
    log_cutoff: f32,
    /// The second filter's, the same way.
    log_cutoff2: f32,
    /// Pitch bend -1..1.
    bend: f32,
    wheel: f32,
    aftertouch: f32,
    lfo: Lfo,
    lfo2: Lfo,
    lfo2_value: f32,
    /// Smoothed LFO value, used for modulation and shown by the interface.
    lfo_value: f32,
    pedal: bool,
    counter: u64,
    /// Last note played, where glide starts from.
    last_note: Option<u8>,
    /// Keys held in mono/legato, oldest first; the newest sounds.
    stack: [u8; 16],
    stack_len: usize,
    arp: Arp,
    host_bpm: Option<f32>,
    /// For the matrix's random source.
    seed: u32,
    /// Read-only tables shared by all voices (and by the interface for drawing).
    bank: Arc<WavetableBank>,
}

impl Engine {
    /// Builds the wavetables (a fraction of a second); call it outside the audio
    /// callback.
    pub fn new(sample_rate: f32) -> Self {
        Self::with_bank(sample_rate, Arc::new(WavetableBank::new()))
    }

    /// Shares already built wavetables (e.g. with the interface).
    pub fn with_bank(sample_rate: f32, bank: Arc<WavetableBank>) -> Self {
        let patch = Patch::default();
        Self {
            sample_rate,
            voices: Default::default(),
            effects: Effects::new(sample_rate, &patch.fx),
            gain: patch.volume,
            log_cutoff: patch.filter.cutoff.log2(),
            log_cutoff2: patch.filter2.cutoff.max(1.0).log2(),
            patch,
            bend: 0.0,
            wheel: 0.0,
            aftertouch: 0.0,
            lfo: Lfo::default(),
            lfo2: Lfo::default(),
            lfo2_value: 0.0,
            lfo_value: 0.0,
            pedal: false,
            counter: 0,
            last_note: None,
            stack: [0; 16],
            stack_len: 0,
            arp: Arp::default(),
            host_bpm: None,
            seed: 0x9E37_79B9,
            bank,
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
            Event::NoteOn { note, velocity: 0 } | Event::NoteOff { note } if self.patch.arp.on => {
                if let Some(action) = self.arp.key_up(note, &self.patch.arp) {
                    self.arp_act(action);
                }
            }
            Event::NoteOn { note, velocity } if self.patch.arp.on => self.arp.key_down(note, velocity, &self.patch.arp),
            Event::NoteOn { note, velocity: 0 } => self.note_off(note),
            Event::NoteOn { note, velocity } => self.note_on(note, velocity as f32 / 127.0),
            Event::NoteOff { note } => self.note_off(note),
            Event::Tempo(bpm) => self.host_bpm = (bpm > 0.0).then_some(bpm),
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
            Event::ClearEffects => self.effects.clear(),
            Event::AllNotesOff => {
                self.arp.clear();
                self.pedal = false;
                self.stack_len = 0;
                for v in &mut self.voices {
                    v.kill();
                }
            }
            Event::Patch(p) => {
                if p.arp.on != self.patch.arp.on {
                    // Switching the arpeggiator either way lets go of what sounds:
                    // its keys were going to the other path.
                    if let Some(action) = self.arp.clear() {
                        self.arp_act(action);
                    }
                    for i in 0..self.voices.len() {
                        if self.voices[i].held {
                            self.voices[i].release(&p, self.sample_rate);
                        }
                    }
                    self.stack_len = 0;
                }
                self.patch = p;
            }
        }
    }

    fn note_on(&mut self, note: u8, velocity: f32) {
        self.counter += 1;
        let gliding = self.patch.play.glide > 0.0;
        let glide_from = self.last_note.filter(|_| gliding).map(|n| n as f32);
        self.last_note = Some(note);
        let random = self.random();
        if self.patch.play.mode != VoiceMode::Poly {
            self.mono_note_on(note, velocity, random);
            return;
        }
        let start = |retrigger, glide_from| Start { note, velocity, order: self.counter, retrigger, random, glide_from };
        // The same note again reuses its voice (no doubled, phasing notes).
        if let Some(v) = self.voices.iter_mut().find(|v| v.is_active() && v.note == note) {
            v.start(start(true, None));
            return;
        }
        let index = self
            .voices
            .iter()
            .position(|v| !v.is_active())
            .or_else(|| self.steal())
            .unwrap_or(0);
        self.voices[index].start(start(false, glide_from));
    }

    /// Mono and legato play on voice 0 and keep a stack of held keys, so letting go
    /// of the newest key goes back to the one still held.
    fn mono_note_on(&mut self, note: u8, velocity: f32, random: f32) {
        self.stack_remove(note);
        if self.stack_len == self.stack.len() {
            self.stack.copy_within(1.., 0);
            self.stack_len -= 1;
        }
        self.stack[self.stack_len] = note;
        self.stack_len += 1;

        let legato = self.patch.play.mode == VoiceMode::Legato;
        let gliding = self.patch.play.glide > 0.0;
        let v = &mut self.voices[0];
        let busy = v.is_active();
        if legato && busy && (v.held || v.sustained) {
            // Only the pitch moves (at once without glide); envelopes run on.
            v.set_note(note);
            return;
        }
        let glide_from = (busy && gliding).then(|| v.pitch());
        v.start(Start { note, velocity, order: self.counter, retrigger: busy, random, glide_from });
    }

    fn stack_remove(&mut self, note: u8) {
        if let Some(i) = self.stack[..self.stack_len].iter().position(|&n| n == note) {
            self.stack.copy_within(i + 1..self.stack_len, i);
            self.stack_len -= 1;
        }
    }

    fn mono_note_off(&mut self, note: u8) {
        self.stack_remove(note);
        let legato = self.patch.play.mode == VoiceMode::Legato;
        let gliding = self.patch.play.glide > 0.0;
        let counter = self.counter;
        let v = &mut self.voices[0];
        if !v.held || v.note != note {
            return;
        }
        if self.stack_len > 0 {
            // Back to the newest key still held.
            let back = self.stack[self.stack_len - 1];
            self.last_note = Some(back);
            if legato {
                v.set_note(back);
            } else {
                let glide_from = gliding.then(|| v.pitch());
                v.start(Start { note: back, velocity: v.velocity(), order: counter, retrigger: true, random: 0.0, glide_from });
            }
        } else if self.pedal {
            v.held = false;
            v.sustained = true;
        } else {
            v.release(&self.patch, self.sample_rate);
        }
    }

    /// -1..1 (xorshift).
    fn random(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed >> 8) as f32 / (1u32 << 23) as f32 - 1.0
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
        if self.patch.play.mode != VoiceMode::Poly {
            self.mono_note_off(note);
            return;
        }
        for v in self.voices.iter_mut().filter(|v| v.held && v.note == note) {
            if self.pedal {
                v.held = false;
                v.sustained = true;
            } else {
                v.release(&self.patch, self.sample_rate);
            }
        }
    }

    /// Second LFO's value of the last block, -1..1.
    pub fn lfo2_value(&self) -> f32 {
        self.lfo2_value
    }

    /// LFO value of the last block, -1..1.
    pub fn lfo_value(&self) -> f32 {
        self.lfo_value
    }

    /// Modulation for the next `samples`, from the LFO, wheel, aftertouch and bend.
    fn modulation(&mut self, samples: usize) -> Modulation {
        let p = &self.patch;
        let bpm = self.tempo();
        let rate = |l: &crate::LfoParams| if l.sync { 1.0 / l.division.seconds(bpm) } else { l.rate };
        let raw = self.lfo.advance(p.lfo.wave, rate(&p.lfo), samples as f32 / self.sample_rate);
        // A few milliseconds of slew: square and sample & hold steps would
        // otherwise jump the cutoff or level at once, which clicks.
        let k = 1.0 - (-(samples as f32) / (LFO_SLEW * self.sample_rate)).exp();
        self.lfo_value += (raw - self.lfo_value) * k;
        let lfo = self.lfo_value;
        let raw2 = self.lfo2.advance(p.lfo2.wave, rate(&p.lfo2), samples as f32 / self.sample_rate);
        self.lfo2_value += (raw2 - self.lfo2_value) * k;
        let wheel = self.wheel.max(self.aftertouch);
        Modulation {
            pitch: self.bend * p.bend_range + lfo * (p.lfo.pitch + wheel * p.wheel.vibrato),
            cutoff_octaves: (lfo * p.lfo.cutoff + wheel * p.wheel.cutoff) * MOD_OCTAVES,
            // Tremolo dips the level from full down to (1 - depth).
            amp: 1.0 - p.lfo.amp * 0.5 * (1.0 - lfo),
            lfo,
            lfo2: self.lfo2_value,
            wheel: self.wheel,
            aftertouch: self.aftertouch,
        }
    }

    /// Silences effect tails (reverb, echoes) at once.
    pub fn clear_effects(&mut self) {
        self.effects.clear();
    }

    fn arp_act(&mut self, action: ArpAction) {
        match action {
            ArpAction::On { note, velocity } => self.note_on(note, velocity as f32 / 127.0),
            ArpAction::Off { note } => self.note_off(note),
        }
    }

    /// Tempo in beats per minute: the host's, or the arpeggiator's own setting.
    pub fn tempo(&self) -> f32 {
        self.host_bpm.unwrap_or(self.patch.arp.bpm).clamp(20.0, 400.0)
    }

    /// Length of one arpeggiator step in samples.
    fn arp_step(&self) -> f64 {
        let bpm = self.tempo() as f64;
        self.sample_rate as f64 * 60.0 / bpm * self.patch.arp.division.beats()
    }

    /// Lets the arpeggiator act if something is due, then says how many samples
    /// may be rendered before it must act again (at most `limit`).
    fn arp_until_next(&mut self, limit: usize) -> usize {
        if !self.patch.arp.on {
            return limit;
        }
        let step = self.arp_step();
        for action in self.arp.due(&self.patch.arp, step).into_iter().flatten() {
            self.arp_act(action);
        }
        match self.arp.next_due() {
            Some(t) if t.is_finite() => (t.ceil().max(1.0) as usize).min(limit),
            _ => limit,
        }
    }

    /// Fills `left` and `right` (same length) with the next samples.
    pub fn render_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        debug_assert_eq!(left.len(), right.len());
        let mut pos = 0;
        while pos < left.len() {
            let n = self.arp_until_next((left.len() - pos).min(BLOCK));
            self.render_block(&mut left[pos..pos + n], &mut right[pos..pos + n]);
            self.arp.advance(n);
            pos += n;
        }
    }

    /// Fills `out` with the next samples, left and right mixed (for tests and tools).
    pub fn render(&mut self, out: &mut [f32]) {
        let mut pos = 0;
        while pos < out.len() {
            let n = self.arp_until_next((out.len() - pos).min(BLOCK));
            let mut r = [0.0f32; BLOCK];
            let chunk = &mut out[pos..pos + n];
            self.render_block(chunk, &mut r[..n]);
            for (l, r) in chunk.iter_mut().zip(r.iter()) {
                *l = 0.5 * (*l + *r);
            }
            self.arp.advance(n);
            pos += n;
        }
    }

    /// At most [`BLOCK`] samples.
    fn render_block(&mut self, left: &mut [f32], right: &mut [f32]) {
        let n = left.len();
        let mut dry_l = [0.0f32; BLOCK];
        let mut dry_r = [0.0f32; BLOCK];
        let (dry_l, dry_r) = (&mut dry_l[..n], &mut dry_r[..n]);
        // About 10 ms smoothing.
        let k = 1.0 - (-(BLOCK as f32) / (0.01 * self.sample_rate)).exp();
        let target_cutoff = self.patch.filter.cutoff.max(1.0).log2();
        let target_cutoff2 = self.patch.filter2.cutoff.max(1.0).log2();
        self.gain += (self.patch.volume - self.gain) * k;
        self.log_cutoff += (target_cutoff - self.log_cutoff) * k;
        self.log_cutoff2 += (target_cutoff2 - self.log_cutoff2) * k;
        let cutoff = [self.log_cutoff.exp2(), self.log_cutoff2.exp2()];
        let modulation = self.modulation(n);
        for v in &mut self.voices {
            v.render_add(dry_l, dry_r, &self.patch, &modulation, cutoff, self.sample_rate, &self.bank);
        }
        let gain = self.gain;
        let mut fx = self.patch.fx;
        if fx.delay.sync {
            fx.delay.time_ms = (fx.delay.division.seconds(self.tempo()) * 1000.0).clamp(20.0, 2000.0);
        }
        for i in 0..n {
            let (l, r) = self.effects.process(dry_l[i] * VOICE_GAIN, dry_r[i] * VOICE_GAIN, &fx);
            // Gentle saturation instead of hard clipping on loud chords.
            left[i] = (l * gain).tanh();
            right[i] = (r * gain).tanh();
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
    use crate::patch::{LfoParams, OscParams, VoiceMode};

    const SR: f32 = 48_000.0;

    fn rising_crossings(x: &[f32]) -> usize {
        x.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count()
    }

    /// A plain sine through an open filter.
    fn sine_patch() -> Patch {
        let mut p = Patch::default();
        p.osc[0] = OscParams { wave: Waveform::Sine, level: 1.0, ..OscParams::default() };
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
        p.lfo = LfoParams { wave: LfoWave::Square, rate: 1.0, pitch: 12.0, cutoff: 0.0, amp: 0.0, ..LfoParams::default() };
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
        let p = Patch { lfo: LfoParams { wave: LfoWave::Square, rate: 2.0, pitch: 0.0, cutoff: 1.0, amp: 0.0, ..LfoParams::default() }, ..Patch::default() };
        e.handle(Event::Patch(p));
        let cut: Vec<f32> = (0..3000).map(|_| e.modulation(32).cutoff_octaves).collect();
        let biggest_step = cut.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0f32, f32::max);
        // The full swing is 2 × 4 octaves; one 32-sample block may move only part of it.
        assert!(biggest_step < 0.2 * 2.0 * MOD_OCTAVES, "{biggest_step}");
        assert!(cut.iter().cloned().fold(f32::MIN, f32::max) > 0.99 * MOD_OCTAVES);
    }

    #[test]
    fn synced_lfo_follows_the_tempo() {
        let mut e = Engine::new(SR);
        let mut p = Patch {
            lfo: LfoParams { wave: LfoWave::Square, rate: 7.0, cutoff: 1.0, sync: true, division: crate::NoteLength::Eighth, ..LfoParams::default() },
            ..Patch::default()
        };
        e.handle(Event::Patch(p));
        e.handle(Event::Tempo(120.0));
        // An eighth at 120 BPM is 0.25 s: 4 cycles a second, 8 sign changes.
        let blocks = (SR as usize) / 32;
        let cut: Vec<f32> = (0..blocks).map(|_| e.modulation(32).cutoff_octaves).collect();
        let flips = cut.windows(2).filter(|w| (w[0] > 0.0) != (w[1] > 0.0)).count();
        assert!((7..=9).contains(&flips), "{flips}");
        // Without a host, the arpeggiator's tempo counts.
        let mut e = Engine::new(SR);
        p.arp.bpm = 60.0;
        e.handle(Event::Patch(p));
        let cut: Vec<f32> = (0..blocks).map(|_| e.modulation(32).cutoff_octaves).collect();
        let flips = cut.windows(2).filter(|w| (w[0] > 0.0) != (w[1] > 0.0)).count();
        assert!((3..=5).contains(&flips), "{flips}");
    }

    #[test]
    fn synced_delay_repeats_on_the_beat() {
        let mut e = Engine::new(SR);
        let mut p = Patch { amp_env: crate::AdsrParams { attack: 0.001, decay: 0.02, sustain: 0.0, release: 0.01 }, ..Patch::default() };
        p.filter.cutoff = 20_000.0;
        p.fx.delay = crate::DelayParams { on: true, time_ms: 1000.0, feedback: 0.0, mix: 1.0, sync: true, division: crate::NoteLength::Quarter, ..Default::default() };
        e.handle(Event::Patch(p));
        e.handle(Event::Tempo(150.0));
        e.handle(Event::NoteOn { note: 60, velocity: 127 });
        let (l, _) = render_lr(&mut e, SR as usize);
        // A quarter at 150 BPM is 0.4 s; the echo is the loudest thing after 0.2 s.
        let from = (0.2 * SR) as usize;
        let at = from + l[from..].iter().enumerate().max_by(|a, b| a.1.abs().total_cmp(&b.1.abs())).unwrap().0;
        let t = at as f32 / SR;
        assert!((0.39..0.44).contains(&t), "{t}");
    }

    #[test]
    fn second_filter_routes() {
        let level = |routing: crate::FilterRouting| {
            let mut e = Engine::new(SR);
            let p = Patch {
                filter: crate::patch::FilterParams { mode: crate::FilterMode::Lp24, cutoff: 400.0, resonance: 0.0, env_amount: 0.0, key_track: 0.0, drive: 0.0 },
                filter2: crate::Filter2Params { routing, mode: crate::FilterMode::HighPass, cutoff: 4000.0, resonance: 0.0, env_amount: 0.0, key_track: 0.0 },
                ..Patch::default()
            };
            e.handle(Event::Patch(p));
            e.handle(Event::NoteOn { note: 48, velocity: 127 });
            let (l, _) = render_lr(&mut e, 24_000);
            rms(&l[12_000..])
        };
        let (off, serial, parallel) = (level(crate::FilterRouting::Off), level(crate::FilterRouting::Serial), level(crate::FilterRouting::Parallel));
        // Low-pass at 400 Hz then high-pass at 4 kHz leaves almost nothing; side by
        // side the low-pass part stays, mixed in at -3 dB.
        assert!(serial < off * 0.01, "{serial} vs {off}");
        assert!((0.6..0.8).contains(&(parallel / off)), "{parallel} vs {off}");
    }

    fn render_lr(e: &mut Engine, n: usize) -> (Vec<f32>, Vec<f32>) {
        let mut l = vec![0.0; n];
        let mut r = vec![0.0; n];
        e.render_stereo(&mut l, &mut r);
        (l, r)
    }

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
    }

    #[test]
    fn single_copy_stays_mono() {
        let mut e = Engine::new(SR);
        e.handle(Event::NoteOn { note: 60, velocity: 100 });
        let (l, r) = render_lr(&mut e, 9600);
        assert!(rms(&l) > 0.01);
        assert_eq!(l, r);
    }

    #[test]
    fn unison_widens_and_keeps_level() {
        let level = |voices: u8| {
            let mut e = Engine::new(SR);
            let p = Patch { unison: crate::UnisonParams { voices, detune: 30.0, spread: 1.0 }, ..Patch::default() };
            e.handle(Event::Patch(p));
            e.handle(Event::NoteOn { note: 48, velocity: 100 });
            render_lr(&mut e, 4800); // past the attack
            let (l, r) = render_lr(&mut e, 48_000);
            let corr = l.iter().zip(&r).map(|(a, b)| a * b).sum::<f32>()
                / (l.iter().map(|a| a * a).sum::<f32>() * r.iter().map(|b| b * b).sum::<f32>()).sqrt();
            (20.0 * (0.5 * (rms(&l) + rms(&r))).log10(), corr)
        };
        let (one, corr_one) = level(1);
        let (seven, corr_seven) = level(7);
        assert!((corr_one - 1.0).abs() < 1e-6);
        assert!(corr_seven < 0.9, "not wide: {corr_seven}");
        assert!((seven - one).abs() < 3.0, "{one:.1} dB vs {seven:.1} dB");
    }

    #[test]
    fn matrix_slots_act_and_off_slots_do_nothing() {
        use crate::{ModDest, ModSlot, ModSource};
        let play = |slot: ModSlot, velocity: u8| {
            let mut e = Engine::new(SR);
            let mut p = Patch::default();
            p.mods[0] = slot;
            e.handle(Event::Patch(p));
            e.handle(Event::NoteOn { note: 60, velocity });
            render_lr(&mut e, 9600)
        };
        let plain = play(ModSlot::default(), 127);
        // A slot whose source is off changes nothing, whatever its amount.
        assert_eq!(play(ModSlot::new(ModSource::Off, ModDest::Volume, -1.0), 127), plain);
        // Full velocity times -1 on volume: silence.
        let (l, _) = play(ModSlot::new(ModSource::Velocity, ModDest::Volume, -1.0), 127);
        assert!(rms(&l) < 1e-6);
        // Velocity on volume: harder notes louder than without the slot.
        let (soft, _) = play(ModSlot::new(ModSource::Velocity, ModDest::Volume, 1.0), 127);
        assert!(rms(&soft) > 1.5 * rms(&plain.0));
        // Random to pan: each note lands somewhere else.
        let (l, r) = play(ModSlot::new(ModSource::Random, ModDest::Pan, 1.0), 100);
        assert!((rms(&l) - rms(&r)).abs() > 0.001);
    }

    #[test]
    fn matrix_lfo_moves_wavetable_position() {
        use crate::{ModDest, ModSlot, ModSource, Waveform};
        let mut e = Engine::new(SR);
        let mut p = sine_patch();
        p.osc[0].wave = Waveform::Wavetable; // basic table, position 0: a sine
        p.lfo = LfoParams { wave: LfoWave::Square, rate: 1.0, pitch: 0.0, cutoff: 0.0, amp: 0.0, ..LfoParams::default() };
        p.mods[0] = ModSlot::new(ModSource::Lfo, ModDest::Osc1Position, 1.0);
        e.handle(Event::Patch(p));
        e.handle(Event::NoteOn { note: 57, velocity: 100 });
        let (l, _) = render_lr(&mut e, 48_000);
        // First half: LFO +1 → position 1 (square, rich); second half: position 0 (sine).
        let sharpness = |x: &[f32]| x.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0f32, f32::max);
        let square = sharpness(&l[4_800..19_200]);
        let sine = sharpness(&l[28_800..43_200]);
        assert!(square > 3.0 * sine, "{square} vs {sine}");
    }

    fn hz(x: &[f32]) -> f32 {
        rising_crossings(x) as f32 * SR / x.len() as f32
    }

    /// Sine patch with a sustain level below the peak, so a restarted envelope shows.
    fn mono_patch(mode: VoiceMode, glide: f32) -> Patch {
        let mut p = sine_patch();
        p.amp_env = crate::AdsrParams { attack: 0.005, decay: 0.05, sustain: 0.5, release: 0.05 };
        p.play = crate::PlayParams { mode, glide, velocity: 1.0 };
        p
    }

    #[test]
    fn legato_keeps_envelope_and_returns_to_held_key() {
        let mut e = Engine::new(SR);
        e.handle(Event::Patch(mono_patch(VoiceMode::Legato, 0.0)));
        e.handle(Event::NoteOn { note: 57, velocity: 100 }); // 220 Hz
        render_lr(&mut e, 14_400);
        let before = e.voices[0].level();
        e.handle(Event::NoteOn { note: 69, velocity: 100 }); // 440 Hz, A3 still held
        let (l, _) = render_lr(&mut e, 9_600);
        assert!((hz(&l[2_400..]) - 440.0).abs() < 8.0, "{}", hz(&l[2_400..]));
        assert!((e.voices[0].level() - before).abs() < 1e-3, "envelope moved");
        assert_eq!(e.active_voices(), 1);
        e.handle(Event::NoteOff { note: 69 });
        let (l, _) = render_lr(&mut e, 9_600);
        assert!((hz(&l[2_400..]) - 220.0).abs() < 8.0, "{}", hz(&l[2_400..]));
        e.handle(Event::NoteOff { note: 57 });
        render_lr(&mut e, 9_600);
        assert_eq!(e.active_voices(), 0);
    }

    #[test]
    fn mono_restarts_envelope_on_each_key() {
        let mut e = Engine::new(SR);
        e.handle(Event::Patch(mono_patch(VoiceMode::Mono, 0.0)));
        e.handle(Event::NoteOn { note: 57, velocity: 100 });
        render_lr(&mut e, 14_400);
        e.handle(Event::NoteOn { note: 69, velocity: 100 });
        render_lr(&mut e, 96); // into the new attack
        assert!(e.voices[0].level() > 0.55, "{}", e.voices[0].level());
        assert_eq!(e.active_voices(), 1);
    }

    #[test]
    fn glide_slides_to_the_new_note() {
        let mut e = Engine::new(SR);
        let mut p = sine_patch();
        p.play.glide = 0.3;
        e.handle(Event::Patch(p));
        e.handle(Event::NoteOn { note: 57, velocity: 100 });
        render_lr(&mut e, 4_800);
        e.handle(Event::NoteOff { note: 57 });
        e.handle(Event::NoteOn { note: 69, velocity: 100 });
        render_lr(&mut e, 960);
        let v = e.voices.iter().find(|v| v.held && v.note == 69).unwrap();
        assert!(v.pitch() < 63.0, "should still be low: {}", v.pitch());
        render_lr(&mut e, 48_000);
        let v = e.voices.iter().find(|v| v.held && v.note == 69).unwrap();
        assert!((v.pitch() - 69.0).abs() < 0.01, "{}", v.pitch());
    }

    #[test]
    fn velocity_sensitivity_and_noise() {
        let level = |sens: f32, velocity: u8| {
            let mut e = Engine::new(SR);
            let mut p = sine_patch();
            p.play.velocity = sens;
            e.handle(Event::Patch(p));
            e.handle(Event::NoteOn { note: 60, velocity });
            rms(&render_lr(&mut e, 9_600).0)
        };
        assert!((level(0.0, 20) - level(0.0, 127)).abs() < 1e-6);
        assert!(level(1.0, 127) > 4.0 * level(1.0, 20));

        let mut e = Engine::new(SR);
        let mut p = sine_patch();
        p.osc[0].level = 0.0;
        p.noise = 0.5;
        e.handle(Event::Patch(p));
        e.handle(Event::NoteOn { note: 60, velocity: 100 });
        let (l, _) = render_lr(&mut e, 48_000);
        assert!(rms(&l) > 0.01 && hz(&l) > 3_000.0, "{} {}", rms(&l), hz(&l));
    }

    /// (sample, note, started?) for every change of the held notes, one sample at a time.
    fn arp_trace(e: &mut Engine, samples: usize) -> Vec<(usize, u8, bool)> {
        let mut changes = Vec::new();
        let mut before = e.held_notes();
        let (mut l, mut r) = ([0.0f32], [0.0f32]);
        for t in 0..samples {
            e.render_stereo(&mut l, &mut r);
            let now = e.held_notes();
            for n in 0..128u8 {
                let bit = 1u128 << n;
                if (now ^ before) & bit != 0 {
                    changes.push((t, n, now & bit != 0));
                }
            }
            before = now;
        }
        changes
    }

    fn arp_patch() -> Patch {
        let mut p = sine_patch();
        p.arp = crate::ArpParams { on: true, gate: 0.5, bpm: 120.0, ..crate::ArpParams::default() };
        p
    }

    #[test]
    fn arpeggio_steps_on_exact_samples() {
        let mut e = Engine::new(SR);
        e.handle(Event::Patch(arp_patch()));
        for n in [67, 60, 64] {
            e.handle(Event::NoteOn { note: n, velocity: 100 });
        }
        let trace = arp_trace(&mut e, 19_000);
        // 120 BPM sixteenths at 48 kHz: 6000 samples a step, half of it sounding.
        let expect = [(0, 60, true), (3000, 60, false), (6000, 64, true), (9000, 64, false), (12000, 67, true), (15000, 67, false), (18000, 60, true)];
        assert_eq!(trace, expect);
    }

    #[test]
    fn host_tempo_wins_and_switching_off_returns_to_keys() {
        let mut e = Engine::new(SR);
        e.handle(Event::Patch(arp_patch()));
        e.handle(Event::Tempo(60.0)); // half the speed: 12 000 samples a step
        e.handle(Event::NoteOn { note: 60, velocity: 100 });
        e.handle(Event::NoteOn { note: 64, velocity: 100 });
        let trace = arp_trace(&mut e, 13_000);
        assert_eq!(trace.iter().filter(|c| c.2).map(|c| (c.0, c.1)).collect::<Vec<_>>(), [(0, 60), (12000, 64)]);
        // Off: what the arpeggiator played is let go, keys play directly again.
        let mut p = arp_patch();
        p.arp.on = false;
        e.handle(Event::Patch(p));
        assert_eq!(e.held_notes(), 0);
        e.handle(Event::NoteOn { note: 72, velocity: 100 });
        assert_eq!(e.held_notes(), 1u128 << 72);
    }

    #[test]
    fn lfo2_and_mod_envelope_drive_the_matrix() {
        use crate::{ModDest, ModSlot, ModSource};
        // LFO 2, square at 1 Hz, ±1 octave on pitch: 440 Hz, then 110 Hz.
        let mut e = Engine::new(SR);
        let mut p = sine_patch();
        p.lfo2 = LfoParams { wave: LfoWave::Square, rate: 1.0, ..LfoParams::default() };
        p.mods[0] = ModSlot::new(ModSource::Lfo2, ModDest::Pitch, 1.0);
        e.handle(Event::Patch(p));
        e.handle(Event::NoteOn { note: 57, velocity: 100 });
        let (l, _) = render_lr(&mut e, 48_000);
        assert!((hz(&l[4_800..19_200]) - 440.0).abs() < 10.0, "{}", hz(&l[4_800..19_200]));
        assert!((hz(&l[28_800..43_200]) - 110.0).abs() < 10.0, "{}", hz(&l[28_800..43_200]));

        // Mod envelope, 0.5 s attack, an octave on pitch: rises from 220 to 440 Hz.
        let mut e = Engine::new(SR);
        let mut p = sine_patch();
        p.mod_env = crate::AdsrParams { attack: 0.5, decay: 0.1, sustain: 1.0, release: 0.1 };
        p.mods[0] = ModSlot::new(ModSource::ModEnv, ModDest::Pitch, 1.0);
        e.handle(Event::Patch(p));
        e.handle(Event::NoteOn { note: 57, velocity: 100 });
        let (l, _) = render_lr(&mut e, 48_000);
        let start = hz(&l[..2_400]);
        let end = hz(&l[38_400..]);
        assert!(start < 260.0 && (end - 440.0).abs() < 10.0, "{start} → {end}");
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

