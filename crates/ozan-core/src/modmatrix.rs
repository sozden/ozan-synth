//! Modulation matrix: a few slots, each sending one source to one destination with
//! a signed amount. Evaluated per voice once per block.

/// Number of slots in a patch.
pub const SLOTS: usize = 6;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ModSource {
    #[default]
    Off,
    /// -1..1.
    Lfo,
    /// -1..1.
    Lfo2,
    /// 0..1.
    FilterEnv,
    /// 0..1.
    AmpEnv,
    /// 0..1.
    ModEnv,
    /// 0..1.
    Velocity,
    /// -1 (C-1) .. 0 (C4) .. 1, by note number.
    Note,
    /// 0..1.
    Wheel,
    /// 0..1.
    Aftertouch,
    /// -1..1, a new value for every note.
    Random,
}

impl ModSource {
    pub const ALL: [ModSource; 11] = [
        ModSource::Off,
        ModSource::Lfo,
        ModSource::Lfo2,
        ModSource::FilterEnv,
        ModSource::AmpEnv,
        ModSource::ModEnv,
        ModSource::Velocity,
        ModSource::Note,
        ModSource::Wheel,
        ModSource::Aftertouch,
        ModSource::Random,
    ];
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ModDest {
    /// ±6 octaves at full amount.
    #[default]
    Cutoff,
    /// ±1.
    Resonance,
    /// Both oscillators, ±12 semitones.
    Pitch,
    Osc1Pitch,
    Osc2Pitch,
    /// Wavetable position, ±1.
    Osc1Position,
    Osc2Position,
    /// ±1 added to the level.
    Osc1Level,
    Osc2Level,
    /// Level factor 0..2 (1 ± amount).
    Volume,
    /// ±1, left to right.
    Pan,
    /// ±50 cents of unison detune.
    UnisonDetune,
    /// ±1 added to the filter drive.
    Drive,
    /// Second filter's cutoff, ±6 octaves.
    Cutoff2,
}

impl ModDest {
    pub const ALL: [ModDest; 14] = [
        ModDest::Cutoff,
        ModDest::Resonance,
        ModDest::Pitch,
        ModDest::Osc1Pitch,
        ModDest::Osc2Pitch,
        ModDest::Osc1Position,
        ModDest::Osc2Position,
        ModDest::Osc1Level,
        ModDest::Osc2Level,
        ModDest::Volume,
        ModDest::Pan,
        ModDest::UnisonDetune,
        ModDest::Drive,
        ModDest::Cutoff2,
    ];
    pub const COUNT: usize = Self::ALL.len();

    pub fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(default))]
pub struct ModSlot {
    pub source: ModSource,
    pub dest: ModDest,
    /// -1..1.
    pub amount: f32,
}

impl ModSlot {
    pub fn new(source: ModSource, dest: ModDest, amount: f32) -> Self {
        Self { source, dest, amount }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_match_the_list() {
        for (i, d) in ModDest::ALL.iter().enumerate() {
            assert_eq!(d.index(), i);
        }
    }
}
