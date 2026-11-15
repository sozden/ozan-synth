//! OZAN Synth as a CLAP and VST3 instrument (through nih-plug).
//!
//! The sound is a full [`Patch`], edited in the plugin's window (the same panel as
//! the standalone program) and saved with the DAW project. The DAW also sees a
//! preset selector and relative "macro" parameters for automation (see [`macros`]).

mod editor;
pub mod macros;

use std::num::NonZeroU32;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, RwLock};

use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use ozan_core::wavetable::WavetableBank;
use ozan_core::{Engine, Event, Patch};

use macros::Macros;

/// Values the audio thread shows in the window (f32 bits).
#[derive(Default)]
pub struct Meters {
    peak: AtomicU32,
    voices: AtomicU32,
    lfo: AtomicU32,
    wheel: AtomicU32,
    aftertouch: AtomicU32,
}

impl Meters {
    fn store(cell: &AtomicU32, v: f32) {
        cell.store(v.to_bits(), Ordering::Relaxed);
    }

    fn load(cell: &AtomicU32) -> f32 {
        f32::from_bits(cell.load(Ordering::Relaxed))
    }

    /// Highest peak since the window last looked.
    pub fn take_peak(&self) -> f32 {
        f32::from_bits(self.peak.swap(0, Ordering::Relaxed))
    }
}

pub struct OzanPlugin {
    params: Arc<OzanParams>,
    presets: Vec<(&'static str, Patch)>,
    /// Shared by the engine and the window (which draws the tables).
    bank: Arc<WavetableBank>,
    meters: Arc<Meters>,
    /// Made in `initialize`, where allocating is allowed.
    engine: Option<Engine>,
    /// Patch last sent to the engine.
    sent: Option<Patch>,
    /// The sound when the window last held the lock; used while it holds it.
    base: Patch,
    /// Preset parameter value last acted on; a change loads that preset.
    preset_seen: Option<i32>,
    wheel: f32,
    aftertouch: f32,
}

#[derive(Params)]
pub struct OzanParams {
    /// Window size, saved with the project.
    #[persist = "editor-state"]
    pub editor_state: Arc<EguiState>,
    /// The full sound, saved with the project.
    #[persist = "patch"]
    pub patch: Arc<RwLock<Patch>>,

    #[id = "preset"]
    pub preset: IntParam,
    #[id = "volume"]
    pub volume: FloatParam,
    #[id = "cutoff"]
    pub cutoff: FloatParam,
    #[id = "reso"]
    pub resonance: FloatParam,
    #[id = "envamt"]
    pub env_amount: FloatParam,
    #[id = "attack"]
    pub attack: FloatParam,
    #[id = "release"]
    pub release: FloatParam,
    #[id = "lforate"]
    pub lfo_rate: FloatParam,
    #[id = "wtpos"]
    pub position: FloatParam,
    #[id = "detune"]
    pub detune: FloatParam,
    #[id = "glide"]
    pub glide: FloatParam,
    #[id = "chorus"]
    pub chorus: FloatParam,
    #[id = "delay"]
    pub delay: FloatParam,
    #[id = "reverb"]
    pub reverb: FloatParam,
}

/// A bipolar parameter centred on 0.
fn offset(name: &str, range: f32, unit: &'static str) -> FloatParam {
    FloatParam::new(name, 0.0, FloatRange::Linear { min: -range, max: range }).with_unit(unit).with_step_size(0.01)
}

/// A factor, 1 in the middle of its travel (log scale between 1/`range` and `range`).
fn factor(name: &str, range: f32) -> FloatParam {
    FloatParam::new(name, 1.0, FloatRange::Skewed { min: 1.0 / range, max: range, factor: FloatRange::skew_factor(-1.0) })
        .with_unit("×")
        .with_value_to_string(Arc::new(|v| format!("{v:.2}")))
}

/// A mix factor from 0 (effect off) to 2 (twice the preset's level).
fn mix(name: &str) -> FloatParam {
    FloatParam::new(name, 1.0, FloatRange::Linear { min: 0.0, max: 2.0 }).with_unit("×").with_step_size(0.01)
}

impl OzanParams {
    fn new(names: Vec<&'static str>, first: Patch) -> Self {
        let count = names.len() as i32;
        Self {
            editor_state: EguiState::from_size(editor::WIDTH, editor::HEIGHT),
            patch: Arc::new(RwLock::new(first)),
            preset: IntParam::new("Preset", 0, IntRange::Linear { min: 0, max: count - 1 }).with_value_to_string(Arc::new(
                move |i| names.get(i as usize).map_or_else(|| i.to_string(), |n| format!("{:02} {n}", i + 1)),
            )),
            volume: FloatParam::new("Volume", 0.0, FloatRange::Linear { min: -24.0, max: 6.0 }).with_unit(" dB").with_step_size(0.1),
            cutoff: offset("Cutoff", 4.0, " oct"),
            resonance: offset("Resonance", 1.0, ""),
            env_amount: offset("Filter env amount", 1.0, ""),
            attack: factor("Attack", 10.0),
            release: factor("Release", 10.0),
            lfo_rate: factor("LFO rate", 10.0),
            position: offset("Wavetable position", 1.0, ""),
            detune: offset("Unison detune", 50.0, " ct"),
            glide: FloatParam::new("Glide", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 }).with_unit(" s").with_step_size(0.01),
            chorus: mix("Chorus mix"),
            delay: mix("Delay mix"),
            reverb: mix("Reverb mix"),
        }
    }

