//! The sound panel shared by the standalone program and the plugin: every section
//! that edits a [`Patch`], plus the meters it shows.

use egui::{self, RichText, Vec2};
use ozan_core::wavetable::WavetableBank;
use ozan_core::{AdsrParams, ArpDivision, ArpParams, ModDest, ModSource, OscParams, Patch, Waveform, WavetableKind};

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
const MIDDLE: f32 = 126.0;
const LOWER: f32 = 178.0;

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
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(12.5), if on { th.text } else { th.muted });
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
    let (top, middle) = (row(TOP), row(MIDDLE));

    let w = widths(ui, &[200.0, 250.0, 250.0, 310.0, 282.0]);
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
            ui.add_space(32.0);
            ui.horizontal(|ui| {
                Knob::keyed("volume", s().volume, &mut p.volume, 0.0, 1.0).default_value(0.7).format(percent).show(ui);
                widgets::vu_meter(ui, m.vu, m.peak, Vec2::new(128.0, 92.0));
                ui.vertical(|ui| {
                    ui.label(RichText::new(s().voices).size(11.0).color(theme::t().muted));
                    ui.label(RichText::new(format!("{} / {}", m.voices, ozan_core::MAX_VOICES)).monospace().size(16.0));
                    ui.add_space(8.0);
                    if ui.button(s().init).on_hover_text(s().init_hint).clicked() {
                        *p = Patch::default();
                    }
                });
            });
        });
    });

    let w = widths(ui, &[370.0, 370.0, 500.0]);
    ui.horizontal_top(|ui| {
        theme::section_box(ui, "filter_env", s().filter_env, Vec2::new(w[0], middle), |ui| envelope(ui, &mut p.filter_env));
        theme::section_box(ui, "amp_env", s().amp_env, Vec2::new(w[1], middle), |ui| envelope(ui, &mut p.amp_env));
        theme::section_box(ui, "scope", s().scope, Vec2::new(w[2], middle), |ui| {
            let size = Vec2::new(ui.available_width(), 92.0);
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
            let w = widths(ui, &[250.0, 210.0, 370.0, 400.0]);
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
                        Knob::keyed("rate", s().rate, &mut l.rate, 0.05, 20.0).log().default_value(5.0).format(rate).show(ui);
                        Knob::keyed("vibrato", s().vibrato, &mut l.pitch, 0.0, 12.0).default_value(0.0).format(semitones).show(ui);
                        Knob::keyed("filter_knob", s().filter_knob, &mut l.cutoff, -1.0, 1.0).bipolar().default_value(0.0).format(signed_percent).show(ui);
                        Knob::keyed("tremolo", s().tremolo, &mut l.amp, 0.0, 1.0).default_value(0.0).format(percent).show(ui);
                    });
                });
                theme::section_box(ui, "lfo2", s().lfo2, Vec2::new(w[1], lower), |ui| {
                    ui.horizontal(|ui| {
                        knob::lfo_wave_selector(ui, &mut p.lfo2.wave);
                        ui.add_space(6.0);
                        let th = theme::t();
                        widgets::led(ui, th.accent.gamma_multiply(0.12 + 0.88 * (0.5 + 0.5 * m.lfo2))).on_hover_text(s().lfo2_hint);
                    });
                    Knob::keyed("rate", s().rate, &mut p.lfo2.rate, 0.02, 20.0).log().default_value(0.5).format(rate).show(ui);
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
            let w = widths(ui, &[190.0, 250.0, 310.0]);
            ui.horizontal_top(|ui| {
                theme::section_box(ui, "chorus", s().chorus, Vec2::new(w[0], lower), |ui| {
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
                theme::section_box(ui, "delay", s().delay, Vec2::new(w[1], lower), |ui| {
                    ui.horizontal(|ui| {
                        widgets::toggle(ui, &mut fx.delay.on, s().on);
                        ui.add_space(12.0);
                        widgets::toggle(ui, &mut fx.delay.ping_pong, s().ping_pong);
                    });
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        let d = &mut fx.delay;
                        Knob::keyed("time", s().time, &mut d.time_ms, 20.0, 2000.0).log().default_value(375.0).format(millis).show(ui);
                        Knob::keyed("feedback", s().feedback, &mut d.feedback, 0.0, 0.95).default_value(0.35).format(percent).show(ui);
                        Knob::keyed("tone", s().tone, &mut d.tone, 500.0, 16_000.0).log().default_value(5000.0).format(hertz).show(ui);
                        Knob::keyed("mix", s().mix, &mut d.mix, 0.0, 1.0).default_value(0.3).format(percent).show(ui);
                    });
                });
                theme::section_box(ui, "reverb", s().reverb, Vec2::new(w[2], lower), |ui| {
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
            });
        }
        Tab::Play => {
            let w = widths(ui, &[150.0, 250.0, 330.0]);
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

/// Source, destination and amount for each matrix slot.
pub fn mod_matrix(ui: &mut egui::Ui, slots: &mut [ozan_core::ModSlot]) {
    let th = theme::t();
    egui::Grid::new("mod_matrix").num_columns(3).spacing(Vec2::new(8.0, 4.0)).show(ui, |ui| {
        for text in [s().source, s().destination, s().amount] {
            ui.label(RichText::new(text).size(11.0).color(th.muted));
        }
        ui.end_row();
        for (i, slot) in slots.iter_mut().enumerate() {
            let active = slot.source != ModSource::Off;
            egui::ComboBox::from_id_salt(("mod_src", i)).selected_text(i18n::source_name(slot.source)).width(108.0).show_ui(ui, |ui| {
                for src in ModSource::ALL {
                    ui.selectable_value(&mut slot.source, src, i18n::source_name(src));
                }
            });
            ui.add_enabled_ui(active, |ui| {
                egui::ComboBox::from_id_salt(("mod_dst", i)).selected_text(i18n::dest_name(slot.dest)).width(124.0).show_ui(ui, |ui| {
                    for dst in ModDest::ALL {
                        ui.selectable_value(&mut slot.dest, dst, i18n::dest_name(dst));
                    }
                });
            });
            ui.add_enabled(
                active,
                egui::Slider::new(&mut slot.amount, -1.0..=1.0).fixed_decimals(2).trailing_fill(true),
            )
            .on_hover_text("Double-click: 0");
            ui.end_row();
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
