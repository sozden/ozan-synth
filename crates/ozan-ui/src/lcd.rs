//! Small backlit displays: the two-line screen in the header, which shows the
//! preset and, for a moment, the control last moved; plus the glass and the bar
//! graph that the other screens of the panel share.

use std::cell::RefCell;

use egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};

use ozan_core::{Patch, VoiceMode};

use crate::i18n::s;
use crate::preset_bar::PresetBar;
use crate::theme;

/// Top line of the header screen: preset number and name, `*` once edited.
pub fn preset_line(bar: &PresetBar, patch: &Patch) -> String {
    let edited = if bar.modified(patch) { "*" } else { "" };
    match bar.current().and_then(|i| bar.name(i).map(|n| (i, n))) {
        Some((i, name)) => format!("{:02} {}{edited}", i + 1, name.to_uppercase()),
        None => format!("-- {}", s().lcd_modified.to_uppercase()),
    }
}

/// Lower line while no control is being moved: voice mode, voices in use, rate.
pub fn idle_line(patch: &Patch, voices: Option<u32>, sample_rate: Option<f32>) -> String {
    let mode = match patch.play.mode {
        VoiceMode::Poly => s().poly,
        VoiceMode::Mono => s().mono,
        VoiceMode::Legato => s().legato,
    };
    let mut line = mode.to_uppercase();
    if patch.arp.on {
        line.push_str(" · ARP");
    }
    if let Some(v) = voices {
        line.push_str(&format!(" · {v}/{} {}", ozan_core::MAX_VOICES, s().lcd_voices));
    }
    if let Some(sr) = sample_rate {
        line.push_str(&format!(" · {:.1} kHz", sr / 1000.0));
    }
    line
}

/// The control last moved.
struct Touch {
    section: String,
    label: String,
    value: String,
    norm: f32,
    bipolar: bool,
    time: f64,
}

#[derive(Default)]
struct State {
    /// Title of the section being drawn.
    section: String,
    last: Option<Touch>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// How long a moved control stays on the screen, seconds.
const HOLD: f64 = 2.5;

/// Sets the section title for the controls drawn next; returns the one before.
pub(crate) fn set_section(title: &str) -> String {
    STATE.with_borrow_mut(|s| std::mem::replace(&mut s.section, title.to_string()))
}

pub(crate) fn set_section_back(title: String) {
    STATE.with_borrow_mut(|s| s.section = title);
}

/// A control in the current section was moved to `value` (`norm` 0..1 of its travel).
pub(crate) fn touch(label: &str, value: String, norm: f32, bipolar: bool, time: f64) {
    STATE.with_borrow_mut(|s| {
        let section = s.section.clone();
        s.last = Some(Touch { section, label: label.to_string(), value, norm, bipolar, time });
    });
}

/// Dark glass with a bezel and faint pixel rows.
pub fn glass(painter: &egui::Painter, rect: Rect) {
    let th = theme::t();
    painter.rect_filled(rect, 4.0, th.lcd_bg);
    let mut y = rect.top() + 2.5;
    while y < rect.bottom() - 2.0 {
        painter.line_segment([Pos2::new(rect.left() + 2.0, y), Pos2::new(rect.right() - 2.0, y)], Stroke::new(1.0, Color32::from_black_alpha(45)));
        y += 3.0;
    }
    painter.rect_stroke(rect, 4.0, Stroke::new(1.5, Color32::from_black_alpha(170)), egui::StrokeKind::Inside);
    painter.rect_stroke(rect.expand(1.0), 5.0, Stroke::new(1.0, th.track), egui::StrokeKind::Outside);
}

/// Segmented bar graph; `norm` 0..1, from the middle when `bipolar`.
pub fn bar_graph(painter: &egui::Painter, rect: Rect, norm: f32, bipolar: bool, ink: Color32) {
    let segments = ((rect.width() / 5.0) as usize).max(4);
    let w = rect.width() / segments as f32;
    let (from, to) = if bipolar { (0.5f32.min(norm), 0.5f32.max(norm)) } else { (0.0, norm) };
    for i in 0..segments {
        let mid = (i as f32 + 0.5) / segments as f32;
        let lit = mid >= from && mid <= to && (to - from) > 0.004;
        let r = Rect::from_min_size(Pos2::new(rect.left() + i as f32 * w, rect.top()), Vec2::new(w - 1.0, rect.height()));
        painter.rect_filled(r, 0.5, if lit { ink } else { ink.gamma_multiply(0.12) });
    }
    if bipolar {
        let x = rect.center().x;
        painter.line_segment([Pos2::new(x, rect.top() - 2.0), Pos2::new(x, rect.bottom() + 2.0)], Stroke::new(1.0, ink.gamma_multiply(0.5)));
    }
}

/// The header screen: `title` on top; below, the control last moved or `idle`.
pub fn show(ui: &mut egui::Ui, size: Vec2, title: &str, idle: &str) {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    show_at(ui, rect, title, idle);
}

/// As [`show`], drawn into `rect` without taking space in the layout.
pub fn show_at(ui: &egui::Ui, rect: Rect, title: &str, idle: &str) {
    let th = theme::t();
    let ink = th.lcd_ink;
    let painter = ui.painter_at(rect.expand(2.0));
    glass(&painter, rect);
    let pad = 10.0;
    let now = ui.input(|i| i.time);
    let moved = STATE.with_borrow(|s| {
        s.last.as_ref().filter(|t| now - t.time < HOLD).map(|t| {
            let name = if t.section.is_empty() { t.label.clone() } else { format!("{} › {}", t.section, t.label) };
            (name.to_uppercase(), t.value.clone(), t.norm, t.bipolar)
        })
    });
    let title: String = title.chars().take(24).collect();
    painter.text(Pos2::new(rect.left() + pad, rect.top() + 7.0), Align2::LEFT_TOP, title, FontId::monospace(14.0), ink);
    let low = Pos2::new(rect.left() + pad, rect.bottom() - 7.0);
    match moved {
        Some((name, value, norm, bipolar)) => {
            painter.text(low, Align2::LEFT_BOTTOM, name, FontId::monospace(11.0), ink.gamma_multiply(0.8));
            painter.text(Pos2::new(rect.right() - pad, low.y), Align2::RIGHT_BOTTOM, value, FontId::monospace(11.0), ink);
            let bar = Rect::from_min_size(Pos2::new(rect.right() - pad - 110.0, rect.top() + 10.0), Vec2::new(110.0, 9.0));
            bar_graph(&painter, bar, norm, bipolar, ink);
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
        }
        None => {
            painter.text(low, Align2::LEFT_BOTTOM, idle, FontId::monospace(11.0), ink.gamma_multiply(0.65));
        }
    }
}