    fn macros(&self) -> Macros {
        Macros {
            volume_db: self.volume.value(),
            cutoff_oct: self.cutoff.value(),
            resonance: self.resonance.value(),
            env_amount: self.env_amount.value(),
            attack: self.attack.value(),
            release: self.release.value(),
            lfo_rate: self.lfo_rate.value(),
            position: self.position.value(),
            detune: self.detune.value(),
            glide: self.glide.value(),
            chorus: self.chorus.value(),
            delay: self.delay.value(),
            reverb: self.reverb.value(),
        }
    }
}

impl Default for OzanPlugin {
    fn default() -> Self {
        let presets = ozan_core::presets::factory();
        let names = presets.iter().map(|(n, _)| *n).collect();
        let first = presets[0].1;
        Self {
            params: Arc::new(OzanParams::new(names, first)),
            presets,
            bank: Arc::new(WavetableBank::new()),
            meters: Arc::new(Meters::default()),
            engine: None,
            sent: None,
            base: first,
            preset_seen: None,
            wheel: 0.0,
            aftertouch: 0.0,
        }
    }
}

/// The engine event for a host note event, if the synth uses it.
fn translate(event: &NoteEvent<()>) -> Option<Event> {
    Some(match *event {
        NoteEvent::NoteOn { note, velocity, .. } => Event::NoteOn { note, velocity: (velocity * 127.0).round().clamp(1.0, 127.0) as u8 },
        NoteEvent::NoteOff { note, .. } | NoteEvent::Choke { note, .. } => Event::NoteOff { note },
        NoteEvent::MidiPitchBend { value, .. } => Event::PitchBend(value * 2.0 - 1.0),
        NoteEvent::MidiChannelPressure { pressure, .. } | NoteEvent::PolyPressure { pressure, .. } => Event::Aftertouch(pressure),
        NoteEvent::MidiCC { cc: 1, value, .. } => Event::ModWheel(value),
        NoteEvent::MidiCC { cc: 64, value, .. } => Event::SustainPedal(value >= 0.5),
        NoteEvent::MidiCC { cc: 120 | 123, .. } => Event::AllNotesOff,
        _ => return None,
    })
}

