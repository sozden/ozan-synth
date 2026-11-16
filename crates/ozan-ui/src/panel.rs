//! The sound panel shared by the standalone program and the plugin: every section
//! that edits a [`Patch`], plus the meters it shows.

use egui::{self, RichText, Vec2};
use ozan_core::wavetable::WavetableBank;
use ozan_core::{AdsrParams, ArpDivision, ArpMode, ArpParams, ModDest, ModSource, NoteLength, OscParams, Patch, SeqStep, Waveform, WavetableKind, SEQ_STEPS};

use crate::i18n::{self, s};
use crate::knob::{self, Knob};
use crate::theme;
use crate::widgets;

/// What the panel shows but does not edit, read from the audio side.
#[derive(Clone, Copy, Debug, Default)]
pub struct Meters<'a> {
    /// VU needle level and recent peak (linear).
    pub vu: f32,
    pub peak: f32,
    pub voices: u32,
    /// LFO values, -1..1.
    pub lfo: f32,
    pub lfo2: f32,
    pub wheel: f32,
    pub aftertouch: f32,
    /// Latest output samples, oldest first, for the scope.
    pub scope: &'a [f32],
    pub sample_rate: f32,
    /// Sequencer step sounding now.
    pub seq_step: Option<usize>,
}

/// Which group the tabbed lower part shows.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    Modulation,
    Effects,
    Play,
}

/// Outer widths for sections sharing a row: each gets its content width
/// (`content`) plus the section's own room, and the rest of the row is shared out
/// in proportion, so the row is filled edge to edge.
fn widths(ui: &egui::Ui, content: &[f32]) -> Vec<f32> {
    let gap = ui.spacing().item_spacing.x;
    let room = theme::section_overhead().x;
    let sum: f32 = content.iter().sum();
    let spare = (ui.available_width() - gap * (content.len() - 1) as f32 - room * content.len() as f32 - sum).max(0.0);
    content.iter().map(|c| (c + room + spare * c / sum).floor()).collect()
}

/// Content heights of the rows; every section in a row is as tall as the row.
const TOP: f32 = 136.0;
const MIDDLE: f32 = 158.0;
const LOWER: f32 = 206.0;

/// Outer height of a row whose content is `content` tall.
fn row(content: f32) -> f32 {
    content + theme::section_overhead().y
}

/// The lower part's tabs.
fn tab_bar(ui: &mut egui::Ui, tab: &mut Tab) {
    let th = theme::t();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for (t, label) in [(Tab::Modulation, s().tab_modulation), (Tab::Effects, s().tab_effects), (Tab::Play, s().tab_play)] {
            let (rect, response) = ui.allocate_exact_size(Vec2::new(170.0, 28.0), egui::Sense::click());
            let on = *tab == t;
            let fill = if on { th.accent_dark } else if response.hovered() { th.knob_hover } else { th.knob };
            ui.painter().rect_filled(rect, 4.0, fill);
            if !on {
                ui.painter().rect_stroke(rect, 4.0, egui::Stroke::new(1.0, th.track), egui::StrokeKind::Inside);
            }
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(13.5), if on { th.text } else { th.muted });
            if response.clicked() {
                *tab = t;
            }
        }
    });
}

