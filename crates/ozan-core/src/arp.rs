//! Arpeggiator: plays the held keys one after another in time.
//!
//! The engine asks how many samples remain until the next step or note end
//! ([`Arp::next_due`]), renders up to there, then lets the arpeggiator act
//! ([`Arp::due`]), so steps land on the exact sample.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ArpMode {
    #[default]
    Up,
    Down,
    /// Up then down, the top and bottom notes once each.
    UpDown,
    Random,
    /// In the order the keys were pressed.
    Played,
}

impl ArpMode {
    pub const ALL: [ArpMode; 5] = [ArpMode::Up, ArpMode::Down, ArpMode::UpDown, ArpMode::Random, ArpMode::Played];
}

/// Length of one step.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ArpDivision {
    Quarter,
    Eighth,
    EighthTriplet,
    #[default]
    Sixteenth,
    SixteenthTriplet,
    ThirtySecond,
}

impl ArpDivision {
    pub const ALL: [ArpDivision; 6] = [
        ArpDivision::Quarter,
        ArpDivision::Eighth,
        ArpDivision::EighthTriplet,
        ArpDivision::Sixteenth,
        ArpDivision::SixteenthTriplet,
        ArpDivision::ThirtySecond,
    ];

    /// Length in beats (quarter notes).
    pub fn beats(self) -> f64 {
        match self {
            ArpDivision::Quarter => 1.0,
            ArpDivision::Eighth => 0.5,
            ArpDivision::EighthTriplet => 1.0 / 3.0,
            ArpDivision::Sixteenth => 0.25,
            ArpDivision::SixteenthTriplet => 1.0 / 6.0,
            ArpDivision::ThirtySecond => 0.125,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ArpDivision::Quarter => "1/4",
            ArpDivision::Eighth => "1/8",
            ArpDivision::EighthTriplet => "1/8T",
            ArpDivision::Sixteenth => "1/16",
            ArpDivision::SixteenthTriplet => "1/16T",
            ArpDivision::ThirtySecond => "1/32",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(default))]
pub struct ArpParams {
    pub on: bool,
    pub mode: ArpMode,
    pub division: ArpDivision,
    /// Octaves the pattern spans, 1..4.
    pub octaves: u8,
    /// Note length as a share of the step, 0.05..1.
    pub gate: f32,
    /// Keeps playing after the keys are let go, until a new chord is pressed.
    pub hold: bool,
    /// Tempo in beats per minute when the host does not give one (40..240).
    pub bpm: f32,
}

impl Default for ArpParams {
    fn default() -> Self {
        Self { on: false, mode: ArpMode::Up, division: ArpDivision::Sixteenth, octaves: 1, gate: 0.5, hold: false, bpm: 120.0 }
    }
}

/// What the arpeggiator wants the engine to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArpAction {
    On { note: u8, velocity: u8 },
    Off { note: u8 },
}

const MAX_KEYS: usize = 16;

#[derive(Clone, Debug)]
pub struct Arp {
    /// Held (or held-over) keys with their velocities, in the order pressed.
    keys: [(u8, u8); MAX_KEYS],
    len: usize,
    /// Keys physically down; with hold, the pattern outlives them.
    down: usize,
    /// Samples until the next step and until the sounding note ends.
    clock: f64,
    gate_left: f64,
    playing: Option<u8>,
    step: u32,
    seed: u32,
}

impl Default for Arp {
    fn default() -> Self {
        Self { keys: [(0, 0); MAX_KEYS], len: 0, down: 0, clock: 0.0, gate_left: 0.0, playing: None, step: 0, seed: 0x2545_F491 }
    }
}

impl Arp {
    pub fn key_down(&mut self, note: u8, velocity: u8, p: &ArpParams) {
        // With hold, a key pressed after all were let go starts a new chord.
        if p.hold && self.down == 0 {
            self.len = 0;
        }
        self.down += 1;
        if self.keys[..self.len].iter().any(|k| k.0 == note) {
            return;
        }
        if self.len == 0 {
            // The first key starts the pattern at once.
            self.clock = 0.0;
            self.step = 0;
        }
        if self.len < MAX_KEYS {
            self.keys[self.len] = (note, velocity);
            self.len += 1;
        }
    }

    /// Returns the note to silence when the pattern stops.
    pub fn key_up(&mut self, note: u8, p: &ArpParams) -> Option<ArpAction> {
        self.down = self.down.saturating_sub(1);
        if p.hold {
            return None;
        }
        if let Some(i) = self.keys[..self.len].iter().position(|k| k.0 == note) {
            self.keys.copy_within(i + 1..self.len, i);
            self.len -= 1;
        }
        if self.len == 0 { self.stop() } else { None }
    }

    /// Forgets every key; returns the note to silence, if one sounds.
    pub fn clear(&mut self) -> Option<ArpAction> {
        self.len = 0;
        self.down = 0;
        self.stop()
    }

    fn stop(&mut self) -> Option<ArpAction> {
        self.playing.take().map(|note| ArpAction::Off { note })
    }

