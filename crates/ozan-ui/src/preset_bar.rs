//! The preset selector with ◀ ▶, save and delete, shared by the program and the
//! plugin. Factory presets come first, then the user's own files.

use egui::{self, Key, RichText};
use ozan_core::Patch;

use crate::i18n::{fill, s};
use crate::presets::{self, Preset};
use crate::theme;

#[derive(Default)]
pub struct PresetBar {
    list: Vec<Preset>,
    /// Preset the sound came from (it may have been changed since).
    current: Option<usize>,
    /// Name being typed while the save window is open.
    save_name: Option<String>,
    confirm_delete: bool,
    note: Option<String>,
}

impl PresetBar {
    /// Reads the presets; `select` marks one as current by name.
    pub fn new(select: Option<&str>) -> Self {
        let mut bar = Self::default();
        bar.reload(select);
        bar
    }

    pub fn reload(&mut self, select: Option<&str>) {
        let (list, problems) = presets::load_all();
        self.list = list;
        self.note = problems.first().map(|p| fill(s().unreadable_preset, &[p]));
        self.current = select.and_then(|name| self.list.iter().position(|p| p.name == name));
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    /// Factory presets are the first ones.
    pub fn factory_count(&self) -> usize {
        self.list.iter().filter(|p| p.file.is_none()).count()
    }

    pub fn current(&self) -> Option<usize> {
        self.current
    }

    /// Marks a preset as current without loading it (e.g. the host chose it).
    pub fn set_current(&mut self, index: Option<usize>) {
        self.current = index.filter(|&i| i < self.list.len());
    }

    pub fn name(&self, index: usize) -> Option<&str> {
        self.list.get(index).map(|p| p.name.as_str())
    }

    /// Loads preset `index` into `patch`.
    pub fn load(&mut self, index: usize, patch: &mut Patch) -> bool {
        let Some(preset) = self.list.get(index) else { return false };
        *patch = preset.patch;
        self.current = Some(index);
        true
    }

    /// The sound differs from the preset it was loaded from.
    pub fn modified(&self, patch: &Patch) -> bool {
        self.current.is_none_or(|i| self.list[i].patch != *patch)
    }

    fn step(&self, step: isize) -> Option<usize> {
        let n = self.list.len() as isize;
        (n > 0).then(|| self.current.map_or(0, |c| (c as isize + step).rem_euclid(n)) as usize)
    }

    /// The selector row. Returns the preset loaded into `patch`, if one was.
    pub fn bar(&mut self, ui: &mut egui::Ui, patch: &mut Patch) -> Option<usize> {
        let th = theme::t();
        let mut chosen = None;
        if ui.button("◀").on_hover_text(s().prev_preset).clicked() {
            chosen = self.step(-1);
        }
        let shown = match self.current {
            Some(i) => format!("{:02}  {}{}", i + 1, self.list[i].name, if self.modified(patch) { " *" } else { "" }),
            None => s().unsaved.to_string(),
        };
        egui::ComboBox::from_id_salt("preset")
            .selected_text(RichText::new(shown).monospace().color(th.accent))
            .width(230.0)
            .height(520.0)
            .show_ui(ui, |ui| {
                let factory = self.factory_count();
                for (i, preset) in self.list.iter().enumerate() {
                    if i == factory {
                        ui.separator();
                        ui.label(RichText::new(s().your_presets).size(12.0).color(th.muted));
                    }
                    if ui.selectable_label(self.current == Some(i), format!("{:02}  {}", i + 1, preset.name)).clicked() {
                        chosen = Some(i);
                    }
                }
            });
        if ui.button("▶").on_hover_text(s().next_preset).clicked() {
            chosen = self.step(1);
        }
        if ui.button(s().save).on_hover_text(s().save_hint).clicked() {
            let own = self.current.filter(|&i| self.list[i].file.is_some()).map(|i| self.list[i].name.clone());
            self.save_name = Some(own.unwrap_or_default());
        }
        let deletable = self.current.is_some_and(|i| self.list[i].file.is_some());
        if ui.add_enabled(deletable, egui::Button::new(s().delete)).on_disabled_hover_text(s().factory_locked).clicked() {
            self.confirm_delete = true;
        }
        if let Some(note) = &self.note {
            ui.label(RichText::new(note).size(12.0).color(th.warm));
        }
        chosen.filter(|&i| self.load(i, patch))
    }

    /// Save and delete windows. Returns the name a preset was saved under.
    pub fn dialogs(&mut self, ctx: &egui::Context, patch: &Patch) -> Option<String> {
        let mut saved = None;
        if let Some(mut name) = self.save_name.take() {
            let mut close = false;
            let mut save = false;
            let modal = egui::Modal::new(egui::Id::new("save_preset")).show(ctx, |ui| {
                ui.set_width(320.0);
                ui.heading(s().save_title);
                ui.add_space(6.0);
                let field = ui.add(egui::TextEdit::singleline(&mut name).hint_text(s().preset_name).desired_width(f32::INFINITY));
                field.request_focus();
                let trimmed = name.trim();
                if !trimmed.is_empty() && presets::path_for(trimmed).is_some_and(|p| p.exists()) {
                    ui.label(RichText::new(s().will_overwrite).size(12.0).color(theme::t().warm));
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let ok = !trimmed.is_empty();
                    if ui.add_enabled(ok, egui::Button::new(s().save)).clicked() || (ok && ui.input(|i| i.key_pressed(Key::Enter))) {
                        save = true;
                    }
                    if ui.button(s().cancel).clicked() {
                        close = true;
                    }
                });
            });
            if save {
                let name = name.trim().to_string();
                match presets::save(&name, patch) {
                    Ok(_) => {
                        self.reload(Some(&name));
                        saved = Some(name);
                    }
                    Err(e) => self.note = Some(fill(s().save_failed, &[&e])),
                }
            } else if !close && !modal.should_close() {
                self.save_name = Some(name);
            }
        }
        if self.confirm_delete {
            let Some(i) = self.current.filter(|&i| self.list[i].file.is_some()) else {
                self.confirm_delete = false;
                return saved;
            };
            let name = self.list[i].name.clone();
            let mut answer = None;
            let modal = egui::Modal::new(egui::Id::new("delete_preset")).show(ctx, |ui| {
                ui.set_width(320.0);
                ui.heading(s().delete_title);
                ui.add_space(6.0);
                ui.label(fill(s().delete_body, &[&name]));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button(s().delete).clicked() {
                        answer = Some(true);
                    }
                    if ui.button(s().cancel).clicked() {
                        answer = Some(false);
                    }
                });
            });
            if answer == Some(true) {
                if let Some(file) = self.list[i].file.clone() {
                    match presets::delete(&file) {
                        // The sound stays as it is; it just has no preset now.
                        Ok(()) => self.reload(None),
                        Err(e) => self.note = Some(fill(s().delete_failed, &[&e])),
                    }
                }
            }
            if answer.is_some() || modal.should_close() {
                self.confirm_delete = false;
            }
        }
        saved
    }
}