/// Every section of the panel. Edits `p` in place; the caller compares it with a
/// copy taken before to see whether something changed.
///
/// Top: the sound path, always visible (oscillators, mix, filter, output; then the
/// envelopes and the scope). Below: tabs for modulation, effects, and playing.
pub fn panel(ui: &mut egui::Ui, p: &mut Patch, bank: &WavetableBank, m: &Meters) {
    ui.spacing_mut().item_spacing = Vec2::new(12.0, 12.0);
    // A wavetable oscillator adds a row (table and frame), so the whole top row grows.
    let tables = p.osc.iter().any(|o| o.wave == Waveform::Wavetable);
    let (top, middle) = (row(TOP + if tables { 28.0 } else { 0.0 }), row(MIDDLE));

    // Wavetable oscillators grow a position knob; each knob takes 66 points.
    let osc_width = |knobs: usize, wt: bool| (66.0 * (knobs + wt as usize) as f32).max(200.0);
    let osc1 = osc_width(3, p.osc[0].wave == Waveform::Wavetable);
    let osc2 = osc_width(4, p.osc[1].wave == Waveform::Wavetable);
    let w = widths(ui, &[osc1, osc2, 396.0, 330.0, 214.0]);
    ui.horizontal_top(|ui| {
        for (i, osc) in p.osc.iter_mut().enumerate() {
            let (key, title) = if i == 0 { ("osc1", s().osc1) } else { ("osc2", s().osc2) };
            theme::section_box(ui, key, title, Vec2::new(w[i], top), |ui| oscillator(ui, osc, i == 1, bank));
        }
        // Noise and unison both shape what goes into the filter.
        theme::section_box(ui, "unison", s().mix_unison, Vec2::new(w[2], top), |ui| {
            ui.add_space(32.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                Knob::keyed("noise", s().noise, &mut p.noise, 0.0, 1.0).default_value(0.0).format(percent).show(ui);
                let u = &mut p.unison;
                let mut copies = u.voices as f32;
                if Knob::keyed("unison_voices", s().unison_voices, &mut copies, 1.0, 7.0).integer().default_value(1.0).format(count).show(ui) {
                    u.voices = copies as u8;
                }
                Knob::keyed("detune", s().detune, &mut u.detune, 0.0, 100.0).default_value(20.0).format(cents_plain).show(ui);
                Knob::keyed("spread", s().spread, &mut u.spread, 0.0, 1.0).default_value(0.6).format(percent).show(ui);
                // Oscillator 2 bending (FM) and multiplying (ring) oscillator 1.
                Knob::keyed("fm", s().fm, &mut p.fm, 0.0, 1.0).default_value(0.0).format(percent).show(ui);
                Knob::keyed("ring", s().ring, &mut p.ring, 0.0, 1.0).default_value(0.0).format(percent).show(ui);
            });
        });
        theme::section_box(ui, "filter", s().filter, Vec2::new(w[3], top), |ui| {
            knob::filter_mode_selector(ui, &mut p.filter.mode);
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let f = &mut p.filter;
                Knob::keyed("cutoff", s().cutoff, &mut f.cutoff, 20.0, 20_000.0).log().default_value(800.0).format(hertz).show(ui);
                Knob::keyed("resonance", s().resonance, &mut f.resonance, 0.0, 1.0).default_value(0.3).format(percent).show(ui);
                Knob::keyed("env_amount", s().env_amount, &mut f.env_amount, -1.0, 1.0).bipolar().default_value(0.4).format(signed_percent).show(ui);
                Knob::keyed("key_track", s().key_track, &mut f.key_track, 0.0, 1.0).default_value(0.5).format(percent).show(ui);
                Knob::keyed("drive", s().drive, &mut f.drive, 0.0, 1.0).default_value(0.0).format(percent).show(ui);
            });
        });
        theme::section_box(ui, "output", s().output, Vec2::new(w[4], top), |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.add_space(14.0);
                    Knob::keyed("volume", s().volume, &mut p.volume, 0.0, 1.0).default_value(0.7).format(percent).show(ui);
                });
                // The meter fills the rest of the section.
                let width = (ui.available_width()).clamp(150.0, 230.0);
                widgets::vu_meter(ui, m.vu, m.peak, Vec2::new(width, 110.0));
            });
        });
    });

    let w = widths(ui, &[390.0, 390.0, 280.0, 300.0]);
    ui.horizontal_top(|ui| {
        theme::section_box(ui, "filter_env", s().filter_env, Vec2::new(w[0], middle), |ui| envelope(ui, &mut p.filter_env));
        theme::section_box(ui, "amp_env", s().amp_env, Vec2::new(w[1], middle), |ui| envelope(ui, &mut p.amp_env));
        // The second filter: how it joins the first, its type and its knobs.
        theme::section_box(ui, "filter2", s().filter2, Vec2::new(w[2], middle), |ui| {
            let f = &mut p.filter2;
            knob::routing_selector(ui, &mut f.routing);
            knob::filter_mode_selector(ui, &mut f.mode);
            ui.add_enabled_ui(f.routing != ozan_core::FilterRouting::Off, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    Knob::keyed("cutoff", s().cutoff, &mut f.cutoff, 20.0, 20_000.0).log().default_value(200.0).format(hertz).show(ui);
                    Knob::keyed("resonance", s().resonance, &mut f.resonance, 0.0, 1.0).default_value(0.1).format(percent).show(ui);
                    Knob::keyed("env_amount", s().env_amount, &mut f.env_amount, -1.0, 1.0).bipolar().default_value(0.0).format(signed_percent).show(ui);
                    Knob::keyed("key_track", s().key_track, &mut f.key_track, 0.0, 1.0).default_value(0.0).format(percent).show(ui);
                });
            });
        });
        theme::section_box(ui, "scope", s().scope, Vec2::new(w[3], middle), |ui| {
            let size = Vec2::new(ui.available_width(), 104.0);
            crate::scope::section(ui, m.scope, m.sample_rate, size)
        });
    });

    let id = ui.id().with("panel_tab");
    let mut tab = ui.data_mut(|d| *d.get_persisted_mut_or_default::<TabId>(id)).tab();
    tab_bar(ui, &mut tab);
    ui.data_mut(|d| d.insert_persisted(id, TabId::from(tab)));

    // The lower row takes whatever height is left in view.
    let left = ui.max_rect().bottom() - ui.cursor().top() - 2.0;
    let lower = row(LOWER).max(left);
    match tab {
        Tab::Modulation => {
            let w = widths(ui, &[264.0, 230.0, 390.0, 666.0]);
            ui.horizontal_top(|ui| {
                theme::section_box(ui, "lfo", s().lfo, Vec2::new(w[0], lower), |ui| {
                    ui.horizontal(|ui| {
                        knob::lfo_wave_selector(ui, &mut p.lfo.wave);
                        ui.add_space(6.0);
                        let th = theme::t();
                        widgets::led(ui, th.accent.gamma_multiply(0.12 + 0.88 * (0.5 + 0.5 * m.lfo))).on_hover_text(s().lfo_led);
                    });
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        let l = &mut p.lfo;
                        rate_or_length(ui, s().rate, &mut l.rate, 0.05, 20.0, 5.0, l.sync, &mut l.division);
                        Knob::keyed("vibrato", s().vibrato, &mut l.pitch, 0.0, 12.0).default_value(0.0).format(semitones).show(ui);
                        Knob::keyed("filter_knob", s().filter_knob, &mut l.cutoff, -1.0, 1.0).bipolar().default_value(0.0).format(signed_percent).show(ui);
                        Knob::keyed("tremolo", s().tremolo, &mut l.amp, 0.0, 1.0).default_value(0.0).format(percent).show(ui);
                    });
                    sync_switch(ui, &mut p.lfo.sync);
                });
                theme::section_box(ui, "lfo2", s().lfo2, Vec2::new(w[1], lower), |ui| {
                    ui.horizontal(|ui| {
                        knob::lfo_wave_selector(ui, &mut p.lfo2.wave);
                        ui.add_space(6.0);
                        let th = theme::t();
                        widgets::led(ui, th.accent.gamma_multiply(0.12 + 0.88 * (0.5 + 0.5 * m.lfo2))).on_hover_text(s().lfo2_hint);
                    });
                    ui.horizontal(|ui| {
                        let l = &mut p.lfo2;
                        rate_or_length(ui, s().rate, &mut l.rate, 0.02, 20.0, 0.5, l.sync, &mut l.division);
                        let size = Vec2::new(ui.available_width(), 80.0);
                        widgets::lcd_trace(ui, "lfo2_trace", m.lfo2, size);
                    });
                    sync_switch(ui, &mut p.lfo2.sync);
                });
                theme::section_box(ui, "mod_env", s().mod_env, Vec2::new(w[2], lower), |ui| {
                    ui.add_space(32.0);
                    envelope(ui, &mut p.mod_env)
                });
                theme::section_box(ui, "mod_matrix", s().mod_matrix, Vec2::new(w[3], lower), |ui| mod_matrix(ui, &mut p.mods));
            });
        }
        Tab::Effects => {
            let fx = &mut p.fx;
            // In the order of the chain.
            let w = widths(ui, &[200.0, 264.0, 198.0, 300.0, 330.0, 198.0]);
            ui.horizontal_top(|ui| {
                theme::section_box(ui, "distortion", s().distortion, Vec2::new(w[0], lower), |ui| {
                    let d = &mut fx.distortion;
                    widgets::toggle(ui, &mut d.on, s().on);
                    ui.add_space(4.0);
                    knob::drive_mode_selector(ui, &mut d.mode);
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        Knob::keyed("dist_drive", s().drive, &mut d.drive, 0.0, 1.0).default_value(0.4).format(percent).show(ui);
                        Knob::keyed("dist_tone", s().tone, &mut d.tone, 500.0, 16_000.0).log().default_value(6000.0).format(hertz).show(ui);
                        Knob::keyed("mix", s().mix, &mut d.mix, 0.0, 1.0).default_value(1.0).format(percent).show(ui);
                    });
                });
                theme::section_box(ui, "phaser", s().phaser, Vec2::new(w[1], lower), |ui| {
                    let ph = &mut fx.phaser;
                    widgets::toggle(ui, &mut ph.on, s().on);
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        Knob::keyed("rate", s().rate, &mut ph.rate, 0.05, 5.0).log().default_value(0.4).format(rate).show(ui);
                        Knob::keyed("depth", s().depth, &mut ph.depth, 0.0, 1.0).default_value(0.7).format(percent).show(ui);
                        Knob::keyed("feedback", s().feedback, &mut ph.feedback, 0.0, 0.9).default_value(0.5).format(percent).show(ui);
                        Knob::keyed("mix", s().mix, &mut ph.mix, 0.0, 1.0).default_value(1.0).format(percent).show(ui);
                    });
                });
                theme::section_box(ui, "chorus", s().chorus, Vec2::new(w[2], lower), |ui| {
                    widgets::toggle(ui, &mut fx.chorus.on, s().on);
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        let c = &mut fx.chorus;
                        Knob::keyed("rate", s().rate, &mut c.rate, 0.1, 5.0).log().default_value(0.6).format(rate).show(ui);
                        Knob::keyed("depth", s().depth, &mut c.depth, 0.0, 1.0).default_value(0.5).format(percent).show(ui);
                        Knob::keyed("mix", s().mix, &mut c.mix, 0.0, 1.0).default_value(0.7).format(percent).show(ui);
                    });
                });
                theme::section_box(ui, "delay", s().delay, Vec2::new(w[3], lower), |ui| {
                    ui.horizontal(|ui| {
                        widgets::toggle(ui, &mut fx.delay.on, s().on);
                        ui.add_space(12.0);
                        widgets::toggle(ui, &mut fx.delay.ping_pong, s().ping_pong);
                        ui.add_space(12.0);
                        sync_switch(ui, &mut fx.delay.sync);
                    });
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        let d = &mut fx.delay;
                        if d.sync {
                            length_knob(ui, s().time, &mut d.division);
                        } else {
                            Knob::keyed("time", s().time, &mut d.time_ms, 20.0, 2000.0).log().default_value(375.0).format(millis).show(ui);
                        }
                        Knob::keyed("feedback", s().feedback, &mut d.feedback, 0.0, 0.95).default_value(0.35).format(percent).show(ui);
                        Knob::keyed("tone", s().tone, &mut d.tone, 500.0, 16_000.0).log().default_value(5000.0).format(hertz).show(ui);
                        Knob::keyed("mix", s().mix, &mut d.mix, 0.0, 1.0).default_value(0.3).format(percent).show(ui);
                    });
                });
                theme::section_box(ui, "reverb", s().reverb, Vec2::new(w[4], lower), |ui| {
                    widgets::toggle(ui, &mut fx.reverb.on, s().on);
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        let r = &mut fx.reverb;
                        Knob::keyed("size", s().size, &mut r.size, 0.0, 1.0).default_value(0.5).format(percent).show(ui);
                        Knob::keyed("reverb_decay", s().reverb_decay, &mut r.decay, 0.3, 10.0).log().default_value(1.8).format(seconds).show(ui);
                        Knob::keyed("damping", s().damping, &mut r.damping, 0.0, 1.0).default_value(0.4).format(percent).show(ui);
                        Knob::keyed("predelay", s().predelay, &mut r.predelay_ms, 0.0, 100.0).default_value(10.0).format(millis).show(ui);
                        Knob::keyed("mix", s().mix, &mut r.mix, 0.0, 1.0).default_value(0.3).format(percent).show(ui);
                    });
                });
                theme::section_box(ui, "eq", s().eq, Vec2::new(w[5], lower), |ui| {
                    let e = &mut fx.eq;
                    widgets::toggle(ui, &mut e.on, s().on);
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        Knob::keyed("eq_low", s().eq_low, &mut e.low, -12.0, 12.0).bipolar().default_value(0.0).format(decibels).show(ui);
                        Knob::keyed("eq_mid", s().eq_mid, &mut e.mid, -12.0, 12.0).bipolar().default_value(0.0).format(decibels).show(ui);
                        Knob::keyed("eq_high", s().eq_high, &mut e.high, -12.0, 12.0).bipolar().default_value(0.0).format(decibels).show(ui);
                    });
                });
            });
        }
        Tab::Play => {
            let w = widths(ui, &[150.0, 250.0, 370.0, 600.0]);
            ui.horizontal_top(|ui| {
                theme::section_box(ui, "play", s().play, Vec2::new(w[0], lower), |ui| {
                    knob::voice_mode_selector(ui, &mut p.play.mode);
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        Knob::keyed("glide", s().glide, &mut p.play.glide, 0.0, 2.0).default_value(0.0).format(seconds).show(ui);
                        Knob::keyed("velocity", s().velocity, &mut p.play.velocity, 0.0, 1.0).default_value(1.0).format(percent).show(ui);
                    });
                });
                theme::section_box(ui, "wheel_section", s().wheel_section, Vec2::new(w[1], lower), |ui| {
                    ui.horizontal(|ui| {
                        widgets::mini_bar(ui, s().wheel, m.wheel);
                        ui.add_space(8.0);
                        widgets::mini_bar(ui, s().aftertouch, m.aftertouch);
                    });
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        Knob::keyed("vibrato", s().vibrato, &mut p.wheel.vibrato, 0.0, 2.0).default_value(0.5).format(semitones).show(ui);
                        Knob::keyed("filter_knob", s().filter_knob, &mut p.wheel.cutoff, -1.0, 1.0).bipolar().default_value(0.0).format(signed_percent).show(ui);
                        Knob::keyed("bend", s().bend, &mut p.bend_range, 0.0, 12.0).integer().default_value(2.0).format(bend).show(ui);
                    });
                });
                theme::section_box(ui, "arp", s().arp, Vec2::new(w[2], lower), |ui| arpeggiator(ui, &mut p.arp));
                theme::section_box(ui, "sequencer", s().sequencer, Vec2::new(w[3], lower), |ui| sequencer(ui, &mut p.arp, m.seq_step));
            });
        }
    }
}

