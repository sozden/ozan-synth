//! What the audio, MIDI and interface threads share.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use synth_core::Event;

/// Sends events to the audio thread. The queue itself is lock-free on the audio side;
/// the mutex only orders the two senders (MIDI thread and interface).
#[derive(Clone)]
pub struct Events(Arc<Mutex<rtrb::Producer<Event>>>);

impl Events {
    pub fn new(capacity: usize) -> (Self, rtrb::Consumer<Event>) {
        let (tx, rx) = rtrb::RingBuffer::new(capacity);
        (Self(Arc::new(Mutex::new(tx))), rx)
    }

    /// Drops the event if the queue is full (the audio thread has stalled).
    pub fn send(&self, event: Event) {
        if let Ok(mut tx) = self.0.lock() {
            let _ = tx.push(event);
        }
    }
}

/// State the audio and MIDI threads report back to the interface.
#[derive(Default)]
pub struct Status {
    voices: AtomicU32,
    /// Highest sample since the interface last looked (f32 bits; positive floats
    /// order the same as their bits, so `fetch_max` works).
    peak: AtomicU32,
    held: [AtomicU64; 2],
    midi_messages: AtomicU32,
}

impl Status {
    pub fn report_audio(&self, voices: usize, peak: f32, held: u128) {
        self.voices.store(voices as u32, Ordering::Relaxed);
        self.peak.fetch_max(peak.abs().to_bits(), Ordering::Relaxed);
        self.held[0].store(held as u64, Ordering::Relaxed);
        self.held[1].store((held >> 64) as u64, Ordering::Relaxed);
    }

    pub fn voices(&self) -> u32 {
        self.voices.load(Ordering::Relaxed)
    }

    /// Peak since the previous call.
    pub fn take_peak(&self) -> f32 {
        f32::from_bits(self.peak.swap(0, Ordering::Relaxed))
    }

    pub fn held_notes(&self) -> u128 {
        self.held[0].load(Ordering::Relaxed) as u128 | (self.held[1].load(Ordering::Relaxed) as u128) << 64
    }

    pub fn midi_received(&self) {
        self.midi_messages.fetch_add(1, Ordering::Relaxed);
    }

    pub fn midi_messages(&self) -> u32 {
        self.midi_messages.load(Ordering::Relaxed)
    }
}
