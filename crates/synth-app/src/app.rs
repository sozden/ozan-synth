//! The synthesizer window.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{self, Key, RichText, Vec2};
use synth_core::{AdsrParams, Event, OscParams, Patch};

use crate::audio::{self, AudioOutput};
use crate::knob::{self, Knob};
use crate::midi_in::{self, MidiConnection};
use crate::shared::{Events, Status};
use crate::theme;
use crate::widgets;

/// Lowest and highest key of the on-screen keyboard.
const PIANO_LOW: u8 = 36; // C2
const PIANO_HIGH: u8 = 96; // C7

/// Computer keys played like a piano, two rows from C: A W S E D F T G Y H U J K O L.
const PC_KEYS: [(Key, u8); 15] = [
    (Key::A, 0), (Key::W, 1), (Key::S, 2), (Key::E, 3), (Key::D, 4), (Key::F, 5), (Key::T, 6), (Key::G, 7),
    (Key::Y, 8), (Key::H, 9), (Key::U, 10), (Key::J, 11), (Key::K, 12), (Key::O, 13), (Key::L, 14),
];

/// How often MIDI inputs are looked for while none is connected.
const RESCAN: Duration = Duration::from_secs(2);

pub struct SynthApp {
    events: Events,
    status: Arc<Status>,
    audio: Result<AudioOutput, String>,

    midi_ports: Vec<String>,
    midi: Option<MidiConnection>,
    midi_error: Option<String>,
    /// The user chose "none"; do not reconnect on our own.
    midi_off: bool,
    last_scan: Instant,
    seen_messages: u32,
    midi_glow: f32,

    patch: Patch,

    /// Octave of the computer keyboard's A key (4 → C4).
    pc_octave: i32,
    pc_held: HashMap<Key, u8>,
    mouse_note: Option<u8>,
    meter: f32,
}