/// The tab, stored in egui's memory between frames (and runs).
#[derive(Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
struct TabId(u8);

impl TabId {
    fn tab(self) -> Tab {
        match self.0 {
            1 => Tab::Effects,
            2 => Tab::Play,
            _ => Tab::Modulation,
        }
    }
}

impl From<Tab> for TabId {
    fn from(t: Tab) -> Self {
        TabId(match t {
            Tab::Modulation => 0,
            Tab::Effects => 1,
            Tab::Play => 2,
        })
    }
}

/// Label of a note length knob position.
fn note_length(v: f32) -> String {
    NoteLength::ALL[(v.max(0.0) as usize).min(NoteLength::ALL.len() - 1)].label().to_string()
}

/// A knob stepping through note lengths, longest at the left.
fn length_knob(ui: &mut egui::Ui, label: &str, length: &mut NoteLength) {
    let mut i = length.index() as f32;
    let last = (NoteLength::ALL.len() - 1) as f32;
    let default = NoteLength::Quarter.index() as f32;
    if Knob::keyed("length", label, &mut i, 0.0, last).integer().default_value(default).format(note_length).show(ui) {
        *length = NoteLength::ALL[i as usize];
    }
}

/// A rate in Hz, or a note length when locked to the tempo.
#[allow(clippy::too_many_arguments)]
fn rate_or_length(ui: &mut egui::Ui, label: &str, hz: &mut f32, min: f32, max: f32, default: f32, sync: bool, length: &mut NoteLength) {
    if sync {
        length_knob(ui, label, length);
    } else {
        Knob::keyed("rate", label, hz, min, max).log().default_value(default).format(rate).show(ui);
    }
}

