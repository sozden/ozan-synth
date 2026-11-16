//! Undo and redo for the sound. Each finished change is one step: a knob let go,
//! a click, a preset loaded, a throw of the dice. Changes without the mouse (the
//! mouse wheel, MIDI controllers) count as finished after a short pause.

use std::time::Duration;

use egui::{self, Key, Modifiers, Pos2, Sense, Stroke, Vec2};
use ozan_core::Patch;

use crate::i18n::s;
use crate::theme;

/// Steps kept.
const STEPS: usize = 100;
/// Seconds without a change after which a change counts as finished.
const SETTLE: f64 = 0.4;

#[derive(Default)]
pub struct History {
    past: Vec<Patch>,
    future: Vec<Patch>,
    /// The sound as of the last finished change.
    committed: Option<Patch>,
    /// The sound in the previous frame, and when it last changed.
    seen: Option<Patch>,
    changed_at: f64,
}

impl History {
    /// Call once per frame, after everything that may change the sound.
    pub fn track(&mut self, ctx: &egui::Context, patch: &Patch) {
        let now = ctx.input(|i| i.time);
        let Some(committed) = self.committed else {
            self.committed = Some(*patch);
            self.seen = Some(*patch);
            return;
        };
        if self.seen != Some(*patch) {
            self.seen = Some(*patch);
            self.changed_at = now;
        }
        if *patch == committed {
            return;
        }
        let busy = ctx.input(|i| i.pointer.any_down());
        if !busy && now - self.changed_at >= SETTLE {
            self.commit(*patch);
        } else {
            ctx.request_repaint_after(Duration::from_secs_f64(SETTLE));
        }
    }

    fn commit(&mut self, patch: Patch) {
        if let Some(old) = self.committed.replace(patch) {
            if old != patch {
                self.past.push(old);
                if self.past.len() > STEPS {
                    self.past.remove(0);
                }
                self.future.clear();
            }
        }
        self.seen = Some(patch);
    }

    pub fn can_undo(&self, patch: &Patch) -> bool {
        !self.past.is_empty() || self.committed.is_some_and(|c| c != *patch)
    }

    pub fn can_redo(&self, patch: &Patch) -> bool {
        !self.future.is_empty() && self.committed == Some(*patch)
    }

    /// Takes back the last change (an unfinished one first).
    pub fn undo(&mut self, patch: &mut Patch) -> bool {
        if self.committed != Some(*patch) {
            self.commit(*patch);
        }
        let Some(before) = self.past.pop() else { return false };
        self.future.push(*patch);
        *patch = before;
        self.committed = Some(before);
        self.seen = Some(before);
        true
    }

    /// Puts back what undo took back. A new change since then clears it.
    pub fn redo(&mut self, patch: &mut Patch) -> bool {
        if self.committed != Some(*patch) {
            self.commit(*patch);
        }
        let Some(after) = self.future.pop() else { return false };
        self.past.push(*patch);
        *patch = after;
        self.committed = Some(after);
        self.seen = Some(after);
        true
    }

    fn report(&self, undone: bool, time: f64) {
        let label = if undone { s().undo } else { s().redo };
        let value = format!("{} / {}", self.past.len(), self.past.len() + self.future.len());
        crate::lcd::touch(label, value, 1.0, false, time);
    }

    /// Ctrl+Z undoes; Ctrl+Y and Ctrl+Shift+Z redo. Call before anything else reads
    /// the keyboard. True when `patch` changed.
    pub fn shortcuts(&mut self, ctx: &egui::Context, patch: &mut Patch) -> bool {
        if ctx.wants_keyboard_input() {
            return false;
        }
        let (undo, redo) = ctx.input_mut(|i| {
            let redo = i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z) || i.consume_key(Modifiers::COMMAND, Key::Y);
            let undo = !redo && i.consume_key(Modifiers::COMMAND, Key::Z);
            (undo, redo)
        });
        let time = ctx.input(|i| i.time);
        let changed = (undo && self.undo(patch)) || (redo && self.redo(patch));
        if changed {
            self.report(undo, time);
        }
        changed
    }

    /// The undo and redo buttons. True when `patch` changed.
    pub fn buttons(&mut self, ui: &mut egui::Ui, patch: &mut Patch) -> bool {
        let mut changed = false;
        for undo in [true, false] {
            let able = if undo { self.can_undo(patch) } else { self.can_redo(patch) };
            let response = arrow_button(ui, undo, able).on_hover_text(if undo { s().undo_hint } else { s().redo_hint });
            if response.clicked() && able {
                let done = if undo { self.undo(patch) } else { self.redo(patch) };
                if done {
                    self.report(undo, ui.input(|i| i.time));
                    changed = true;
                }
            }
        }
        changed
    }
}

/// A curved arrow, pointing left (undo) or right (redo).
fn arrow_button(ui: &mut egui::Ui, left: bool, able: bool) -> egui::Response {
    let th = theme::t();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(26.0, 24.0), Sense::click());
    let painter = ui.painter_at(rect);
    if able && response.hovered() {
        painter.rect_filled(rect.shrink(1.0), 4.0, th.knob_hover);
    }
    let color = if able { th.text } else { th.muted.gamma_multiply(0.45) };
    let c = rect.center() + Vec2::new(0.0, 2.0);
    let r = 6.0;
    let side = if left { -1.0 } else { 1.0 };
    // An arc over the top, from the far side round to the arrow's side.
    let points: Vec<Pos2> = (0..=14)
        .map(|k| {
            let a = std::f32::consts::PI * k as f32 / 14.0;
            Pos2::new(c.x - side * r * a.cos(), c.y - r * a.sin())
        })
        .collect();
    let tip = Pos2::new(c.x + side * r, c.y + 3.5);
    painter.add(egui::Shape::line(points, Stroke::new(1.8, color)));
    painter.add(egui::Shape::convex_polygon(
        vec![tip, Pos2::new(c.x + side * r - 3.5, c.y - 0.5), Pos2::new(c.x + side * r + 3.5, c.y - 0.5)],
        color,
        Stroke::NONE,
    ));
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn patch(volume: f32) -> Patch {
        Patch { volume, ..Patch::default() }
    }

    /// One finished change, as `track` would record it.
    fn change(h: &mut History, p: Patch) {
        if h.committed.is_none() {
            h.committed = Some(Patch::default());
        }
        h.commit(p);
    }

    #[test]
    fn undo_and_redo_walk_the_steps() {
        let mut h = History::default();
        change(&mut h, patch(0.1));
        change(&mut h, patch(0.2));
        let mut p = patch(0.2);
        assert!(h.undo(&mut p));
        assert_eq!(p, patch(0.1));
        assert!(h.undo(&mut p));
        assert_eq!(p, Patch::default());
        assert!(!h.undo(&mut p));
        assert!(h.redo(&mut p));
        assert_eq!(p, patch(0.1));
        // A new change drops what could be redone.
        change(&mut h, patch(0.5));
        let mut p = patch(0.5);
        assert!(!h.redo(&mut p));
        assert!(h.undo(&mut p));
        assert_eq!(p, patch(0.1));
    }

    #[test]
    fn an_unfinished_change_is_undone_first() {
        let mut h = History::default();
        change(&mut h, patch(0.1));
        // Still being turned: not committed yet.
        let mut p = patch(0.3);
        assert!(h.can_undo(&p));
        assert!(h.undo(&mut p));
        assert_eq!(p, patch(0.1));
        assert!(h.redo(&mut p));
        assert_eq!(p, patch(0.3));
    }
}