impl SynthApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::apply(&cc.egui_ctx);
        let status = Arc::new(Status::default());
        let (events, rx) = Events::new(1024);
        let audio = audio::open(rx, status.clone());
        let mut app = Self {
            events,
            status,
            audio,
            midi_ports: Vec::new(),
            midi: None,
            midi_error: None,
            midi_off: false,
            last_scan: Instant::now() - RESCAN,
            seen_messages: 0,
            midi_glow: 0.0,
            patch: Patch::default(),
            pc_octave: 4,
            pc_held: HashMap::new(),
            mouse_note: None,
            meter: 0.0,
        };
        app.events.send(Event::Patch(app.patch));
        app.scan_midi(true);
        app
    }

    /// Refreshes the list of MIDI inputs; with `auto`, connects to the first one when
    /// nothing is connected.
    fn scan_midi(&mut self, auto: bool) {
        self.last_scan = Instant::now();
        self.midi_ports = midi_in::ports();
        if auto && self.midi.is_none() && !self.midi_off {
            if let Some(first) = self.midi_ports.first().cloned() {
                self.connect_midi(&first);
            }
        }
    }

    fn connect_midi(&mut self, name: &str) {
        self.midi = None; // close first: Windows allows one user per input
        match midi_in::connect(name, self.events.clone(), self.status.clone()) {
            Ok(c) => {
                self.midi = Some(c);
                self.midi_error = None;
                self.midi_off = false;
            }
            Err(e) => self.midi_error = Some(e),
        }
    }

    fn note_on(&self, note: u8, velocity: u8) {
        self.events.send(Event::NoteOn { note, velocity });
    }

    fn note_off(&self, note: u8) {
        self.events.send(Event::NoteOff { note });
    }

    fn release_pc_keys(&mut self) {
        for (_, note) in std::mem::take(&mut self.pc_held) {
            self.note_off(note);
        }
    }

    fn computer_keyboard(&mut self, ctx: &egui::Context) {
        if !ctx.input(|i| i.focused) {
            self.release_pc_keys();
            return;
        }
        // Text fields (none yet) would need the keys for themselves.
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let keys: Vec<(Key, bool, bool)> = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Key { key, pressed, repeat, .. } => Some((*key, *pressed, *repeat)),
                    _ => None,
                })
                .collect()
        });
        for (key, pressed, repeat) in keys {
            if repeat {
                continue;
            }
            match key {
                Key::Z if pressed => self.pc_octave = (self.pc_octave - 1).max(0),
                Key::X if pressed => self.pc_octave = (self.pc_octave + 1).min(8),
                _ => {}
            }
            let Some(&(_, offset)) = PC_KEYS.iter().find(|(k, _)| *k == key) else { continue };
            if pressed {
                let note = ((self.pc_octave + 1) * 12 + offset as i32).clamp(0, 127) as u8;
                if let std::collections::hash_map::Entry::Vacant(slot) = self.pc_held.entry(key) {
                    slot.insert(note);
                    self.note_on(note, 100);
                }
            } else if let Some(note) = self.pc_held.remove(&key) {
                self.note_off(note);
            }
        }
    }

    fn header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("SYNTH").size(20.0).strong().color(theme::ACCENT));
            ui.label(RichText::new(format!("v{} · aşama 2", env!("CARGO_PKG_VERSION"))).color(theme::MUTED));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| match &self.audio {
                Ok(a) => {
                    ui.label(RichText::new(format!("{} · {:.1} kHz", a.name, a.sample_rate as f32 / 1000.0)).color(theme::MUTED));
                    widgets::led(ui, theme::GOOD);
                }
                Err(e) => {
                    ui.label(RichText::new(format!("Ses çıkışı açılamadı: {e}")).color(theme::HOT));
                }
            });
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("MIDI giriş");
            let current = self.midi.as_ref().map(|m| m.name.clone());
            let shown = current.clone().unwrap_or_else(|| if self.midi_ports.is_empty() { "aygıt yok".into() } else { "kapalı".into() });
            let mut choice: Option<Option<String>> = None;
            egui::ComboBox::from_id_salt("midi_port").selected_text(shown).width(260.0).show_ui(ui, |ui| {
                if ui.selectable_label(current.is_none(), "kapalı").clicked() {
                    choice = Some(None);
                }
                for p in &self.midi_ports {
                    if ui.selectable_label(current.as_deref() == Some(p), p).clicked() {
                        choice = Some(Some(p.clone()));
                    }
                }
            });
            match choice {
                Some(Some(name)) => self.connect_midi(&name),
                Some(None) => {
                    self.midi = None;
                    self.midi_off = true;
                }
                None => {}
            }
            if ui.button("⟳").on_hover_text("MIDI aygıtlarını yeniden ara").clicked() {
                self.scan_midi(false);
            }
            let glow = theme::GOOD.gamma_multiply(0.25 + 0.75 * self.midi_glow);
            widgets::led(ui, if self.midi.is_some() { glow } else { theme::TRACK }).on_hover_text("MIDI mesajı geldikçe yanar");
            if let Some(e) = &self.midi_error {
                ui.label(RichText::new(e).color(theme::WARM));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Tüm notaları sustur").clicked() {
                    self.release_pc_keys();
                    self.events.send(Event::AllNotesOff);
                }
            });
        });
    }

    fn panel(&mut self, ui: &mut egui::Ui) {
        let before = self.patch;
        let p = &mut self.patch;
        ui.spacing_mut().item_spacing = Vec2::new(12.0, 12.0);
        ui.horizontal_top(|ui| {
            for (i, osc) in p.osc.iter_mut().enumerate() {
                let title = if i == 0 { "OSİLATÖR 1" } else { "OSİLATÖR 2" };
                theme::section(ui, title, |ui| oscillator(ui, osc, i == 1));
            }
            theme::section(ui, "FİLTRE", |ui| {
                knob::filter_mode_selector(ui, &mut p.filter.mode);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    let f = &mut p.filter;
                    Knob::new("Cutoff", &mut f.cutoff, 20.0, 20_000.0).log().default_value(800.0).format(hertz).show(ui);
                    Knob::new("Rezonans", &mut f.resonance, 0.0, 1.0).default_value(0.3).format(percent).show(ui);
                    Knob::new("Zarf", &mut f.env_amount, -1.0, 1.0).bipolar().default_value(0.4).format(signed_percent).show(ui);
                    Knob::new("Nota izi", &mut f.key_track, 0.0, 1.0).default_value(0.5).format(percent).show(ui);
                });
            });
        });
        ui.horizontal_top(|ui| {
            theme::section(ui, "FİLTRE ZARFI", |ui| envelope(ui, &mut p.filter_env));
            theme::section(ui, "AMFİ ZARFI", |ui| envelope(ui, &mut p.amp_env));
            theme::section(ui, "ÇIKIŞ", |ui| {
                ui.horizontal(|ui| {
                    Knob::new("Ses", &mut p.volume, 0.0, 1.0).default_value(0.7).format(percent).show(ui);
                    widgets::level_meter(ui, self.meter, Vec2::new(12.0, 92.0));
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Ses sayısı").size(11.0).color(theme::MUTED));
                        ui.label(RichText::new(format!("{} / {}", self.status.voices(), synth_core::MAX_VOICES)).monospace().size(16.0));
                        ui.add_space(8.0);
                        if ui.button("Varsayılan ses").on_hover_text("Tüm ayarları başlangıç değerine döndürür").clicked() {
                            *p = Patch::default();
                        }
                    });
                });
            });
        });
        if self.patch != before {
            self.events.send(Event::Patch(self.patch));
        }
    }

    fn keyboard(&mut self, ui: &mut egui::Ui) {
        let held = self.status.held_notes();
        let names: Vec<String> = (0..128u8).filter(|n| held & (1u128 << n) != 0).map(synth_core::note_name).collect();
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("Bilgisayar klavyesi: A W S E D F T G Y H U J K · Z/X oktav (A = C{})", self.pc_octave)).size(12.0).color(theme::MUTED));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(if names.is_empty() { "—".to_string() } else { names.join(" ") }).monospace().color(theme::ACCENT));
            });
        });
        ui.add_space(4.0);
        let height = ui.available_height().clamp(90.0, 170.0);
        for (note, velocity) in widgets::piano(ui, PIANO_LOW, PIANO_HIGH, held, &mut self.mouse_note, height) {
            match velocity {
                Some(v) => self.note_on(note, v),
                None => self.note_off(note),
            }
        }
    }
}

