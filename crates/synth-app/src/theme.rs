//! Looks of the panel. A theme is a set of colours plus a few drawing styles; the
//! interface thread reads the current one through [`t`].

use std::sync::atomic::{AtomicUsize, Ordering};

use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum KnobStyle {
    /// Flat cap with a coloured arc showing the value.
    Modern,
    /// Black cap on a wide skirt with a scale of ticks around it, as on 1970s gear.
    Skirted,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SectionStyle {
    /// Rounded card with screws in the corners.
    Card,
    /// A rule across the top with the title in it; the panel shows through.
    Rule,
}

pub struct Theme {
    pub name: &'static str,
    /// Stable key for the settings file.
    pub key: &'static str,
    pub knob_style: KnobStyle,
    pub section_style: SectionStyle,
    /// Wooden end cheeks left and right.
    pub wood: bool,
    pub bg: Color32,
    pub card: Color32,
    pub screen: Color32,
    pub border: Color32,
    pub track: Color32,
    pub knob: Color32,
    pub knob_hover: Color32,
    pub skirt: Color32,
    pub pointer: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub accent_dark: Color32,
    pub warm: Color32,
    pub hot: Color32,
    pub good: Color32,
    pub white_key: Color32,
    pub black_key: Color32,
    pub vu_face: Color32,
    pub vu_ink: Color32,
    pub wood_base: Color32,
    pub wood_grain: Color32,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

pub static THEMES: [Theme; 2] = [
    Theme {
        name: "Stüdyo (70'ler)",
        key: "studio",
        knob_style: KnobStyle::Skirted,
        section_style: SectionStyle::Rule,
        wood: true,
        bg: rgb(0x16, 0x15, 0x14),
        card: rgb(0x16, 0x15, 0x14),
        screen: rgb(0x0b, 0x0b, 0x0a),
        border: rgb(0xe9, 0xdf, 0xc8),
        track: rgb(0x3a, 0x37, 0x33),
        knob: rgb(0x14, 0x14, 0x14),
        knob_hover: rgb(0x22, 0x22, 0x22),
        skirt: rgb(0x2c, 0x2b, 0x29),
        pointer: rgb(0xf4, 0xf1, 0xea),
        text: rgb(0xe9, 0xdf, 0xc8),
        muted: rgb(0xa3, 0x99, 0x85),
        accent: rgb(0xe8, 0x74, 0x2a),
        accent_dark: rgb(0xb8, 0x52, 0x14),
        warm: rgb(0xef, 0xb0, 0x40),
        hot: rgb(0xe2, 0x4b, 0x4a),
        good: rgb(0x5b, 0xd0, 0x6a),
        white_key: rgb(0xf4, 0xf1, 0xea),
        black_key: rgb(0x12, 0x12, 0x12),
        vu_face: rgb(0xf1, 0xe4, 0xbf),
        vu_ink: rgb(0x2a, 0x24, 0x1c),
        wood_base: rgb(0x6b, 0x3f, 0x1f),
        wood_grain: rgb(0x4a, 0x2a, 0x12),
    },
    Theme {
        name: "Gece",
        key: "night",
        knob_style: KnobStyle::Modern,
        section_style: SectionStyle::Card,
        wood: false,
        bg: rgb(0x15, 0x16, 0x1b),
        card: rgb(0x1f, 0x21, 0x29),
        screen: rgb(0x10, 0x11, 0x15),
        border: rgb(0x34, 0x37, 0x44),
        track: rgb(0x34, 0x37, 0x44),
        knob: rgb(0x2c, 0x2f, 0x3a),
        knob_hover: rgb(0x38, 0x3c, 0x4a),
        skirt: rgb(0x2c, 0x2f, 0x3a),
        pointer: rgb(0xe6, 0xe6, 0xec),
        text: rgb(0xe6, 0xe6, 0xec),
        muted: rgb(0x8c, 0x8f, 0x9c),
        accent: rgb(0x8b, 0x7c, 0xf6),
        accent_dark: rgb(0x5b, 0x4c, 0xc6),
        warm: rgb(0xef, 0x9f, 0x27),
        hot: rgb(0xe2, 0x4b, 0x4a),
        good: rgb(0x3d, 0xc9, 0x8a),
        white_key: rgb(0xec, 0xec, 0xf0),
        black_key: rgb(0x1a, 0x1b, 0x20),
        vu_face: rgb(0xe9, 0xe1, 0xc9),
        vu_ink: rgb(0x26, 0x24, 0x22),
        wood_base: rgb(0, 0, 0),
        wood_grain: rgb(0, 0, 0),
    },
];

static CURRENT: AtomicUsize = AtomicUsize::new(0);

/// The theme in use.
pub fn t() -> &'static Theme {
    &THEMES[CURRENT.load(Ordering::Relaxed) % THEMES.len()]
}

pub fn current_index() -> usize {
    CURRENT.load(Ordering::Relaxed)
}

/// Switches theme and restyles egui's own widgets to match.
pub fn select(ctx: &egui::Context, index: usize) {
    CURRENT.store(index % THEMES.len(), Ordering::Relaxed);
    apply(ctx);
}

pub fn apply(ctx: &egui::Context) {
    let th = t();
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = th.bg;
    visuals.window_fill = th.card;
    visuals.extreme_bg_color = th.screen;
    visuals.selection.bg_fill = th.accent_dark;
    visuals.widgets.hovered.bg_stroke.color = th.accent;
    visuals.override_text_color = Some(th.text);
    ctx.set_visuals(visuals);
}

/// Index of the theme saved under `key`.
pub fn index_of(key: &str) -> Option<usize> {
    THEMES.iter().position(|th| th.key == key)
}

/// A titled section of the panel.
pub fn section<R>(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let th = t();
    match th.section_style {
        SectionStyle::Card => {
            let response = egui::Frame::new()
                .fill(th.card)
                .stroke(Stroke::new(1.0, th.border))
                .corner_radius(8.0)
                .inner_margin(egui::Margin::same(12))
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 6.0;
                        ui.label(egui::RichText::new(title).size(12.0).color(th.muted).strong());
                        add(ui)
                    })
                    .inner
                });
            let r = response.response.rect.shrink(6.0);
            for corner in [r.left_top(), r.right_top(), r.left_bottom(), r.right_bottom()] {
                screw(ui.painter(), corner, th);
            }
            response.inner
        }
        SectionStyle::Rule => {
            egui::Frame::new()
                .inner_margin(egui::Margin { left: 4, right: 4, top: 0, bottom: 4 })
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 6.0;
                        let title = spaced(title);
                        let galley = ui.painter().layout_no_wrap(title, egui::FontId::proportional(11.0), th.text);
                        let (rect, _) = ui.allocate_exact_size(Vec2::new(galley.size().x, 16.0), egui::Sense::hover());
                        let text_pos = Pos2::new(rect.left(), rect.center().y - galley.size().y / 2.0);
                        let inner = add(ui);
                        // Rule across the whole section, broken where the title sits.
                        let right = ui.min_rect().right();
                        let y = rect.center().y;
                        let p = ui.painter();
                        p.galley(text_pos, galley, th.text);
                        p.line_segment([Pos2::new(rect.right() + 8.0, y), Pos2::new(right, y)], Stroke::new(1.5, th.border));
                        inner
                    })
                    .inner
                })
                .inner
        }
    }
}

