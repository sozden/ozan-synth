//! USB MIDI keyboards through midir (WinMM on Windows).
//!
//! On Windows a MIDI input can be open in one program at a time: if a DAW already
//! uses the keyboard, connecting here fails.

use std::sync::Arc;

use midir::{Ignore, MidiInput, MidiInputConnection};

use crate::shared::{Events, Status};

const CLIENT: &str = "Synth";

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

pub fn connect(name: &str, events: Events, status: Arc<Status>) -> Result<MidiConnection, String> {
    let mut input = MidiInput::new(CLIENT).map_err(|e| e.to_string())?;
    input.ignore(Ignore::All);
    let ports = input.ports();
    let port = ports
        .iter()
        .find(|p| input.port_name(p).ok().as_deref() == Some(name))
        .ok_or_else(|| format!("“{name}” artık bağlı değil"))?;
    let connection = input
        .connect(
            port,
            "synth-in",
            move |_time, message, _| {
                status.midi_received();
                if let Some(event) = synth_core::midi::parse(message) {
                    events.send(event);
                }
            },
            (),
        )
        .map_err(|e| format!("“{name}” açılamadı ({e}); başka bir program kullanıyor olabilir"))?;
    Ok(MidiConnection { _connection: connection, name: name.to_string() })
}