/// Switch locking a rate or time to the tempo.
fn sync_switch(ui: &mut egui::Ui, on: &mut bool) {
    ui.horizontal(|ui| widgets::toggle(ui, on, s().sync)).response.on_hover_text(s().sync_hint);
}

fn decibels(v: f32) -> String {
    format!("{v:+.1} dB")
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
    format!("{v:.2} {}", s().semitone_unit)
}

fn bend(v: f32) -> String {
    format!("±{} {}", v as i32, s().semitone_unit)
}

fn signed_int(v: f32) -> String {
    format!("{:+}", v as i32)
}

fn cents(v: f32) -> String {
    format!("{v:+.0} ct")
}

/// Waveform buttons and pitch/level knobs of one oscillator. Only the second one
/// gets a semitone knob (for intervals like a fifth).
pub fn oscillator(ui: &mut egui::Ui, osc: &mut OscParams, semitones: bool, bank: &WavetableBank) {
    knob::wave_selector(ui, &mut osc.wave);
    if osc.wave == Waveform::Wavetable {
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt(("table", semitones)).selected_text(i18n::table_name(osc.table)).width(118.0).show_ui(ui, |ui| {
                for kind in WavetableKind::ALL {
                    ui.selectable_value(&mut osc.table, kind, i18n::table_name(kind));
                }
            });
            widgets::wave_display(ui, bank.get(osc.table).frame(osc.position), Vec2::new(ui.available_width().min(96.0), 22.0));
        });
    }
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let mut octave = osc.octave as f32;
        if Knob::keyed("octave", s().octave, &mut octave, -3.0, 3.0).integer().default_value(0.0).bipolar().format(signed_int).show(ui) {
            osc.octave = octave as i8;
        }
        if semitones {
            let mut semi = osc.semitones as f32;
            if Knob::keyed("semitone", s().semitone, &mut semi, -12.0, 12.0).integer().default_value(0.0).bipolar().format(signed_int).show(ui) {
                osc.semitones = semi as i8;
            }
        }
        Knob::keyed("fine", s().fine, &mut osc.fine, -50.0, 50.0).default_value(0.0).bipolar().format(cents).show(ui);
        Knob::keyed("level", s().level, &mut osc.level, 0.0, 1.0).format(percent).show(ui);
        if osc.wave == Waveform::Wavetable {
            Knob::keyed("position", s().position, &mut osc.position, 0.0, 1.0).default_value(0.0).format(percent).show(ui);
        }
    });
}