impl Plugin for OzanPlugin {
    const NAME: &'static str = "OZAN Synth";
    const VENDOR: &'static str = "OZAN";
    const URL: &'static str = "";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: None,
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::MidiCCs;
    const SAMPLE_ACCURATE_AUTOMATION: bool = false;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let names = self.presets.iter().map(|(n, _)| *n).collect();
        editor::create(self.params.clone(), self.meters.clone(), self.bank.clone(), names)
    }

    fn initialize(&mut self, _layout: &AudioIOLayout, buffer_config: &BufferConfig, _context: &mut impl InitContext<Self>) -> bool {
        self.engine = Some(Engine::with_bank(buffer_config.sample_rate, self.bank.clone()));
        self.sent = None;
        // The saved sound stays; only a later change of the preset parameter loads one.
        self.preset_seen = Some(self.params.preset.value());
        true
    }

    fn reset(&mut self) {
        if let Some(engine) = &mut self.engine {
            engine.handle(Event::AllNotesOff);
            engine.handle(Event::ClearEffects);
        }
    }

    fn process(&mut self, buffer: &mut Buffer, _aux: &mut AuxiliaryBuffers, context: &mut impl ProcessContext<Self>) -> ProcessStatus {
        let Some(engine) = &mut self.engine else { return ProcessStatus::Normal };

        let preset = self.params.preset.value();
        if self.preset_seen != Some(preset) {
            // A new preset from the host or the window replaces the sound.
            if let Ok(mut p) = self.params.patch.try_write() {
                *p = self.presets[(preset.max(0) as usize).min(self.presets.len() - 1)].1;
                self.preset_seen = Some(preset);
            }
        }
        // The window may be editing; never wait for it here.
        if let Ok(p) = self.params.patch.try_read() {
            self.base = *p;
        }
        let patch = macros::apply(&self.base, &self.params.macros());
        if self.sent != Some(patch) {
            engine.handle(Event::Patch(patch));
            self.sent = Some(patch);
        }

        let frames = buffer.samples();
        let channels = buffer.as_slice();
        let [left, right] = channels else { return ProcessStatus::Error("OZAN Synth needs a stereo output") };
        let mut pos = 0;
        // Render up to each event, then apply it: notes start on the right sample.
        while let Some(event) = context.next_event() {
            let at = (event.timing() as usize).min(frames);
            if at > pos {
                engine.render_stereo(&mut left[pos..at], &mut right[pos..at]);
                pos = at;
            }
            if let Some(e) = translate(&event) {
                match e {
                    Event::ModWheel(v) => self.wheel = v,
                    Event::Aftertouch(v) => self.aftertouch = v,
                    _ => {}
                }
                engine.handle(e);
            }
        }
        if pos < frames {
            engine.render_stereo(&mut left[pos..frames], &mut right[pos..frames]);
        }
        let peak = left.iter().chain(right.iter()).fold(0.0f32, |m, v| m.max(v.abs()));
        let m = &self.meters;
        m.peak.fetch_max(peak.to_bits(), Ordering::Relaxed);
        Meters::store(&m.voices, engine.active_voices() as f32);
        Meters::store(&m.lfo, engine.lfo_value());
        Meters::store(&m.wheel, self.wheel);
        Meters::store(&m.aftertouch, self.aftertouch);
        // Reverb and echo tails ring on after the last note.
        ProcessStatus::KeepAlive
    }
}

impl ClapPlugin for OzanPlugin {
    const CLAP_ID: &'static str = "com.ozan-synth.ozan";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Open-source subtractive and wavetable synthesizer");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::Instrument, ClapFeature::Synthesizer, ClapFeature::Stereo];
}

impl Vst3Plugin for OzanPlugin {
    const VST3_CLASS_ID: [u8; 16] = *b"OZANSynthPlugin1";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[Vst3SubCategory::Instrument, Vst3SubCategory::Synth];
}

nih_export_clap!(OzanPlugin);
nih_export_vst3!(OzanPlugin);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_and_controllers_translate() {
        let on = NoteEvent::NoteOn { timing: 0, voice_id: None, channel: 0, note: 60, velocity: 0.5 };
        assert_eq!(translate(&on), Some(Event::NoteOn { note: 60, velocity: 64 }));
        let bend = NoteEvent::MidiPitchBend { timing: 0, channel: 0, value: 1.0 };
        assert_eq!(translate(&bend), Some(Event::PitchBend(1.0)));
        let pedal = NoteEvent::MidiCC { timing: 0, channel: 0, cc: 64, value: 1.0 };
        assert_eq!(translate(&pedal), Some(Event::SustainPedal(true)));
        let other = NoteEvent::MidiCC { timing: 0, channel: 0, cc: 7, value: 1.0 };
        assert_eq!(translate(&other), None);
    }

    #[test]
    fn preset_names_show_in_the_host() {
        let plugin = OzanPlugin::default();
        let p = &plugin.params.preset;
        assert_eq!(p.normalized_value_to_string(0.0, false), "01 Init");
        assert_eq!(plugin.params.macros(), Macros::default());
    }
}
