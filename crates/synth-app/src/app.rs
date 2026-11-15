//! The synthesizer window.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{self, Key, RichText, Vec2};
use synth_core::{AdsrParams, Event, OscParams, Patch};

use crate::audio::{self, AudioOutput};
use crate::knob::{self, Knob};
use crate::midi_in::{self, MidiConnection};
use crate::presets::{self, Preset};
use crate::settings::Settings;
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
    presets: Vec<Preset>,
    /// Preset the patch came from (it may have been changed since).
    current: Option<usize>,
    /// Name being typed while the save window is open.
    save_name: Option<String>,
    confirm_delete: bool,
    preset_note: Option<String>,

    /// Octave of the computer keyboard's A key (4 → C4).
    pc_octave: i32,
    pc_held: HashMap<Key, u8>,
    mouse_note: Option<u8>,
    /// Recent peak (for the clip light) and the VU needle's level.
    meter: f32,
    vu: f32,
    settings: Settings,
}

impl SynthApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let settings = Settings::load();
        let theme = settings.get("theme").and_then(theme::index_of).unwrap_or(0);
        theme::select(&cc.egui_ctx, theme);
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
            vu: 0.0,
            settings,
            presets: Vec::new(),
            current: None,
            save_name: None,
            confirm_delete: false,
            preset_note: None,
        };
        let last = app.settings.get("preset").map(str::to_string);
        app.reload_presets(last.as_deref());
        app.select_preset(app.current.unwrap_or(0));
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

    fn reload_presets(&mut self, select: Option<&str>) {
        let (list, problems) = presets::load_all();
        self.presets = list;
        self.preset_note = problems.first().map(|p| format!("Okunamayan preset: {p}"));
        self.current = select.and_then(|name| self.presets.iter().position(|p| p.name == name));
    }

    fn select_preset(&mut self, index: usize) {
        let Some(preset) = self.presets.get(index) else { return };
        self.patch = preset.patch;
        self.current = Some(index);
        let name = preset.name.clone();
        self.events.send(Event::Patch(self.patch));
        self.settings.set("preset", &name);
    }

    fn step_preset(&mut self, step: isize) {
        let n = self.presets.len() as isize;
        if n > 0 {
            let i = self.current.map_or(0, |c| (c as isize + step).rem_euclid(n));
            self.select_preset(i as usize);
        }
    }

    /// The patch differs from the preset it was loaded from.
    fn modified(&self) -> bool {
        self.current.is_none_or(|i| self.presets[i].patch != self.patch)
    }

    fn preset_bar(&mut self, ui: &mut egui::Ui) {
        let th = theme::t();
        if ui.button("◀").on_hover_text("Önceki preset").clicked() {
            self.step_preset(-1);
        }
        let shown = match self.current {
            Some(i) => format!("{:02}  {}{}", i + 1, self.presets[i].name, if self.modified() { " *" } else { "" }),
            None => "—  kaydedilmemiş ses".to_string(),
        };
        let mut chosen = None;
        egui::ComboBox::from_id_salt("preset")
            .selected_text(RichText::new(shown).monospace().color(th.accent))
            .width(230.0)
            .height(480.0)
            .show_ui(ui, |ui| {
                let factory = self.presets.iter().filter(|p| p.file.is_none()).count();
                for (i, preset) in self.presets.iter().enumerate() {
                    if i == factory {
                        ui.separator();
                        ui.label(RichText::new("Senin preset'lerin").size(11.0).color(th.muted));
                    }
                    if ui.selectable_label(self.current == Some(i), format!("{:02}  {}", i + 1, preset.name)).clicked() {
                        chosen = Some(i);
                    }
                }
            });
        if let Some(i) = chosen {
            self.select_preset(i);
        }
        if ui.button("▶").on_hover_text("Sonraki preset").clicked() {
            self.step_preset(1);
        }
        if ui.button("Kaydet").on_hover_text("Şu anki sesi preset olarak kaydeder").clicked() {
            let name = self.current.map(|i| self.presets[i].name.clone()).filter(|_| self.current.is_some_and(|i| self.presets[i].file.is_some()));
            self.save_name = Some(name.unwrap_or_default());
        }
        let deletable = self.current.is_some_and(|i| self.presets[i].file.is_some());
        if ui.add_enabled(deletable, egui::Button::new("Sil")).on_disabled_hover_text("Fabrika preset'leri silinemez").clicked() {
            self.confirm_delete = true;
        }
        if let Some(note) = &self.preset_note {
            ui.label(RichText::new(note).size(11.0).color(th.warm));
        }
    }

    /// Save and delete windows.
    fn preset_dialogs(&mut self, ctx: &egui::Context) {
        if let Some(mut name) = self.save_name.take() {
            let mut close = false;
            let mut save = false;
            let modal = egui::Modal::new(egui::Id::new("save_preset")).show(ctx, |ui| {
                ui.set_width(320.0);
                ui.heading("Preset kaydet");
                ui.add_space(6.0);
                let field = ui.add(egui::TextEdit::singleline(&mut name).hint_text("Preset adı").desired_width(f32::INFINITY));
                field.request_focus();
                let trimmed = name.trim();
                if presets::path_for(trimmed).is_some_and(|p| p.exists()) && !trimmed.is_empty() {
                    ui.label(RichText::new("Bu adda bir preset var; üzerine yazılacak.").size(11.0).color(theme::t().warm));
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let ok = !trimmed.is_empty();
                    if ui.add_enabled(ok, egui::Button::new("Kaydet")).clicked() || (ok && ui.input(|i| i.key_pressed(Key::Enter))) {
                        save = true;
                    }
                    if ui.button("Vazgeç").clicked() {
                        close = true;
                    }
                });
            });
            if save {
                let name = name.trim().to_string();
                match presets::save(&name, &self.patch) {
                    Ok(_) => {
                        self.reload_presets(Some(&name));
                        self.settings.set("preset", &name);
                    }
                    Err(e) => self.preset_note = Some(format!("Kaydedilemedi: {e}")),
                }
            } else if !close && !modal.should_close() {
                self.save_name = Some(name);
            }
        }
        if self.confirm_delete {
            let Some(i) = self.current.filter(|&i| self.presets[i].file.is_some()) else {
                self.confirm_delete = false;
                return;
            };
            let name = self.presets[i].name.clone();
            let mut answer = None;
            let modal = egui::Modal::new(egui::Id::new("delete_preset")).show(ctx, |ui| {
                ui.set_width(320.0);
                ui.heading("Preset silinsin mi?");
                ui.add_space(6.0);
                ui.label(format!("“{name}” dosyası kalıcı olarak silinir; geri alınamaz."));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("Sil").clicked() {
                        answer = Some(true);
                    }
                    if ui.button("Vazgeç").clicked() {
                        answer = Some(false);
                    }
                });
            });
            if answer == Some(true) {
                if let Some(file) = self.presets[i].file.clone() {
                    match presets::delete(&file) {
                        Ok(()) => {
                            // The sound stays as it is; it just has no preset now.
                            self.reload_presets(None);
                        }
                        Err(e) => self.preset_note = Some(format!("Silinemedi: {e}")),
                    }
                }
            }
            if answer.is_some() || modal.should_close() {
                self.confirm_delete = false;
            }
        }
    }

    fn header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let th = theme::t();
            if th.wood {
                ui.label(RichText::new("Synth").size(26.0).italics().color(th.text));
            } else {
                ui.label(RichText::new("SYNTH").size(20.0).strong().color(th.accent));
            }
            ui.label(RichText::new(format!("v{}", env!("CARGO_PKG_VERSION"))).color(th.muted));
            ui.add_space(16.0);
            self.preset_bar(ui);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| match &self.audio {
                Ok(a) => {
                    ui.label(RichText::new(format!("{} · {:.1} kHz", a.name, a.sample_rate as f32 / 1000.0)).color(theme::t().muted));
                    widgets::led(ui, theme::t().good);
                }
                Err(e) => {
                    ui.label(RichText::new(format!("Ses çıkışı açılamadı: {e}")).color(theme::t().hot));
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
            let glow = theme::t().good.gamma_multiply(0.25 + 0.75 * self.midi_glow);
            widgets::led(ui, if self.midi.is_some() { glow } else { theme::t().track }).on_hover_text("MIDI mesajı geldikçe yanar");
            if let Some(e) = &self.midi_error {
                ui.label(RichText::new(e).color(theme::t().warm));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Tüm notaları sustur").on_hover_text("Çalan notaları, yankı ve reverb kuyruklarını keser").clicked() {
                    self.release_pc_keys();
                    self.events.send(Event::AllNotesOff);
                    self.events.send(Event::ClearEffects);
                }
                ui.add_space(12.0);
                let th = theme::t();
                let mut index = theme::current_index();
                egui::ComboBox::from_id_salt("theme").selected_text(th.name).width(130.0).show_ui(ui, |ui| {
                    for (i, option) in theme::THEMES.iter().enumerate() {
                        ui.selectable_value(&mut index, i, option.name);
                    }
                });
                ui.label(RichText::new("Tema").color(th.muted));
                if index != theme::current_index() {
                    theme::select(ui.ctx(), index);
                    self.settings.set("theme", theme::t().key);
                }
            });
        });
    }

    fn panel(&mut self, ui: &mut egui::Ui) {
        let before = self.patch;
        let lfo_now = self.status.lfo();
        let (wheel, aftertouch) = (self.status.wheel(), self.status.aftertouch());
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
                    widgets::vu_meter(ui, self.vu, self.meter, Vec2::new(136.0, 92.0));
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Ses sayısı").size(11.0).color(theme::t().muted));
                        ui.label(RichText::new(format!("{} / {}", self.status.voices(), synth_core::MAX_VOICES)).monospace().size(16.0));
                        ui.add_space(8.0);
                        if ui.button("Varsayılan").on_hover_text("Tüm ayarları başlangıç değerine döndürür").clicked() {
                            *p = Patch::default();
                        }
                    });
                });
            });
        });
        ui.horizontal_top(|ui| {
            theme::section(ui, "LFO", |ui| {
                ui.horizontal(|ui| {
                    knob::lfo_wave_selector(ui, &mut p.lfo.wave);
                    ui.add_space(6.0);
                    let th = theme::t();
                    widgets::led(ui, th.accent.gamma_multiply(0.12 + 0.88 * (0.5 + 0.5 * lfo_now))).on_hover_text("LFO'nun o anki değeri");
                });
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    let l = &mut p.lfo;
                    Knob::new("Hız", &mut l.rate, 0.05, 20.0).log().default_value(5.0).format(rate).show(ui);
                    Knob::new("Vibrato", &mut l.pitch, 0.0, 12.0).default_value(0.0).format(semitones).show(ui);
                    Knob::new("Filtre", &mut l.cutoff, -1.0, 1.0).bipolar().default_value(0.0).format(signed_percent).show(ui);
                    Knob::new("Tremolo", &mut l.amp, 0.0, 1.0).default_value(0.0).format(percent).show(ui);
                });
            });
            theme::section(ui, "TEKERLEK · AFTERTOUCH · BEND", |ui| {
                ui.horizontal(|ui| {
                    widgets::mini_bar(ui, "Teker", wheel);
                    ui.add_space(8.0);
                    widgets::mini_bar(ui, "Aftertouch", aftertouch);
                });
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    Knob::new("Vibrato", &mut p.wheel.vibrato, 0.0, 2.0).default_value(0.5).format(semitones).show(ui);
                    Knob::new("Filtre", &mut p.wheel.cutoff, -1.0, 1.0).bipolar().default_value(0.0).format(signed_percent).show(ui);
                    Knob::new("Bend", &mut p.bend_range, 0.0, 12.0).integer().default_value(2.0).format(bend).show(ui);
                });
            });
        });
        ui.horizontal_top(|ui| {
            let fx = &mut p.fx;
            theme::section(ui, "CHORUS", |ui| {
                widgets::toggle(ui, &mut fx.chorus.on, "Açık");
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    let c = &mut fx.chorus;
                    Knob::new("Hız", &mut c.rate, 0.1, 5.0).log().default_value(0.6).format(rate).show(ui);
                    Knob::new("Derinlik", &mut c.depth, 0.0, 1.0).default_value(0.5).format(percent).show(ui);
                    Knob::new("Karışım", &mut c.mix, 0.0, 1.0).default_value(0.7).format(percent).show(ui);
                });
            });
            theme::section(ui, "DELAY", |ui| {
                ui.horizontal(|ui| {
                    widgets::toggle(ui, &mut fx.delay.on, "Açık");
                    ui.add_space(12.0);
                    widgets::toggle(ui, &mut fx.delay.ping_pong, "Ping-pong");
                });
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    let d = &mut fx.delay;
                    Knob::new("Süre", &mut d.time_ms, 20.0, 2000.0).log().default_value(375.0).format(millis).show(ui);
                    Knob::new("Tekrar", &mut d.feedback, 0.0, 0.95).default_value(0.35).format(percent).show(ui);
                    Knob::new("Ton", &mut d.tone, 500.0, 16_000.0).log().default_value(5000.0).format(hertz).show(ui);
                    Knob::new("Karışım", &mut d.mix, 0.0, 1.0).default_value(0.3).format(percent).show(ui);
                });
            });
            theme::section(ui, "REVERB", |ui| {
                widgets::toggle(ui, &mut fx.reverb.on, "Açık");
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    let r = &mut fx.reverb;
                    Knob::new("Boyut", &mut r.size, 0.0, 1.0).default_value(0.5).format(percent).show(ui);
                    Knob::new("Süre", &mut r.decay, 0.3, 10.0).log().default_value(1.8).format(seconds).show(ui);
                    Knob::new("Sönüm", &mut r.damping, 0.0, 1.0).default_value(0.4).format(percent).show(ui);
                    Knob::new("Gecikme", &mut r.predelay_ms, 0.0, 100.0).default_value(10.0).format(millis).show(ui);
                    Knob::new("Karışım", &mut r.mix, 0.0, 1.0).default_value(0.3).format(percent).show(ui);
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
            ui.label(RichText::new(format!("Bilgisayar klavyesi: A W S E D F T G Y H U J K · Z/X oktav (A = C{})", self.pc_octave)).size(12.0).color(theme::t().muted));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(if names.is_empty() { "—".to_string() } else { names.join(" ") }).monospace().color(theme::t().accent));
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

fn millis(v: f32) -> String {
    format!("{v:.0} ms")
}

fn rate(v: f32) -> String {
    if v < 10.0 { format!("{v:.2} Hz") } else { format!("{v:.1} Hz") }
}

fn semitones(v: f32) -> String {
    format!("{v:.2} yt")
}

fn bend(v: f32) -> String {
    format!("±{} yt", v as i32)
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
        widgets::envelope_graph(ui, env.attack, env.decay, env.sustain, env.release, Vec2::new(118.0, 92.0));
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
        // VU ballistics: the needle needs about 300 ms to settle (per 16 ms frame).
        self.vu += (peak - self.vu) * 0.12;
        if let Some(program) = self.status.take_program() {
            if !self.presets.is_empty() {
                self.select_preset(program as usize % self.presets.len());
            }
        }
        self.preset_dialogs(&ctx);

        if theme::t().wood {
            for (id, left) in [("wood_left", true), ("wood_right", false)] {
                let panel = if left { egui::Panel::left(id) } else { egui::Panel::right(id) };
                panel.exact_size(26.0).resizable(false).show_separator_line(false).frame(egui::Frame::new()).show(ui, |ui| {
                    theme::paint_wood(ui.painter(), ui.max_rect());
                });
            }
        }
        let pad = egui::Frame::new().fill(theme::t().bg).inner_margin(egui::Margin::same(14));
        egui::Panel::top("header").frame(pad).show(ui, |ui| self.header(ui));
        egui::Panel::bottom("keys").frame(pad).show(ui, |ui| self.keyboard(ui));
        egui::CentralPanel::default().frame(pad).show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| self.panel(ui));
        });

        // Meter, LED and lit keys follow the audio thread.
        ctx.request_repaint_after(Duration::from_millis(16));
    }
}
