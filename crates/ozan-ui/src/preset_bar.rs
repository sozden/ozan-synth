//! The preset selector with ◀ ▶, save and delete, shared by the program and the
//! plugin. Factory presets come first, then the user's own files.

use std::collections::BTreeSet;

use egui::{self, Align2, FontId, Key, Pos2, RichText, Sense, Stroke, Vec2};
use ozan_core::Patch;

use crate::i18n::{fill, s};
use crate::presets::{self, Category, Preset};
use crate::settings::Settings;
use crate::theme;

/// Settings key of the favorites: `f:` or `u:` (factory or user) and the name,
/// tab separated.
const FAVORITES: &str = "favorites";

/// Which presets the browser lists.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Filter {
    #[default]
    All,
    Favorites,
    Category(Category),
    Yours,
}

#[derive(Default)]
struct Browser {
    open: bool,
    filter: Filter,
    query: String,
    /// The preset field: the browser opens under it, and clicks on it are not "outside".
    field: Option<egui::Rect>,
    /// Pass in which a click outside closed the browser.
    closed_in: Option<u64>,
    /// Row to bring into view.
    scroll_to: Option<usize>,
}

#[derive(Default)]
pub struct PresetBar {
    list: Vec<Preset>,
    /// Preset the sound came from (it may have been changed since).
    current: Option<usize>,
    /// Name being typed while the save window is open.
    save_name: Option<String>,
    confirm_delete: bool,
    note: Option<String>,
    browser: Browser,
    favorites: BTreeSet<String>,
    /// Category chosen in the save window.
    save_category: Option<Category>,
}