/// Destinations in the order of the pin matrix columns, with their short codes.
const DESTS: [(ModDest, &str); 15] = [
    (ModDest::Pitch, "PIT"),
    (ModDest::Osc1Pitch, "P1"),
    (ModDest::Osc2Pitch, "P2"),
    (ModDest::Osc1Level, "L1"),
    (ModDest::Osc2Level, "L2"),
    (ModDest::Osc1Position, "W1"),
    (ModDest::Osc2Position, "W2"),
    (ModDest::Fm, "FM"),
    (ModDest::UnisonDetune, "DET"),
    (ModDest::Cutoff, "CUT"),
    (ModDest::Resonance, "RES"),
    (ModDest::Cutoff2, "CT2"),
    (ModDest::Drive, "DRV"),
    (ModDest::Volume, "VOL"),
    (ModDest::Pan, "PAN"),
];

/// Pin matrix cell size, the width of the source names and of the screen beside it.
const CELL: Vec2 = Vec2::new(26.0, 18.0);
const NAMES: f32 = 96.0;
const HEAD: f32 = 20.0;
const SCREEN: Vec2 = Vec2::new(170.0, 88.0);

/// Colour of a pin carrying `amount`: the accent for positive, a cool blue for negative.
fn pin_color(amount: f32) -> egui::Color32 {
    let th = theme::t();
    let base = if amount >= 0.0 { th.accent } else { egui::Color32::from_rgb(0x5b, 0x9b, 0xd0) };
    base.gamma_multiply(0.35 + 0.65 * amount.abs().min(1.0))
}

