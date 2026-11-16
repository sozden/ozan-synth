//! The synthesizer window.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{self, Key, RichText};
use ozan_core::wavetable::WavetableBank;
use ozan_core::{Event, Patch};

use crate::audio::{self, AudioOutput};
use ozan_ui::i18n::{self, fill, s, Lang};
use ozan_ui::panel::Meters;
use crate::midi_in::{self, MidiConnection};
use ozan_ui::preset_bar::PresetBar;
use ozan_ui::settings::Settings;
use crate::shared::{Events, Status};
use ozan_ui::{theme, widgets};

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

pub struct OzanApp {
    events: Events,
    status: Arc<Status>,
    audio: Result<AudioOutput, String>,

    midi_ports: Vec<String>,
    midi: Option<MidiConnection>,
    /// Controller changes from the MIDI input, for MIDI learn.
    controllers: Option<rtrb::Consumer<(u8, u8)>>,
    /// MIDI learn bindings as last written to the settings.
    saved_map: String,
    midi_error: Option<String>,
    /// The user chose "none"; do not reconnect on our own.
    midi_off: bool,
    last_scan: Instant,
    seen_messages: u32,
    midi_glow: f32,

    patch: Patch,
    /// Scope samples, refilled each frame.
    scope: Vec<f32>,
    /// Wavetables shared with the engine, here for drawing.
    bank: Arc<WavetableBank>,
    bar: PresetBar,

    /// Octave of the computer keyboard's A key (4 → C4).
    pc_octave: i32,
    pc_held: HashMap<Key, u8>,
    mouse_note: Option<u8>,
    /// Recent peak (for the clip light) and the VU needle's level.
    meter: f32,
    vu: f32,
    settings: Settings,
}

impl OzanApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let settings = Settings::load();
        ozan_ui::learn::enable(settings.get("midi_map"));
        i18n::select(settings.get("lang").and_then(Lang::from_code).unwrap_or(Lang::En));
        let theme = settings.get("theme").and_then(theme::index_of).unwrap_or(0);
        theme::select(&cc.egui_ctx, theme);
        let status = Arc::new(Status::default());
        let (events, rx) = Events::new(1024);
        let bank = Arc::new(WavetableBank::new());
        let audio = audio::open(rx, status.clone(), bank.clone());
        let mut app = Self {
            events,
            status,
            audio,
            midi_ports: Vec::new(),
            midi: None,
            controllers: None,
            saved_map: ozan_ui::learn::save(),
            midi_error: None,
            midi_off: false,
            last_scan: Instant::now() - RESCAN,
            seen_messages: 0,
            midi_glow: 0.0,
            patch: Patch::default(),
            scope: Vec::with_capacity(ozan_ui::scope::LEN),
            bank,
            pc_octave: 4,
            pc_held: HashMap::new(),
            mouse_note: None,
            meter: 0.0,
            vu: 0.0,
            settings,
            bar: PresetBar::default(),
        };
        let last = app.settings.get("preset").map(str::to_string);
        app.bar = PresetBar::new(last.as_deref());
        let first = app.bar.current().unwrap_or(0);
        app.select_preset(first);
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
        let (tx, rx) = rtrb::RingBuffer::new(256);
        match midi_in::connect(name, self.events.clone(), self.status.clone(), tx) {
            Ok(c) => {
                self.midi = Some(c);
                self.controllers = Some(rx);
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
        if ctx.wants_keyboard_input() {
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

    fn select_preset(&mut self, index: usize) {
        if self.bar.load(index, &mut self.patch) {
            self.preset_loaded(index);
        }
    }

    /// After the bar loaded a preset into the patch.
    fn preset_loaded(&mut self, index: usize) {
        self.events.send(Event::Patch(self.patch));
        if let Some(name) = self.bar.name(index).map(str::to_string) {
            self.settings.set("preset", &name);
        }
    }

    fn header(&mut self, ui: &mut egui::Ui) {
        let mut bar_right = 0.0;
        ui.horizontal(|ui| {
            let th = theme::t();
            ozan_ui::logo::lockup(ui, 22.0, th.text, th.accent, th.muted).on_hover_text("OZAN Synth — open-source synthesizer");
            ui.label(RichText::new(format!("v{}", env!("CARGO_PKG_VERSION"))).color(th.muted));
            ui.add_space(16.0);
            if let Some(i) = self.bar.bar(ui, &mut self.patch) {
                self.preset_loaded(i);
            }
            ozan_ui::dice::button(ui, &mut self.patch, &self.bank);
            bar_right = ui.min_rect().right();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| match &self.audio {
                Ok(a) => {
                    ui.label(RichText::new(format!("{} · {:.1} kHz", a.name, a.sample_rate as f32 / 1000.0)).color(theme::t().muted));
                    widgets::led(ui, theme::t().good);
                }
                Err(e) => {
                    ui.label(RichText::new(fill(s().audio_failed, &[e])).color(theme::t().hot));
                }
            });
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(s().midi_input);
            let current = self.midi.as_ref().map(|m| m.name.clone());
            let shown = current.clone().unwrap_or_else(|| if self.midi_ports.is_empty() { s().no_device.into() } else { s().off.into() });
            let mut choice: Option<Option<String>> = None;
            egui::ComboBox::from_id_salt("midi_port").selected_text(shown).width(260.0).show_ui(ui, |ui| {
                if ui.selectable_label(current.is_none(), s().off).clicked() {
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
            if ui.button("⟳").on_hover_text(s().rescan).clicked() {
                self.scan_midi(false);
            }
            let glow = theme::t().good.gamma_multiply(0.25 + 0.75 * self.midi_glow);
            widgets::led(ui, if self.midi.is_some() { glow } else { theme::t().track }).on_hover_text(s().midi_led);
            if let Some(e) = &self.midi_error {
                ui.label(RichText::new(e).color(theme::t().warm));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(s().panic).on_hover_text(s().panic_hint).clicked() {
                    self.release_pc_keys();
                    self.events.send(Event::AllNotesOff);
                    self.events.send(Event::ClearEffects);
                }
                ui.add_space(12.0);
                let th = theme::t();
                let mut index = theme::current_index();
                egui::ComboBox::from_id_salt("theme").selected_text(theme::display_name(th)).width(130.0).show_ui(ui, |ui| {
                    for (i, option) in theme::THEMES.iter().enumerate() {
                        ui.selectable_value(&mut index, i, theme::display_name(option));
                    }
                });
                ui.label(RichText::new(s().theme).color(th.muted));
                ui.add_space(12.0);
                let mut lang = i18n::current();
                egui::ComboBox::from_id_salt("language").selected_text(lang.native_name()).width(90.0).show_ui(ui, |ui| {
                    for option in Lang::ALL {
                        ui.selectable_value(&mut lang, option, option.native_name());
                    }
                });
                ui.label(RichText::new(s().language).color(th.muted));
                if lang != i18n::current() {
                    i18n::select(lang);
                    self.settings.set("lang", lang.code());
                }
                if index != theme::current_index() {
                    theme::select(ui.ctx(), index);
                    self.settings.set("theme", theme::t().key);
                }
            });
        });
        // The screen spans both header rows, right of the preset selector.
        let r = ui.min_rect();
        let rect = egui::Rect::from_min_size(egui::pos2(bar_right + 28.0, r.top()), egui::vec2(340.0, r.height()));
        let rate = self.audio.as_ref().ok().map(|a| a.sample_rate as f32);
        let idle = ozan_ui::lcd::idle_line(&self.patch, Some(self.status.voices()), rate);
        ozan_ui::lcd::show_at(ui, rect, &ozan_ui::lcd::preset_line(&self.bar, &self.patch), &idle);
    }

    fn keyboard(&mut self, ui: &mut egui::Ui) {
        let held = self.status.held_notes();
        let names: Vec<String> = (0..128u8).filter(|n| held & (1u128 << n) != 0).map(ozan_core::note_name).collect();
        ui.horizontal(|ui| {
            ui.label(RichText::new(fill(s().pc_keys, &[&self.pc_octave])).size(13.0).color(theme::t().muted));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(if names.is_empty() { "—".to_string() } else { names.join(" ") }).monospace().color(theme::t().accent));
            });
        });
        ui.add_space(4.0);
        let height = ui.available_height().clamp(90.0, 170.0);
        let mut pressed = Vec::new();
        ui.horizontal(|ui| {
            // The wheels follow the controller and can be moved with the mouse.
            let size = egui::vec2(34.0, height);
            let mut bend = self.status.bend();
            if widgets::wheel(ui, s().pitch_wheel, &mut bend, true, size) {
                self.status.set_bend(bend);
                self.events.send(Event::PitchBend(bend));
            }
            let mut wheel = self.status.wheel();
            if widgets::wheel(ui, s().mod_wheel, &mut wheel, false, size) {
                self.status.set_wheel(wheel);
                self.events.send(Event::ModWheel(wheel));
            }
            ui.add_space(6.0);
            pressed = widgets::piano(ui, PIANO_LOW, PIANO_HIGH, held, &mut self.mouse_note, height);
        });
        for (note, velocity) in pressed {
            match velocity {
                Some(v) => self.note_on(note, v),
                None => self.note_off(note),
            }
        }
    }
}