    /// Samples until something is due, or `None` when idle.
    pub fn next_due(&self) -> Option<f64> {
        let step = (self.len > 0).then_some(self.clock);
        let gate = self.playing.map(|_| self.gate_left);
        match (step, gate) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// Time passes.
    pub fn advance(&mut self, samples: usize) {
        self.clock -= samples as f64;
        self.gate_left -= samples as f64;
    }

    /// Actions due now (at most an end and a start). `step_len` is in samples.
    pub fn due(&mut self, p: &ArpParams, step_len: f64) -> [Option<ArpAction>; 2] {
        let mut out = [None, None];
        if self.playing.is_some() && self.gate_left <= 0.0 {
            out[0] = self.stop();
        }
        if self.len > 0 && self.clock <= 0.0 {
            if out[0].is_none() {
                out[0] = self.stop();
            }
            let (note, velocity) = self.pick(p);
            self.playing = Some(note);
            out[1] = Some(ArpAction::On { note, velocity });
            self.gate_left = (step_len * p.gate.clamp(0.05, 1.0) as f64).max(1.0);
            // A gate of 100% runs up to the next step, which ends the note itself.
            if p.gate >= 1.0 {
                self.gate_left = f64::INFINITY;
            }
            self.clock += step_len.max(1.0);
            if self.clock <= 0.0 {
                self.clock = step_len.max(1.0);
            }
            self.step = self.step.wrapping_add(1);
        }
        out
    }

    /// The note for the current step.
    fn pick(&mut self, p: &ArpParams) -> (u8, u8) {
        let mut sorted = [(0u8, 0u8); MAX_KEYS];
        sorted[..self.len].copy_from_slice(&self.keys[..self.len]);
        let order = &mut sorted[..self.len];
        if p.mode != ArpMode::Played {
            order.sort_unstable_by_key(|k| k.0);
        }
        let octaves = p.octaves.clamp(1, 4) as usize;
        let total = self.len * octaves;
        let step = self.step as usize;
        let index = match p.mode {
            ArpMode::Up | ArpMode::Played => step % total,
            ArpMode::Down => total - 1 - step % total,
            ArpMode::UpDown => {
                let cycle = (2 * total).saturating_sub(2).max(1);
                let k = step % cycle;
                if k < total { k } else { cycle - k }
            }
            ArpMode::Random => {
                self.seed ^= self.seed << 13;
                self.seed ^= self.seed >> 17;
                self.seed ^= self.seed << 5;
                self.seed as usize % total
            }
        };
        let (note, velocity) = order[index % self.len];
        ((note as usize + 12 * (index / self.len)).min(127) as u8, velocity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Notes started over `steps` steps.
    fn run(p: &ArpParams, keys: &[u8], steps: usize) -> Vec<u8> {
        let mut a = Arp::default();
        for &k in keys {
            a.key_down(k, 100, p);
        }
        let mut notes = Vec::new();
        for _ in 0..steps {
            for act in a.due(p, 100.0).into_iter().flatten() {
                if let ArpAction::On { note, .. } = act {
                    notes.push(note);
                }
            }
            a.advance(100);
        }
        notes
    }

    #[test]
    fn patterns() {
        let p = |mode, octaves| ArpParams { on: true, mode, octaves, ..ArpParams::default() };
        assert_eq!(run(&p(ArpMode::Up, 1), &[64, 60, 67], 4), [60, 64, 67, 60]);
        assert_eq!(run(&p(ArpMode::Down, 1), &[64, 60, 67], 4), [67, 64, 60, 67]);
        assert_eq!(run(&p(ArpMode::Played, 1), &[64, 60, 67], 4), [64, 60, 67, 64]);
        assert_eq!(run(&p(ArpMode::Up, 2), &[60, 64], 5), [60, 64, 72, 76, 60]);
        assert_eq!(run(&p(ArpMode::UpDown, 1), &[60, 64, 67], 6), [60, 64, 67, 64, 60, 64]);
        let random = run(&p(ArpMode::Random, 2), &[60, 64, 67], 50);
        assert!(random.iter().all(|n| [60, 64, 67, 72, 76, 79].contains(n)));
    }

    #[test]
    fn gate_ends_notes_and_release_stops() {
        let p = ArpParams { on: true, gate: 0.25, ..ArpParams::default() };
        let mut a = Arp::default();
        a.key_down(60, 90, &p);
        assert_eq!(a.due(&p, 400.0), [None, Some(ArpAction::On { note: 60, velocity: 90 })]);
        assert_eq!(a.next_due(), Some(100.0));
        a.advance(100);
        assert_eq!(a.due(&p, 400.0), [Some(ArpAction::Off { note: 60 }), None]);
        assert_eq!(a.next_due(), Some(300.0));
        a.advance(300);
        a.due(&p, 400.0);
        assert_eq!(a.key_up(60, &p), Some(ArpAction::Off { note: 60 }));
        assert_eq!(a.next_due(), None);
    }

    #[test]
    fn hold_keeps_playing_until_a_new_chord() {
        let p = ArpParams { on: true, hold: true, ..ArpParams::default() };
        let mut a = Arp::default();
        a.key_down(60, 100, &p);
        a.key_down(64, 100, &p);
        assert_eq!(a.key_up(60, &p), None);
        assert_eq!(a.key_up(64, &p), None);
        assert_eq!(a.len, 2);
        // All keys up, then a new key: it replaces the held chord.
        a.key_down(72, 100, &p);
        assert_eq!(&a.keys[..a.len], &[(72, 100)]);
    }
}