/// The modulation matrix as a pin board, sources down the side and destinations
/// across: click a hole to connect or disconnect, drag a pin up and down (Shift for
/// fine) or use the mouse wheel to set its amount. The engine has as many
/// connections as `slots`; the screen beside the board describes the pin under the
/// mouse.
pub fn mod_matrix(ui: &mut egui::Ui, slots: &mut [ozan_core::ModSlot]) {
    let th = theme::t();
    let sources = &ModSource::ALL[1..];
    let size = Vec2::new(NAMES + CELL.x * DESTS.len() as f32, HEAD + CELL.y * sources.len() as f32);
    ui.horizontal_top(|ui| {
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
        let id = response.id;
        let grid = egui::Rect::from_min_size(rect.min + Vec2::new(NAMES, HEAD), Vec2::new(CELL.x * DESTS.len() as f32, CELL.y * sources.len() as f32));
        let cell_at = |pos: egui::Pos2| -> Option<(usize, usize)> {
            if !grid.contains(pos) {
                return None;
            }
            let c = ((pos.x - grid.left()) / CELL.x) as usize;
            let r = ((pos.y - grid.top()) / CELL.y) as usize;
            (r < sources.len() && c < DESTS.len()).then_some((r, c))
        };
        let center = |r: usize, c: usize| grid.min + Vec2::new((c as f32 + 0.5) * CELL.x, (r as f32 + 0.5) * CELL.y);
        let find = |slots: &[ozan_core::ModSlot], r: usize, c: usize| slots.iter().position(|s| s.source == sources[r] && s.dest == DESTS[c].0);

        // The pin being dragged stays chosen even if the pointer leaves its hole.
        let dragging: Option<(usize, usize)> = ui.data(|d| d.get_temp(id));
        let hovered = dragging.or_else(|| response.hover_pos().and_then(cell_at));
        let mut full = false;
        let mut connect = |slots: &mut [ozan_core::ModSlot], r: usize, c: usize| -> Option<usize> {
            if let Some(i) = find(slots, r, c) {
                return Some(i);
            }
            let free = slots.iter().position(|s| s.source == ModSource::Off);
            match free {
                Some(i) => {
                    slots[i] = ozan_core::ModSlot { source: sources[r], dest: DESTS[c].0, amount: 0.5 };
                    Some(i)
                }
                None => {
                    full = true;
                    None
                }
            }
        };
        if response.drag_started() {
            if let Some((r, c)) = response.interact_pointer_pos().and_then(cell_at) {
                if connect(slots, r, c).is_some() {
                    ui.data_mut(|d| d.insert_temp(id, (r, c)));
                }
            }
        }
        if let Some((r, c)) = dragging {
            if response.dragged() {
                if let Some(i) = find(slots, r, c) {
                    let fine = ui.input(|i| i.modifiers.shift);
                    let a = &mut slots[i].amount;
                    *a = (*a - response.drag_delta().y / if fine { 600.0 } else { 120.0 }).clamp(-1.0, 1.0);
                }
            } else {
                ui.data_mut(|d| d.remove::<(usize, usize)>(id));
            }
        }
        if response.clicked() {
            if let Some((r, c)) = response.interact_pointer_pos().and_then(cell_at) {
                match find(slots, r, c) {
                    Some(i) => slots[i] = ozan_core::ModSlot { source: ModSource::Off, ..slots[i] },
                    None => {
                        connect(slots, r, c);
                    }
                }
            }
        }
        if let Some((r, c)) = hovered {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                if let Some(i) = find(slots, r, c) {
                    slots[i].amount = (slots[i].amount + scroll / 600.0).clamp(-1.0, 1.0);
                }
            }
        }

        let painter = ui.painter_at(rect.expand(4.0));
        // Cross-hair under the pointer.
        if let Some((r, c)) = hovered {
            let row = egui::Rect::from_min_size(egui::pos2(rect.left(), grid.top() + r as f32 * CELL.y), Vec2::new(rect.width(), CELL.y));
            let col = egui::Rect::from_min_size(egui::pos2(grid.left() + c as f32 * CELL.x, rect.top()), Vec2::new(CELL.x, rect.height()));
            painter.rect_filled(row, 2.0, egui::Color32::from_white_alpha(6));
            painter.rect_filled(col, 2.0, egui::Color32::from_white_alpha(6));
        }
        for (c, (_, code)) in DESTS.iter().enumerate() {
            let on = hovered.is_some_and(|h| h.1 == c);
            painter.text(egui::pos2(grid.left() + (c as f32 + 0.5) * CELL.x, rect.top() + HEAD / 2.0 - 1.0), egui::Align2::CENTER_CENTER, *code, egui::FontId::monospace(12.0), if on { th.text } else { th.muted });
        }
        for (r, src) in sources.iter().enumerate() {
            let on = hovered.is_some_and(|h| h.0 == r);
            let y = grid.top() + (r as f32 + 0.5) * CELL.y;
            painter.text(egui::pos2(rect.left(), y), egui::Align2::LEFT_CENTER, i18n::source_name(*src), egui::FontId::proportional(14.0), if on { th.text } else { th.muted });
            // A thin bus line along each row, as on a patch panel.
            painter.line_segment([egui::pos2(grid.left() + 4.0, y), egui::pos2(grid.right() - 4.0, y)], egui::Stroke::new(1.0, th.track.gamma_multiply(0.6)));
        }
        for r in 0..sources.len() {
            for c in 0..DESTS.len() {
                let p = center(r, c);
                match find(slots, r, c) {
                    Some(i) => {
                        // A plugged pin: shadow, coloured head, glint.
                        let amount = slots[i].amount;
                        painter.circle_filled(p + Vec2::new(1.0, 1.5), 7.0, egui::Color32::from_black_alpha(110));
                        painter.circle_filled(p, 6.8, pin_color(amount));
                        painter.circle_stroke(p, 6.8, egui::Stroke::new(1.0, egui::Color32::from_black_alpha(80)));
                        painter.circle_filled(p + Vec2::new(-2.2, -2.2), 2.0, egui::Color32::from_white_alpha(110));
                    }
                    None => {
                        painter.circle_filled(p, 3.6, th.track);
                        painter.circle_filled(p, 1.6, th.bg);
                    }
                }
                if hovered == Some((r, c)) {
                    painter.circle_stroke(p, 8.0, egui::Stroke::new(1.2, th.text));
                }
            }
        }

        // Screen describing the pin under the pointer.
        ui.add_space(10.0);
        ui.vertical(|ui| {
            let (screen, _) = ui.allocate_exact_size(SCREEN, egui::Sense::hover());
            let painter = ui.painter_at(screen.expand(2.0));
            crate::lcd::glass(&painter, screen);
            let ink = th.lcd_ink;
            let at = |y: f32| egui::pos2(screen.left() + 8.0, screen.top() + y);
            let used = slots.iter().filter(|s| s.source != ModSource::Off).count();
            match hovered {
                Some((r, c)) => {
                    painter.text(at(8.0), egui::Align2::LEFT_TOP, i18n::source_name(sources[r]).to_uppercase(), egui::FontId::monospace(13.0), ink);
                    painter.text(at(26.0), egui::Align2::LEFT_TOP, format!("→ {}", i18n::dest_name(DESTS[c].0).to_uppercase()), egui::FontId::monospace(13.0), ink);
                    match find(slots, r, c) {
                        Some(i) => {
                            let a = slots[i].amount;
                            let bar = egui::Rect::from_min_size(at(46.0), Vec2::new(screen.width() - 16.0, 8.0));
                            crate::lcd::bar_graph(&painter, bar, 0.5 + 0.5 * a, true, ink);
                            painter.text(at(62.0), egui::Align2::LEFT_TOP, format!("{:+.2}", a), egui::FontId::monospace(13.0), ink);
                            crate::lcd::touch(&format!("{} → {}", i18n::source_name(sources[r]), i18n::dest_name(DESTS[c].0)), format!("{a:+.2}"), 0.5 + 0.5 * a, true, ui.input(|i| i.time));
                        }
                        None => {
                            let text = if used == slots.len() { s().pins_full } else { s().pins_connect };
                            painter.text(at(52.0), egui::Align2::LEFT_TOP, text.to_uppercase(), egui::FontId::monospace(12.0), ink.gamma_multiply(0.6));
                        }
                    }
                }
                None => {
                    painter.text(at(8.0), egui::Align2::LEFT_TOP, s().mod_matrix.to_uppercase(), egui::FontId::monospace(13.0), ink);
                    painter.text(at(28.0), egui::Align2::LEFT_TOP, i18n::fill(s().pins_used, &[&used, &slots.len()]), egui::FontId::monospace(13.0), ink.gamma_multiply(0.8));
                    let bar = egui::Rect::from_min_size(at(48.0), Vec2::new(screen.width() - 16.0, 8.0));
                    crate::lcd::bar_graph(&painter, bar, used as f32 / slots.len() as f32, false, ink);
                }
            }
            ui.add_space(4.0);
            ui.label(RichText::new(s().pins_hint).size(13.0).color(th.muted));
        });
        if full {
            response.on_hover_text(s().pins_full);
        }
    });
}