fn percent(v: f32) -> String {
    format!("{:.0}%", v * 100.0)
}

fn signed_percent(v: f32) -> String {
    format!("{:+.0}%", v * 100.0)
}

fn seconds(v: f32) -> String {
    if v < 1.0 { format!("{:.0} ms", v * 1000.0) } else { format!("{v:.2} s") }
}

fn hertz(v: f32) -> String {
    if v < 1000.0 { format!("{v:.0} Hz") } else { format!("{:.2} kHz", v / 1000.0) }
}

fn signed_int(v: f32) -> String {
    format!("{:+}", v as i32)
}

fn cents(v: f32) -> String {
    format!("{v:+.0} ct")
}

/// Waveform buttons and pitch/level knobs of one oscillator. Only the second one
/// gets a semitone knob (for intervals like a fifth).
fn oscillator(ui: &mut egui::Ui, osc: &mut OscParams, semitones: bool) {
    knob::wave_selector(ui, &mut osc.wave);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let mut octave = osc.octave as f32;
        if Knob::new("Oktav", &mut octave, -3.0, 3.0).integer().default_value(0.0).bipolar().format(signed_int).show(ui) {
            osc.octave = octave as i8;
        }
        if semitones {
            let mut semi = osc.semitones as f32;
            if Knob::new("Yarım ton", &mut semi, -12.0, 12.0).integer().default_value(0.0).bipolar().format(signed_int).show(ui) {
                osc.semitones = semi as i8;
            }
        }
        Knob::new("İnce", &mut osc.fine, -50.0, 50.0).default_value(0.0).bipolar().format(cents).show(ui);
        Knob::new("Seviye", &mut osc.level, 0.0, 1.0).format(percent).show(ui);
    });
}

/// Four ADSR knobs and the shape they make.
fn envelope(ui: &mut egui::Ui, env: &mut AdsrParams) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        Knob::new("Attack", &mut env.attack, 0.001, 5.0).log().format(seconds).show(ui);
        Knob::new("Decay", &mut env.decay, 0.005, 5.0).log().format(seconds).show(ui);
        Knob::new("Sustain", &mut env.sustain, 0.0, 1.0).format(percent).show(ui);
        Knob::new("Release", &mut env.release, 0.005, 8.0).log().format(seconds).show(ui);
        ui.add_space(8.0);
        widgets::envelope_graph(ui, env.attack, env.decay, env.sustain, env.release, Vec2::new(150.0, 92.0));
    });
}

impl eframe::App for SynthApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.computer_keyboard(&ctx);

        if self.midi.is_none() && !self.midi_off && self.last_scan.elapsed() >= RESCAN {
            self.scan_midi(true);
        }
        let messages = self.status.midi_messages();
        if messages != self.seen_messages {
            self.seen_messages = messages;
            self.midi_glow = 1.0;
        }
        self.midi_glow = (self.midi_glow - 0.08).max(0.0);
        let peak = self.status.take_peak();
        self.meter = peak.max(self.meter * 0.85);

        let pad = egui::Frame::new().fill(theme::BG).inner_margin(egui::Margin::same(14));
        egui::Panel::top("header").frame(pad).show(ui, |ui| self.header(ui));
        egui::Panel::bottom("keys").frame(pad).show(ui, |ui| self.keyboard(ui));
        egui::CentralPanel::default().frame(pad).show(ui, |ui| self.panel(ui));

        // Meter, LED and lit keys follow the audio thread.
        ctx.request_repaint_after(Duration::from_millis(16));
    }
}
