//! The plugin window: the standalone program's panel under a header with the logo
//! and the preset selector.

use std::sync::Arc;

use nih_plug::prelude::{Editor, ParamSetter};
use nih_plug_egui::create_egui_editor;
use nih_plug_egui::egui::{self, RichText};
use ozan_core::wavetable::WavetableBank;
use ozan_ui::i18n::{self, s, Lang};
use ozan_ui::preset_bar::PresetBar;
use ozan_ui::settings::Settings;
use ozan_ui::theme;

use crate::{Meters, OzanParams};

pub const WIDTH: u32 = 1560;
pub const HEIGHT: u32 = 800;

/// Per-window state: needle ballistics, scope buffer, the preset bar.
#[derive(Default)]
struct Window {
    bar: PresetBar,
    /// Preset parameter value the bar last followed.
    seen_param: Option<i32>,
    vu: f32,
    peak: f32,
    scope: Vec<f32>,
}

pub fn create(params: Arc<OzanParams>, meters: Arc<Meters>, bank: Arc<WavetableBank>) -> Option<Box<dyn Editor>> {
    create_egui_editor(
        params.editor_state.clone(),
        Window::default(),
        |ctx, window| {
            // The user's presets as they are on disk now.
            *window = Window::default();
            window.bar = PresetBar::new(None);
            // Theme and language as chosen in the standalone program.
            let settings = Settings::load();
            i18n::select(settings.get("lang").and_then(Lang::from_code).unwrap_or(Lang::En));
            theme::select(ctx, settings.get("theme").and_then(theme::index_of).unwrap_or(0));
        },
        move |ctx, setter, window| {
            let peak = meters.take_peak();
            window.peak = peak.max(window.peak * 0.85);
            window.vu += (peak - window.vu) * 0.12;

            ozan_ui::wood_cheeks(ctx);
            egui::TopBottomPanel::top("header").frame(ozan_ui::pad()).show(ctx, |ui| header(ui, &params, setter, window));
            if let Ok(patch) = params.patch.read().map(|p| *p) {
                window.bar.dialogs(ctx, &patch);
            }
            egui::CentralPanel::default().frame(ozan_ui::pad()).show(ctx, |ui| {
                egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                    let Ok(current) = params.patch.read().map(|p| *p) else { return };
                    let mut patch = current;
                    meters.scope.snapshot(&mut window.scope);
                    let shown = ozan_ui::panel::Meters {
                        scope: &window.scope,
                        sample_rate: meters.scope.sample_rate(),
                        vu: window.vu,
                        peak: window.peak,
                        voices: Meters::load(&meters.voices) as u32,
                        lfo: Meters::load(&meters.lfo),
                        lfo2: Meters::load(&meters.lfo2),
                        wheel: Meters::load(&meters.wheel),
                        aftertouch: Meters::load(&meters.aftertouch),
                    };
                    ozan_ui::panel::panel(ui, &mut patch, &bank, &shown);
                    if patch != current {
                        if let Ok(mut p) = params.patch.write() {
                            *p = patch;
                        }
                    }
                });
            });
            ctx.request_repaint();
        },
    )
}

fn set_preset(params: &OzanParams, setter: &ParamSetter, index: i32) {
    setter.begin_set_parameter(&params.preset);
    setter.set_parameter(&params.preset, index);
    setter.end_set_parameter(&params.preset);
}

fn header(ui: &mut egui::Ui, params: &OzanParams, setter: &ParamSetter, window: &mut Window) {
    let th = theme::t();
    ui.horizontal(|ui| {
        ozan_ui::logo::lockup(ui, 22.0, th.text, th.accent, th.muted);
        ui.label(RichText::new(format!("v{}", env!("CARGO_PKG_VERSION"))).color(th.muted));
        ui.add_space(16.0);
        // The host (automation, project load) may change the preset parameter.
        let param = params.preset.value();
        if window.seen_param != Some(param) {
            window.seen_param = Some(param);
            if window.bar.current().is_none_or(|c| c < window.bar.factory_count()) {
                window.bar.set_current(Some(param as usize));
            }
        }
        let Ok(mut patch) = params.patch.read().map(|p| *p) else { return };
        if let Some(i) = window.bar.bar(ui, &mut patch) {
            if let Ok(mut p) = params.patch.write() {
                *p = patch;
            }
            // Factory presets go through the parameter, so the host sees the change.
            if i < window.bar.factory_count() {
                set_preset(params, setter, i as i32);
                window.seen_param = Some(i as i32);
            }
        }
        ui.add_space(24.0);
        let idle = ozan_ui::lcd::idle_line(&patch, None, None);
        ozan_ui::lcd::show(ui, egui::vec2(340.0, 46.0), &ozan_ui::lcd::preset_line(&window.bar, &patch), &idle);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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
            if index != theme::current_index() {
                theme::select(ui.ctx(), index);
                Settings::load().set("theme", theme::t().key);
            }
            if lang != i18n::current() {
                i18n::select(lang);
                Settings::load().set("lang", lang.code());
            }
        });
    });
}
