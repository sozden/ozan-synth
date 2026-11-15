//! USB MIDI keyboards through midir (WinMM on Windows).
//!
//! On Windows a MIDI input can be open in one program at a time: if a DAW already
//! uses the keyboard, connecting here fails.

use std::sync::Arc;

use midir::{Ignore, MidiInput, MidiInputConnection};
use ozan_core::Event;

use ozan_ui::i18n::{fill, s};
use crate::shared::{Events, Status};

const CLIENT: &str = "OZAN Synth";

/// Names of the MIDI inputs present now.
pub fn ports() -> Vec<String> {
    let Ok(input) = MidiInput::new(CLIENT) else { return Vec::new() };
    input.ports().iter().filter_map(|p| input.port_name(p).ok()).collect()
}

/// An open MIDI input; closed when dropped.
pub struct MidiConnection {
    _connection: MidiInputConnection<()>,
    pub name: String,
}

/// Controller changes (CC number, value) go to `controllers` too, for MIDI learn.
pub fn connect(name: &str, events: Events, status: Arc<Status>, mut controllers: rtrb::Producer<(u8, u8)>) -> Result<MidiConnection, String> {
    let mut input = MidiInput::new(CLIENT).map_err(|e| e.to_string())?;
    input.ignore(Ignore::All);
    let ports = input.ports();
    let port = ports
        .iter()
        .find(|p| input.port_name(p).ok().as_deref() == Some(name))
        .ok_or_else(|| fill(s().midi_gone, &[&name]))?;
    let connection = input
        .connect(
            port,
            "ozan-in",
            move |_time, message, _| {
                status.midi_received();
                if let [st, cc, value, ..] = *message {
                    if st & 0xF0 == 0xB0 {
                        let _ = controllers.push((cc & 0x7F, value & 0x7F));
                    }
                }
                match ozan_core::midi::parse(message) {
                    // Choosing a preset is the interface's job.
                    Some(Event::ProgramChange(p)) => status.request_program(p),
                    Some(event) => {
                        match event {
                            Event::ModWheel(v) => status.set_wheel(v),
                            Event::PitchBend(v) => status.set_bend(v),
                            Event::Aftertouch(v) => status.set_aftertouch(v),
                            _ => {}
                        }
                        events.send(event);
                    }
                    None => {}
                }
            },
            (),
        )
        .map_err(|e| fill(s().midi_busy, &[&name, &e]))?;
    Ok(MidiConnection { _connection: connection, name: name.to_string() })
}