impl eframe::App for OzanApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let ctx = ctx.clone();
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
        // VU ballistics: the needle needs about 300 ms to settle (per 16 ms frame).
        self.vu += (peak - self.vu) * 0.12;
        if let Some(program) = self.status.take_program() {
            if !self.bar.is_empty() {
                self.select_preset(program as usize % self.bar.len());
            }
        }
        if let Some(name) = self.bar.dialogs(&ctx, &self.patch) {
            self.settings.set("preset", &name);
        }

        ozan_ui::wood_cheeks(&ctx);
        egui::TopBottomPanel::top("header").frame(ozan_ui::pad()).show(&ctx, |ui| self.header(ui));
        egui::TopBottomPanel::bottom("keys").frame(ozan_ui::pad()).show(&ctx, |ui| self.keyboard(ui));
        egui::CentralPanel::default().frame(ozan_ui::pad()).show(&ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                let before = self.patch;
                self.status.scope.snapshot(&mut self.scope);
                let meters = Meters {
                    scope: &self.scope,
                    sample_rate: self.status.scope.sample_rate(),
                    vu: self.vu,
                    peak: self.meter,
                    voices: self.status.voices(),
                    lfo: self.status.lfo(),
                    lfo2: self.status.lfo2(),
                    wheel: self.status.wheel(),
                    aftertouch: self.status.aftertouch(),
                    seq_step: self.status.seq_step(),
                };
                if let Some(rx) = &mut self.controllers {
                    while let Ok((cc, value)) = rx.pop() {
                        ozan_ui::learn::feed(cc, value);
                    }
                }
                ozan_ui::panel::panel(ui, &mut self.patch, &self.bank, &meters);
                let map = ozan_ui::learn::save();
                if map != self.saved_map {
                    self.settings.set("midi_map", &map);
                    self.saved_map = map;
                }
                if self.patch != before {
                    self.events.send(Event::Patch(self.patch));
                }
            });
        });

        // Meter, LED and lit keys follow the audio thread.
        ctx.request_repaint_after(Duration::from_millis(16));
    }
}
