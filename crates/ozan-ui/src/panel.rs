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
pub struct Meters {
    /// VU needle level and recent peak (linear).
    pub vu: f32,
    pub peak: f32,
    pub voices: u32,
    /// LFO value, -1..1.
    pub lfo: f32,
    pub wheel: f32,
    pub aftertouch: f32,
}

/// Every section of the panel. Edits `p` in place; the caller compares it with a
/// copy taken before to see whether something changed.
pub fn panel(ui: &mut egui::Ui, p: &mut Patch, bank: &WavetableBank, m: &Meters) {
    let lfo_now = m.lfo;
    let (wheel, aftertouch) = (m.wheel, m.aftertouch);
    ui.spacing_mut().item_spacing = Vec2::new(12.0, 12.0);
    ui.horizontal_top(|ui| {
        for (i, osc) in p.osc.iter_mut().enumerate() {
            let title = if i == 0 { s().osc1 } else { s().osc2 };
            theme::section(ui, title, |ui| oscillator(ui, osc, i == 1, bank));
        }
        theme::section(ui, s().filter, |ui| {
            knob::filter_mode_selector(ui, &mut p.filter.mode);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let f = &mut p.filter;
                Knob::new(s().cutoff, &mut f.cutoff, 20.0, 20_000.0).log().default_value(800.0).format(hertz).show(ui);
                Knob::new(s().resonance, &mut f.resonance, 0.0, 1.0).default_value(0.3).format(percent).show(ui);
                Knob::new(s().env_amount, &mut f.env_amount, -1.0, 1.0).bipolar().default_value(0.4).format(signed_percent).show(ui);
                Knob::new(s().key_track, &mut f.key_track, 0.0, 1.0).default_value(0.5).format(percent).show(ui);
            });
        });
        theme::section(ui, s().unison, |ui| {
            ui.add_space(30.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let u = &mut p.unison;
                let mut copies = u.voices as f32;
                if Knob::new(s().unison_voices, &mut copies, 1.0, 7.0).integer().default_value(1.0).format(count).show(ui) {
                    u.voices = copies as u8;
                }
                Knob::new(s().detune, &mut u.detune, 0.0, 100.0).default_value(20.0).format(cents_plain).show(ui);
                Knob::new(s().spread, &mut u.spread, 0.0, 1.0).default_value(0.6).format(percent).show(ui);
            });
        });
    });
    ui.horizontal_top(|ui| {
        theme::section(ui, s().filter_env, |ui| envelope(ui, &mut p.filter_env));
        theme::section(ui, s().amp_env, |ui| envelope(ui, &mut p.amp_env));
        theme::section(ui, s().output, |ui| {
            ui.horizontal(|ui| {
                Knob::new(s().volume, &mut p.volume, 0.0, 1.0).default_value(0.7).format(percent).show(ui);
                widgets::vu_meter(ui, m.vu, m.peak, Vec2::new(136.0, 92.0));
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
    ui.horizontal_top(|ui| {
        theme::section(ui, s().lfo, |ui| {
            ui.horizontal(|ui| {
                knob::lfo_wave_selector(ui, &mut p.lfo.wave);
                ui.add_space(6.0);
                let th = theme::t();
                widgets::led(ui, th.accent.gamma_multiply(0.12 + 0.88 * (0.5 + 0.5 * lfo_now))).on_hover_text(s().lfo_led);
            });
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let l = &mut p.lfo;
                Knob::new(s().rate, &mut l.rate, 0.05, 20.0).log().default_value(5.0).format(rate).show(ui);
                Knob::new(s().vibrato, &mut l.pitch, 0.0, 12.0).default_value(0.0).format(semitones).show(ui);
                Knob::new(s().filter_knob, &mut l.cutoff, -1.0, 1.0).bipolar().default_value(0.0).format(signed_percent).show(ui);
                Knob::new(s().tremolo, &mut l.amp, 0.0, 1.0).default_value(0.0).format(percent).show(ui);
            });
        });
        theme::section(ui, s().wheel_section, |ui| {
            ui.horizontal(|ui| {
                widgets::mini_bar(ui, s().wheel, wheel);
                ui.add_space(8.0);
                widgets::mini_bar(ui, s().aftertouch, aftertouch);
            });
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                Knob::new(s().vibrato, &mut p.wheel.vibrato, 0.0, 2.0).default_value(0.5).format(semitones).show(ui);
                Knob::new(s().filter_knob, &mut p.wheel.cutoff, -1.0, 1.0).bipolar().default_value(0.0).format(signed_percent).show(ui);
                Knob::new(s().bend, &mut p.bend_range, 0.0, 12.0).integer().default_value(2.0).format(bend).show(ui);
            });
        });
        theme::section(ui, s().mod_matrix, |ui| mod_matrix(ui, &mut p.mods));
        theme::section(ui, s().play, |ui| {
            knob::voice_mode_selector(ui, &mut p.play.mode);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                Knob::new(s().glide, &mut p.play.glide, 0.0, 2.0).default_value(0.0).format(seconds).show(ui);
                Knob::new(s().velocity, &mut p.play.velocity, 0.0, 1.0).default_value(1.0).format(percent).show(ui);
                Knob::new(s().noise, &mut p.noise, 0.0, 1.0).default_value(0.0).format(percent).show(ui);
            });
        });
    });
    ui.horizontal_top(|ui| {
        let fx = &mut p.fx;
        theme::section(ui, s().chorus, |ui| {
            widgets::toggle(ui, &mut fx.chorus.on, s().on);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let c = &mut fx.chorus;
                Knob::new(s().rate, &mut c.rate, 0.1, 5.0).log().default_value(0.6).format(rate).show(ui);
                Knob::new(s().depth, &mut c.depth, 0.0, 1.0).default_value(0.5).format(percent).show(ui);
                Knob::new(s().mix, &mut c.mix, 0.0, 1.0).default_value(0.7).format(percent).show(ui);
            });
        });
        theme::section(ui, s().delay, |ui| {
            ui.horizontal(|ui| {
                widgets::toggle(ui, &mut fx.delay.on, s().on);
                ui.add_space(12.0);
                widgets::toggle(ui, &mut fx.delay.ping_pong, s().ping_pong);
            });
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let d = &mut fx.delay;
                Knob::new(s().time, &mut d.time_ms, 20.0, 2000.0).log().default_value(375.0).format(millis).show(ui);
                Knob::new(s().feedback, &mut d.feedback, 0.0, 0.95).default_value(0.35).format(percent).show(ui);
                Knob::new(s().tone, &mut d.tone, 500.0, 16_000.0).log().default_value(5000.0).format(hertz).show(ui);
                Knob::new(s().mix, &mut d.mix, 0.0, 1.0).default_value(0.3).format(percent).show(ui);
            });
        });
        theme::section(ui, s().reverb, |ui| {
            widgets::toggle(ui, &mut fx.reverb.on, s().on);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let r = &mut fx.reverb;
                Knob::new(s().size, &mut r.size, 0.0, 1.0).default_value(0.5).format(percent).show(ui);
                Knob::new(s().reverb_decay, &mut r.decay, 0.3, 10.0).log().default_value(1.8).format(seconds).show(ui);
                Knob::new(s().damping, &mut r.damping, 0.0, 1.0).default_value(0.4).format(percent).show(ui);
                Knob::new(s().predelay, &mut r.predelay_ms, 0.0, 100.0).default_value(10.0).format(millis).show(ui);
                Knob::new(s().mix, &mut r.mix, 0.0, 1.0).default_value(0.3).format(percent).show(ui);
            });
        });
        theme::section(ui, s().arp, |ui| arpeggiator(ui, &mut p.arp));
    });
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
        if Knob::new(s().octave, &mut octave, -3.0, 3.0).integer().default_value(0.0).bipolar().format(signed_int).show(ui) {
            osc.octave = octave as i8;
        }
        if semitones {
            let mut semi = osc.semitones as f32;
            if Knob::new(s().semitone, &mut semi, -12.0, 12.0).integer().default_value(0.0).bipolar().format(signed_int).show(ui) {
                osc.semitones = semi as i8;
            }
        }
        Knob::new(s().fine, &mut osc.fine, -50.0, 50.0).default_value(0.0).bipolar().format(cents).show(ui);
        Knob::new(s().level, &mut osc.level, 0.0, 1.0).format(percent).show(ui);
        if osc.wave == Waveform::Wavetable {
            Knob::new(s().position, &mut osc.position, 0.0, 1.0).default_value(0.0).format(percent).show(ui);
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
        if Knob::new(s().octaves, &mut octaves, 1.0, 4.0).integer().default_value(1.0).format(count).show(ui) {
            a.octaves = octaves as u8;
        }
        Knob::new(s().gate, &mut a.gate, 0.05, 1.0).default_value(0.5).format(percent).show(ui);
        Knob::new(s().tempo, &mut a.bpm, 40.0, 240.0).default_value(120.0).format(bpm).show(ui);
    });
}

fn bpm(v: f32) -> String {
    format!("{v:.0} BPM")
}

/// Four ADSR knobs and the shape they make.
pub fn envelope(ui: &mut egui::Ui, env: &mut AdsrParams) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        Knob::new(s().attack, &mut env.attack, 0.001, 5.0).log().format(seconds).show(ui);
        Knob::new(s().decay, &mut env.decay, 0.005, 5.0).log().format(seconds).show(ui);
        Knob::new(s().sustain, &mut env.sustain, 0.0, 1.0).format(percent).show(ui);
        Knob::new(s().release, &mut env.release, 0.005, 8.0).log().format(seconds).show(ui);
        ui.add_space(8.0);
        widgets::envelope_graph(ui, env.attack, env.decay, env.sustain, env.release, Vec2::new(118.0, 92.0));
    });
}