fn count(v: f32) -> String {
    format!("{}", v as i32)
}

fn cents_plain(v: f32) -> String {
    format!("{v:.0} ct")
}

/// Arpeggiator switches, pattern, step length, octaves, gate and tempo.
pub fn arpeggiator(ui: &mut egui::Ui, a: &mut ArpParams) {
    ui.horizontal(|ui| {
        widgets::toggle(ui, &mut a.on, s().on);
        ui.add_space(12.0);
        widgets::toggle(ui, &mut a.hold, s().hold);
    });
    ui.horizontal(|ui| {
        knob::arp_mode_selector(ui, &mut a.mode);
        ui.add_space(6.0);
        egui::ComboBox::from_id_salt("arp_division").selected_text(a.division.label()).width(58.0).show_ui(ui, |ui| {
            for d in ArpDivision::ALL {
                ui.selectable_value(&mut a.division, d, d.label());
            }
        })
        .response
        .on_hover_text(s().step);
    });
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let mut octaves = a.octaves as f32;
        if Knob::keyed("octaves", s().octaves, &mut octaves, 1.0, 4.0).integer().default_value(1.0).format(count).show(ui) {
            a.octaves = octaves as u8;
        }
        Knob::keyed("gate", s().gate, &mut a.gate, 0.05, 1.0).default_value(0.5).format(percent).show(ui);
        Knob::keyed("tempo", s().tempo, &mut a.bpm, 40.0, 240.0).default_value(120.0).format(bpm).show(ui);
    });
}

/// Left column of the sequencer, with the row names.
const SEQ_GUTTER: f32 = 58.0;
const SEQ_NUMBERS: f32 = 20.0;
const SEQ_SWITCH: f32 = 22.0;