/// Letter-spaced capitals, as printed on old front panels.
fn spaced(s: &str) -> String {
    s.chars().flat_map(|c| [c, '\u{2009}']).collect::<String>().trim_end().to_string()
}

/// A small slotted screw head.
fn screw(painter: &egui::Painter, center: Pos2, th: &Theme) {
    painter.circle_filled(center, 3.2, th.track);
    painter.circle_stroke(center, 3.2, Stroke::new(0.8, th.bg));
    let d = Vec2::new(2.2, -1.2);
    painter.line_segment([center - d, center + d], Stroke::new(1.0, th.bg));
}

/// Wooden end cheek: base colour with wavy grain lines along its length.
pub fn paint_wood(painter: &egui::Painter, rect: Rect) {
    let th = t();
    painter.rect_filled(rect, 0.0, th.wood_base);
    let lines = 9;
    for i in 0..lines {
        let x0 = rect.left() + (i as f32 + 0.5) / lines as f32 * rect.width();
        let phase = i as f32 * 1.7;
        let amp = 1.0 + (i % 3) as f32;
        let points: Vec<Pos2> = (0..=60)
            .map(|k| {
                let y = rect.top() + k as f32 / 60.0 * rect.height();
                let x = x0 + amp * ((y * 0.013 + phase).sin() + 0.4 * (y * 0.041 + phase * 2.0).sin());
                Pos2::new(x, y)
            })
            .collect();
        let alpha = if i % 2 == 0 { 0.55 } else { 0.3 };
        painter.add(egui::Shape::line(points, Stroke::new(1.0 + (i % 2) as f32, th.wood_grain.gamma_multiply(alpha))));
    }
    // Rounded light edge on the inner side and shade on the outer side.
    painter.rect_filled(Rect::from_min_size(rect.min, Vec2::new(2.0, rect.height())), 0.0, Color32::from_black_alpha(90));
    painter.rect_filled(Rect::from_min_size(Pos2::new(rect.right() - 2.0, rect.top()), Vec2::new(2.0, rect.height())), 0.0, Color32::from_white_alpha(18));
}