/// A five-pointed star, filled or outlined.
fn paint_star(painter: &egui::Painter, center: Pos2, r: f32, filled: bool, color: egui::Color32) {
    let points: Vec<Pos2> = (0..10)
        .map(|k| {
            let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * k as f32 / 5.0;
            let radius = if k % 2 == 0 { r } else { r * 0.45 };
            center + Vec2::new(a.cos(), a.sin()) * radius
        })
        .collect();
    if filled {
        // egui fills only convex shapes: the inner pentagon, then the five points.
        let inner: Vec<Pos2> = (0..5).map(|k| points[2 * k + 1]).collect();
        painter.add(egui::Shape::convex_polygon(inner, color, Stroke::NONE));
        for k in 0..5 {
            let tri = vec![points[(2 * k + 9) % 10], points[2 * k], points[2 * k + 1]];
            painter.add(egui::Shape::convex_polygon(tri, color, Stroke::NONE));
        }
    } else {
        painter.add(egui::Shape::closed_line(points, Stroke::new(1.2, color)));
    }
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
        let settings = Settings::load();
        self.favorites = settings.get(FAVORITES).map(|v| v.split('\t').filter(|f| !f.is_empty()).map(String::from).collect()).unwrap_or_default();
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
        // The preset field opens the browser.
        let (rect, field) = ui.allocate_exact_size(Vec2::new(250.0, 22.0), Sense::click());
        let open = self.browser.open;
        let widgets = &ui.visuals().widgets;
        let fill = if open { th.accent_dark } else if field.hovered() { widgets.hovered.weak_bg_fill } else { widgets.inactive.weak_bg_fill };
        ui.painter().rect_filled(rect, 4.0, fill);
        let text: String = shown.chars().take(28).collect();
        ui.painter().text(rect.left_center() + Vec2::new(8.0, 0.0), Align2::LEFT_CENTER, text, FontId::monospace(13.5), if open { th.text } else { th.accent });
        let c = rect.right_center() - Vec2::new(11.0, 0.0);
        let tip = if open { -1.0 } else { 1.0 };
        ui.painter().add(egui::Shape::convex_polygon(
            vec![c + Vec2::new(-4.5, -2.0 * tip), c + Vec2::new(4.5, -2.0 * tip), c + Vec2::new(0.0, 3.0 * tip)],
            th.muted,
            Stroke::NONE,
        ));
        self.browser.field = Some(rect);
        if field.on_hover_text(s().browse_hint).clicked() {
            self.toggle_browser(ui.ctx());
        }
        if ui.button("▶").on_hover_text(s().next_preset).clicked() {
            chosen = self.step(1);
        }
        if ui.button(s().save).on_hover_text(s().save_hint).clicked() {
            let own = self.current.filter(|&i| self.list[i].file.is_some()).map(|i| self.list[i].name.clone());
            self.save_name = Some(own.unwrap_or_default());
            self.save_category = self.current.and_then(|i| self.list[i].category);
        }
        let deletable = self.current.is_some_and(|i| self.list[i].file.is_some());
        if ui.add_enabled(deletable, egui::Button::new(s().delete)).on_disabled_hover_text(s().factory_locked).clicked() {
            self.confirm_delete = true;
        }
        if let Some(note) = &self.note {
            ui.label(RichText::new(note).size(12.0).color(th.warm));
        }
        if self.browser.open {
            if let Some(i) = self.browser_panel(ui.ctx()) {
                chosen = Some(i);
            }
        }
        chosen.filter(|&i| self.load(i, patch))
    }

    /// Opens or closes the browser (the header screen calls this when clicked).
    pub fn toggle_browser(&mut self, ctx: &egui::Context) {
        if self.browser.open {
            self.browser.open = false;
        } else if self.browser.closed_in != Some(ctx.cumulative_pass_nr()) {
            // The same click that closed it from outside does not open it again.
            self.browser.open = true;
            self.browser.scroll_to = self.current;
        }
    }

    fn favorite_key(&self, i: usize) -> String {
        let p = &self.list[i];
        format!("{}:{}", if p.file.is_some() { "u" } else { "f" }, p.name)
    }

    fn toggle_favorite(&mut self, i: usize) {
        let key = self.favorite_key(i);
        if !self.favorites.remove(&key) {
            self.favorites.insert(key);
        }
        let joined: Vec<&str> = self.favorites.iter().map(String::as_str).collect();
        Settings::load().set(FAVORITES, &joined.join("\t"));
    }

    fn filter_name(f: Filter) -> &'static str {
        match f {
            Filter::All => s().cat_all,
            Filter::Favorites => s().cat_fav,
            Filter::Category(c) => c.name(),
            Filter::Yours => s().your_presets,
        }
    }

    fn matches(&self, i: usize, f: Filter) -> bool {
        match f {
            Filter::All => true,
            Filter::Favorites => self.favorites.contains(&self.favorite_key(i)),
            Filter::Category(c) => self.list[i].category == Some(c),
            Filter::Yours => self.list[i].file.is_some(),
        }
    }

    /// Presets the browser lists now: the chosen group, narrowed by the search.
    fn visible(&self) -> Vec<usize> {
        let q = self.browser.query.trim().to_lowercase();
        (0..self.list.len())
            .filter(|&i| self.matches(i, self.browser.filter))
            .filter(|&i| {
                let p = &self.list[i];
                q.is_empty() || p.name.to_lowercase().contains(&q) || p.category.is_some_and(|c| c.name().to_lowercase().contains(&q))
            })
            .collect()
    }

    /// The browser under the preset field. Returns the preset to load.
    fn browser_panel(&mut self, ctx: &egui::Context) -> Option<usize> {
        let th = theme::t();
        let mut chosen = None;
        let mut star = None;
        // Arrows walk the list and load as they go; Enter and Esc close.
        let (up, down, enter, esc) = ctx.input(|i| (i.key_pressed(Key::ArrowUp), i.key_pressed(Key::ArrowDown), i.key_pressed(Key::Enter), i.key_pressed(Key::Escape)));
        let shown = self.visible();
        if up || down {
            let at = self.current.and_then(|c| shown.iter().position(|&v| v == c));
            let next = match at {
                None => shown.first().copied(),
                Some(p) if down => shown.get(p + 1).copied(),
                Some(p) => p.checked_sub(1).map(|p| shown[p]),
            };
            if let Some(n) = next {
                chosen = Some(n);
                self.browser.scroll_to = Some(n);
            }
        }
        if enter {
            // From the search box, Enter takes the first match.
            if self.current.is_none_or(|c| !shown.contains(&c)) {
                chosen = shown.first().copied();
            }
            self.browser.open = false;
        }
        if esc {
            self.browser.open = false;
        }

        let area = egui::Area::new(egui::Id::new("preset_browser"))
            .order(egui::Order::Foreground)
            .fixed_pos(self.browser.field.map_or(Pos2::ZERO, |f| f.left_bottom()) + Vec2::new(0.0, 6.0))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(th.bg)
                    .stroke(Stroke::new(1.0, th.track))
                    .corner_radius(8.0)
                    .inner_margin(10.0)
                    .shadow(ctx.style().visuals.popup_shadow)
                    .show(ui, |ui| {
                        ui.horizontal_top(|ui| {
                            // Groups, with how many presets each holds.
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 2.0;
                                let mut filters = vec![Filter::All, Filter::Favorites];
                                filters.extend(Category::ALL.map(Filter::Category));
                                filters.push(Filter::Yours);
                                for f in filters {
                                    let n = (0..self.list.len()).filter(|&i| self.matches(i, f)).count();
                                    let (rect, r) = ui.allocate_exact_size(Vec2::new(160.0, 26.0), Sense::click());
                                    let on = self.browser.filter == f;
                                    if on || r.hovered() {
                                        ui.painter().rect_filled(rect, 4.0, if on { th.accent_dark } else { th.knob_hover });
                                    }
                                    let color = if on { th.text } else { th.muted };
                                    ui.painter().text(rect.left_center() + Vec2::new(10.0, 0.0), Align2::LEFT_CENTER, Self::filter_name(f), FontId::proportional(14.0), color);
                                    ui.painter().text(rect.right_center() - Vec2::new(10.0, 0.0), Align2::RIGHT_CENTER, n.to_string(), FontId::monospace(12.5), color);
                                    if r.clicked() {
                                        self.browser.filter = f;
                                        self.browser.scroll_to = self.current;
                                    }
                                }
                            });
                            ui.add_space(10.0);
                            ui.vertical(|ui| {
                                ui.set_width(400.0);
                                ui.add(egui::TextEdit::singleline(&mut self.browser.query).hint_text(s().search_presets).desired_width(f32::INFINITY));
                                ui.add_space(4.0);
                                let shown = self.visible();
                                egui::ScrollArea::vertical().max_height(360.0).auto_shrink([false, false]).show(ui, |ui| {
                                    ui.spacing_mut().item_spacing.y = 1.0;
                                    if shown.is_empty() {
                                        ui.label(RichText::new(s().no_match).color(th.muted));
                                    }
                                    for &i in &shown {
                                        let (rect, r) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::click());
                                        let current = self.current == Some(i);
                                        let painter = ui.painter_at(rect);
                                        if current {
                                            painter.rect_filled(rect, 4.0, th.knob);
                                            painter.rect_stroke(rect.shrink(0.5), 4.0, Stroke::new(1.0, th.accent), egui::StrokeKind::Inside);
                                        } else if r.hovered() {
                                            painter.rect_filled(rect, 4.0, th.knob_hover);
                                        }
                                        let fav = self.favorites.contains(&self.favorite_key(i));
                                        let star_at = rect.left_center() + Vec2::new(14.0, 0.0);
                                        paint_star(&painter, star_at, 7.0, fav, if fav { th.accent } else { th.muted });
                                        painter.text(rect.left_center() + Vec2::new(32.0, 0.0), Align2::LEFT_CENTER, format!("{:02}", i + 1), FontId::monospace(12.5), th.muted);
                                        painter.text(rect.left_center() + Vec2::new(62.0, 0.0), Align2::LEFT_CENTER, &self.list[i].name, FontId::proportional(14.5), if current { th.accent } else { th.text });
                                        if let Some(c) = self.list[i].category {
                                            painter.text(rect.right_center() - Vec2::new(10.0, 0.0), Align2::RIGHT_CENTER, c.name(), FontId::proportional(12.5), th.muted);
                                        }
                                        if r.clicked() {
                                            let on_star = r.interact_pointer_pos().is_some_and(|p| p.x < rect.left() + 28.0);
                                            if on_star {
                                                star = Some(i);
                                            } else {
                                                chosen = Some(i);
                                            }
                                        }
                                        if self.browser.scroll_to == Some(i) {
                                            r.scroll_to_me(None);
                                        }
                                    }
                                });
                                ui.add_space(4.0);
                                ui.label(RichText::new(s().browser_hint).size(12.0).color(th.muted));
                            });
                        });
                    });
            });
        self.browser.scroll_to = None;
        if let Some(i) = star {
            self.toggle_favorite(i);
        }
        // A click anywhere else closes it.
        let outside = ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !area.response.rect.contains(p) && !self.browser.field.is_some_and(|f| f.contains(p))));
        if outside {
            self.browser.open = false;
            self.browser.closed_in = Some(ctx.cumulative_pass_nr());
        }
        chosen
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
                ui.horizontal(|ui| {
                    ui.label(s().category);
                    let shown = self.save_category.map_or("—", Category::name);
                    egui::ComboBox::from_id_salt("save_category").selected_text(shown).width(150.0).show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.save_category, None, "—");
                        for c in Category::ALL {
                            ui.selectable_value(&mut self.save_category, Some(c), c.name());
                        }
                    });
                });
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
                match presets::save(&name, patch, self.save_category) {
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
