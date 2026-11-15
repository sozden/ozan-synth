//! Turns raw MIDI messages into engine events. All channels are accepted (omni).

use crate::engine::Event;

/// Parses one complete MIDI message; returns `None` for messages the synth ignores.
pub fn parse(message: &[u8]) -> Option<Event> {
    let (&status, data) = message.split_first()?;
    let d1 = data.first().map(|b| b & 0x7F);
    let d2 = data.get(1).map(|b| b & 0x7F);
    match status & 0xF0 {
        0x80 => Some(Event::NoteOff { note: d1? }),
        0x90 => match d2? {
            0 => Some(Event::NoteOff { note: d1? }),
            velocity => Some(Event::NoteOn { note: d1?, velocity }),
        },
        0xB0 => match d1? {
            1 => Some(Event::ModWheel(d2? as f32 / 127.0)),
            64 => Some(Event::SustainPedal(d2? >= 64)),
            // All sound off / all notes off.
            120 | 123 => Some(Event::AllNotesOff),
            _ => None,
        },
        0xC0 => Some(Event::ProgramChange(d1?)),
        0xD0 => Some(Event::Aftertouch(d1? as f32 / 127.0)),
        0xE0 => {
            let value = ((d2? as i32) << 7 | d1? as i32) - 8192;
            Some(Event::PitchBend(value as f32 / 8192.0))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_on_any_channel() {
        assert_eq!(parse(&[0x90, 60, 100]), Some(Event::NoteOn { note: 60, velocity: 100 }));
        assert_eq!(parse(&[0x9F, 61, 1]), Some(Event::NoteOn { note: 61, velocity: 1 }));
        assert_eq!(parse(&[0x83, 60, 64]), Some(Event::NoteOff { note: 60 }));
        assert_eq!(parse(&[0x90, 60, 0]), Some(Event::NoteOff { note: 60 }));
    }

    #[test]
    fn pedal_and_panic() {
        assert_eq!(parse(&[0xB0, 64, 127]), Some(Event::SustainPedal(true)));
        assert_eq!(parse(&[0xB0, 64, 0]), Some(Event::SustainPedal(false)));
        assert_eq!(parse(&[0xB0, 123, 0]), Some(Event::AllNotesOff));
        assert_eq!(parse(&[0xB0, 7, 50]), None);
        assert_eq!(parse(&[0xB0, 1, 127]), Some(Event::ModWheel(1.0)));
        assert_eq!(parse(&[0xD2, 0]), Some(Event::Aftertouch(0.0)));
        assert_eq!(parse(&[0xC0, 5]), Some(Event::ProgramChange(5)));
    }

    #[test]
    fn pitch_bend_range() {
        assert_eq!(parse(&[0xE0, 0x00, 0x40]), Some(Event::PitchBend(0.0)));
        assert_eq!(parse(&[0xE0, 0x00, 0x00]), Some(Event::PitchBend(-1.0)));
        let Some(Event::PitchBend(up)) = parse(&[0xE0, 0x7F, 0x7F]) else { panic!() };
        assert!((up - 1.0).abs() < 1e-3);
    }

    #[test]
    fn short_or_unknown_messages() {
        assert_eq!(parse(&[]), None);
        assert_eq!(parse(&[0x90, 60]), None);
        assert_eq!(parse(&[0xF8]), None);
    }
}
