//! Sound engine of the synthesizer.
//!
//! Nothing in here allocates or locks while rendering, so [`Engine::render`] can be
//! called from a real-time audio callback.

pub mod engine;
pub mod envelope;
pub mod filter;
pub mod midi;
pub mod oscillator;
pub mod patch;
pub mod voice;

pub use engine::{Engine, Event, MAX_VOICES};
pub use filter::FilterMode;
pub use oscillator::Waveform;
pub use patch::{FilterParams, OscParams, Patch};
pub use envelope::AdsrParams;

/// Frequency in Hz of a (fractional) MIDI note number; 69 = A4 = 440 Hz.
pub fn note_to_hz(note: f32) -> f32 {
    440.0 * ((note - 69.0) / 12.0).exp2()
}

/// Name of a MIDI note, e.g. 60 → "C4".
pub fn note_name(note: u8) -> String {
    const NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    format!("{}{}", NAMES[note as usize % 12], note as i32 / 12 - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a4_is_440() {
        assert!((note_to_hz(69.0) - 440.0).abs() < 1e-3);
        assert!((note_to_hz(81.0) - 880.0).abs() < 1e-2);
        assert!((note_to_hz(60.0) - 261.626).abs() < 1e-2);
    }

    #[test]
    fn names() {
        assert_eq!(note_name(60), "C4");
        assert_eq!(note_name(61), "C#4");
        assert_eq!(note_name(21), "A0");
    }
}