/// The step sequencer: a column per step with its note (drag it up or down, the
/// mouse wheel, double-click for the key itself) and switches to play, accent and
/// tie it. A click on a step number ends the pattern there. `playing` is the step
/// sounding now.
pub fn sequencer(ui: &mut egui::Ui, a: &mut ArpParams, playing: Option<usize>) {
    let th = theme::t();
    ui.horizontal(|ui| {
        let mut on = a.on && a.mode == ArpMode::Sequence;
        if widgets::toggle(ui, &mut on, s().on) {
            a.on = on;
            if on {
                a.mode = ArpMode::Sequence;
            }
        }
        ui.add_space(14.0);
        ui.label(RichText::new(i18n::fill(s().seq_length, &[&a.length.clamp(1, SEQ_STEPS as u8)])).monospace().color(th.muted));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button(s().seq_clear).on_hover_text(s().seq_clear_hint).clicked() {
                a.steps = [SeqStep::default(); SEQ_STEPS];
                a.length = SEQ_STEPS as u8;
            }
            if ui.button(s().seq_random).on_hover_text(s().seq_random_hint).clicked() {
                a.steps = ozan_core::random::steps(crate::dice::seed());
            }
        });
    });
    ui.add_space(4.0);

    let cols = SEQ_STEPS as f32;
    let col = ((ui.available_width() - SEQ_GUTTER) / cols).floor().max(18.0);
    let bar_h = (ui.available_height() - SEQ_NUMBERS - 3.0 * SEQ_SWITCH - 30.0).clamp(64.0, 150.0);
    let size = Vec2::new(SEQ_GUTTER + col * cols, SEQ_NUMBERS + 4.0 + bar_h + 6.0 + 3.0 * SEQ_SWITCH);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
    let id = response.id;
    let left = rect.left() + SEQ_GUTTER;
    let numbers = egui::Rect::from_min_size(egui::pos2(left, rect.top()), Vec2::new(col * cols, SEQ_NUMBERS));
    let bars = egui::Rect::from_min_size(egui::pos2(left, numbers.bottom() + 4.0), Vec2::new(col * cols, bar_h));
    let switches = egui::Rect::from_min_size(egui::pos2(left, bars.bottom() + 6.0), Vec2::new(col * cols, 3.0 * SEQ_SWITCH));
    let column = |x: f32| -> Option<usize> {
        let c = ((x - left) / col).floor();
        (c >= 0.0 && c < cols).then_some(c as usize)
    };
    let length = a.length.clamp(1, SEQ_STEPS as u8) as usize;
    // ±24 semitones fill the bar; the key itself is the middle line.
    let mid = bars.center().y;
    let per_semitone = (bar_h / 2.0 - 4.0) / 24.0;
    let time = ui.input(|i| i.time);
    let show_note = |c: usize, note: i8| crate::lcd::touch(&format!("{} {}", s().seq_note, c + 1), format!("{note:+}"), 0.5 + note as f32 / 48.0, true, time);

    // Input.
    if let Some(p) = response.interact_pointer_pos() {
        if let Some(c) = column(p.x) {
            if response.clicked() {
                if numbers.contains(p) {
                    a.length = (c + 1) as u8;
                } else if switches.contains(p) {
                    let step = &mut a.steps[c];
                    match ((p.y - switches.top()) / SEQ_SWITCH) as usize {
                        0 => step.on = !step.on,
                        1 => step.accent = !step.accent,
                        _ => step.tie = !step.tie,
                    }
                }
            }
            if response.double_clicked() && bars.contains(p) {
                a.steps[c].note = 0;
                show_note(c, 0);
            }
            if response.drag_started() && bars.contains(p) {
                ui.data_mut(|d| d.insert_temp(id, (c, a.steps[c].note as f32)));
            }
        }
    }
    if response.dragged() {
        if let Some((c, v)) = ui.data(|d| d.get_temp::<(usize, f32)>(id)) {
            let fine = ui.input(|i| i.modifiers.shift);
            let v = (v - response.drag_delta().y / if fine { 16.0 } else { 6.0 }).clamp(-24.0, 24.0);
            ui.data_mut(|d| d.insert_temp(id, (c, v)));
            a.steps[c].note = v.round() as i8;
            show_note(c, a.steps[c].note);
        }
    } else {
        ui.data_mut(|d| d.remove::<(usize, f32)>(id));
    }
    if let Some(h) = response.hover_pos().filter(|h| bars.contains(*h)) {
        let scroll = ui.input(|i| i.raw_scroll_delta.y);
        if let (Some(c), true) = (column(h.x), scroll != 0.0) {
            let n = &mut a.steps[c].note;
            *n = (*n + scroll.signum() as i8).clamp(-24, 24);
            show_note(c, *n);
        }
    }

    // Drawing.
    let painter = ui.painter_at(rect.expand(2.0));
    let label = |y: f32, text: &str| {
        painter.text(egui::pos2(rect.left(), y), egui::Align2::LEFT_CENTER, text, egui::FontId::proportional(13.0), th.muted);
    };
    label(bars.center().y, s().seq_note);
    for (k, text) in [s().seq_on, s().seq_accent, s().seq_tie].into_iter().enumerate() {
        label(switches.top() + (k as f32 + 0.5) * SEQ_SWITCH, text);
    }
    painter.rect_filled(bars, 4.0, th.screen);
    for c in 0..SEQ_STEPS {
        let step = a.steps[c];
        let x0 = left + c as f32 * col;
        let cx = x0 + col / 2.0;
        let inside = c < length;
        let fade = |color: egui::Color32| if inside { color } else { color.gamma_multiply(0.3) };
        // Step numbers, in groups of four; the sounding step lights up.
        let num = egui::Rect::from_min_size(egui::pos2(x0 + 1.0, numbers.top()), Vec2::new(col - 2.0, SEQ_NUMBERS));
        let now = playing == Some(c);
        let fill = if now { th.accent } else if (c / 4) % 2 == 0 { th.knob } else { th.knob_hover };
        painter.rect_filled(num, 3.0, fade(fill));
        painter.text(num.center(), egui::Align2::CENTER_CENTER, format!("{}", c + 1), egui::FontId::monospace(12.0), if now { th.bg } else { fade(th.text) });
        if now {
            painter.rect_filled(egui::Rect::from_x_y_ranges(x0..=x0 + col, bars.y_range()), 0.0, egui::Color32::from_white_alpha(10));
        }
        // The note as a bar from the middle line.
        let y = mid - step.note as f32 * per_semitone;
        let bar = egui::Rect::from_x_y_ranges(x0 + 4.0..=x0 + col - 4.0, y.min(mid - 1.5)..=y.max(mid + 1.5));
        let ink = if step.on { th.accent } else { th.muted.gamma_multiply(0.5) };
        painter.rect_filled(bar, 2.0, fade(ink));
        painter.text(egui::pos2(cx, bars.top() + 9.0), egui::Align2::CENTER_CENTER, if step.note == 0 { "0".to_string() } else { format!("{:+}", step.note) }, egui::FontId::monospace(11.5), fade(if step.on { th.text } else { th.muted }));
        // Play, accent and tie.
        let row = |k: usize| switches.top() + (k as f32 + 0.5) * SEQ_SWITCH;
        painter.circle_filled(egui::pos2(cx, row(0)), 5.5, fade(if step.on { th.accent } else { th.track }));
        let square = egui::Rect::from_center_size(egui::pos2(cx, row(1)), Vec2::splat(10.0));
        painter.rect_filled(square, 2.0, fade(if step.accent { th.warm } else { th.track }));
        if step.tie {
            let end = if c + 1 < SEQ_STEPS { cx + col } else { x0 + col };
            painter.line_segment([egui::pos2(cx, row(2)), egui::pos2(end, row(2))], egui::Stroke::new(4.0, fade(th.accent)));
            painter.circle_filled(egui::pos2(cx, row(2)), 3.5, fade(th.accent));
        } else {
            painter.line_segment([egui::pos2(cx - 5.0, row(2)), egui::pos2(cx + 5.0, row(2))], egui::Stroke::new(3.0, fade(th.track)));
        }
    }
    painter.line_segment([egui::pos2(bars.left() + 2.0, mid), egui::pos2(bars.right() - 2.0, mid)], egui::Stroke::new(1.0, th.track));
    ui.label(RichText::new(s().seq_hint).size(12.5).color(th.muted));
}

fn bpm(v: f32) -> String {
    format!("{v:.0} BPM")
}

/// Four ADSR knobs and the shape they make.
pub fn envelope(ui: &mut egui::Ui, env: &mut AdsrParams) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        Knob::keyed("attack", s().attack, &mut env.attack, 0.001, 5.0).log().format(seconds).show(ui);
        Knob::keyed("decay", s().decay, &mut env.decay, 0.005, 5.0).log().format(seconds).show(ui);
        Knob::keyed("sustain", s().sustain, &mut env.sustain, 0.0, 1.0).format(percent).show(ui);
        Knob::keyed("release", s().release, &mut env.release, 0.005, 8.0).log().format(seconds).show(ui);
        ui.add_space(8.0);
        widgets::envelope_graph(ui, env.attack, env.decay, env.sustain, env.release, Vec2::new(118.0, 92.0));
    });
}
