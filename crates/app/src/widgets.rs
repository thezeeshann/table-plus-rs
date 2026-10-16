//! Small reusable widgets with the app's look.

use eframe::egui::{self, Color32, CornerRadius, Response, RichText, Sense, Stroke, StrokeKind, Ui};

use crate::theme::{c32, Colors};

/// Accent-colored call-to-action button.
pub fn primary_button(ui: &mut Ui, c: &Colors, text: &str, enabled: bool) -> Response {
    let fill = if enabled { c32(&c.accent) } else { c32(&c.accent).gamma_multiply(0.45) };
    let button = egui::Button::new(RichText::new(text).color(Color32::WHITE).strong())
        .fill(fill)
        .stroke(Stroke::NONE)
        .corner_radius(CornerRadius::same(6))
        .min_size(egui::vec2(72.0, 26.0));
    ui.add_enabled(enabled, button)
}

pub fn button(ui: &mut Ui, text: &str) -> Response {
    ui.add(egui::Button::new(text).min_size(egui::vec2(0.0, 26.0)))
}

pub fn small_button(ui: &mut Ui, text: &str) -> Response {
    ui.add(egui::Button::new(RichText::new(text).size(12.0)).min_size(egui::vec2(0.0, 22.0)))
}

/// Small rounded label (`production`, `PK`, `view`…).
pub fn pill(ui: &mut Ui, text: &str, color: Color32) -> Response {
    let galley = ui.painter().layout_no_wrap(text.to_string(), egui::FontId::proportional(10.0), color);
    let size = galley.size() + egui::vec2(10.0, 4.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(4), color.gamma_multiply(0.18));
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, color);
    resp
}

pub fn dim(ui: &Ui, text: impl Into<String>) -> RichText {
    RichText::new(text.into()).color(ui.visuals().weak_text_color())
}

pub fn section_title(ui: &mut Ui, c: &Colors, text: &str) {
    ui.label(RichText::new(text.to_uppercase()).size(10.5).strong().color(c32(&c.text_dim)));
}

/// macOS-style segmented control. Returns true when the selection changed.
pub fn segmented<T: PartialEq + Copy>(ui: &mut Ui, c: &Colors, value: &mut T, options: &[(T, &str)]) -> bool {
    let font = egui::FontId::proportional(13.0);
    let widths: Vec<f32> = options
        .iter()
        .map(|(_, l)| ui.painter().layout_no_wrap((*l).to_string(), font.clone(), Color32::WHITE).size().x + 20.0)
        .collect();
    let total = egui::vec2(widths.iter().sum::<f32>() + 4.0, 26.0);
    let (rect, _) = ui.allocate_exact_size(total, Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(6), c32(&c.control_bg));
    let mut x = rect.left() + 2.0;
    let mut changed = false;
    for ((opt, label), w) in options.iter().zip(widths) {
        let seg = egui::Rect::from_min_size(egui::pos2(x, rect.top() + 2.0), egui::vec2(w, rect.height() - 4.0));
        let resp = ui.interact(seg, ui.id().with(("seg", *label)), Sense::click());
        let selected = *value == *opt;
        if selected {
            ui.painter().rect_filled(seg, CornerRadius::same(4), c32(&c.accent));
        } else if resp.hovered() {
            ui.painter().rect_filled(seg, CornerRadius::same(4), c32(&c.tab_hover));
        }
        let color = if selected { Color32::WHITE } else { c32(&c.tab_text) };
        ui.painter().text(seg.center(), egui::Align2::CENTER_CENTER, *label, font.clone(), color);
        if resp.clicked() && !selected {
            *value = *opt;
            changed = true;
        }
        x += w;
    }
    changed
}

#[derive(Clone, Copy, PartialEq)]
pub enum IconKind {
    Table,
    Query,
    Processes,
    View,
    Matview,
    Function,
    Sequence,
}

impl IconKind {
    pub fn of_item(kind: &str) -> Self {
        match kind {
            "view" => Self::View,
            "matview" => Self::Matview,
            "function" => Self::Function,
            "sequence" => Self::Sequence,
            _ => Self::Table,
        }
    }
}

/// Thin line icons for tabs and the object tree, drawn with the painter (the bundled fonts have no glyphs for them).
pub fn paint_tab_icon(p: &egui::Painter, c: egui::Pos2, kind: IconKind, color: Color32) {
    let st = Stroke::new(1.2, color);
    let v = egui::vec2;
    match kind {
        IconKind::Table => {
            let r = egui::Rect::from_center_size(c, v(11.0, 10.0));
            p.rect_stroke(r, CornerRadius::same(1), st, StrokeKind::Inside);
            p.line_segment([egui::pos2(r.left(), c.y - 1.5), egui::pos2(r.right(), c.y - 1.5)], st);
            p.line_segment([egui::pos2(c.x, r.top()), egui::pos2(c.x, r.bottom())], st);
        }
        IconKind::Query => {
            p.line_segment([c + v(-5.0, -3.5), c + v(-1.5, 0.0)], st);
            p.line_segment([c + v(-1.5, 0.0), c + v(-5.0, 3.5)], st);
            p.line_segment([c + v(0.5, 4.0), c + v(5.0, 4.0)], st);
        }
        IconKind::Processes => {
            let pts = [c + v(-6.0, 0.0), c + v(-3.0, 0.0), c + v(-1.0, -5.0), c + v(2.0, 5.0), c + v(4.0, 0.0), c + v(6.0, 0.0)];
            p.add(egui::Shape::line(pts.to_vec(), st));
        }
        IconKind::View => {
            let pts: Vec<egui::Pos2> = (0..=16)
                .map(|i| {
                    let t = i as f32 / 16.0;
                    c + v(-6.5 + 13.0 * t, -3.8 * (std::f32::consts::PI * t).sin())
                })
                .chain((0..=16).rev().map(|i| {
                    let t = i as f32 / 16.0;
                    c + v(-6.5 + 13.0 * t, 3.8 * (std::f32::consts::PI * t).sin())
                }))
                .collect();
            p.add(egui::Shape::line(pts, st));
            p.circle_stroke(c, 1.8, st);
        }
        IconKind::Matview => {
            for (dy, a) in [(-3.0, 1.0), (0.0, 0.7), (3.0, 0.45)] {
                let st = Stroke::new(1.2, color.gamma_multiply(a));
                p.add(egui::Shape::closed_line(vec![c + v(0.0, dy - 3.0), c + v(6.0, dy), c + v(0.0, dy + 3.0), c + v(-6.0, dy)], st));
            }
        }
        IconKind::Function => {
            p.text(c, egui::Align2::CENTER_CENTER, "ƒ", egui::FontId::proportional(13.0), color);
        }
        IconKind::Sequence => {
            p.text(c, egui::Align2::CENTER_CENTER, "#", egui::FontId::proportional(13.0), color);
        }
    }
}

/// Low-poly origami elephant for the empty workspace, drawn from flat facets so it follows the theme.
/// Coordinates are in a 500x360 box (origin top-left); `shade` is how far a facet leans from the
/// background toward the text color.
pub fn paint_elephant(p: &egui::Painter, center: egui::Pos2, width: f32, c: &Colors) {
    const F: &[(&[(f32, f32)], f32)] = &[
        (&[(197.0, 11.0), (291.0, 0.0), (275.0, 22.0), (205.0, 38.0)], 0.17),
        (&[(291.0, 0.0), (392.0, 64.0), (275.0, 22.0)], 0.27),
        (&[(168.0, 33.0), (275.0, 22.0), (205.0, 70.0)], 0.21),
        (&[(168.0, 33.0), (205.0, 70.0), (142.0, 138.0)], 0.17),
        (&[(142.0, 138.0), (275.0, 22.0), (258.0, 228.0)], 0.30),
        (&[(275.0, 22.0), (364.0, 183.0), (258.0, 228.0)], 0.27),
        (&[(275.0, 22.0), (392.0, 64.0), (460.0, 195.0), (364.0, 183.0)], 0.24),
        (&[(433.0, 143.0), (481.0, 105.0), (500.0, 125.0), (458.0, 195.0)], 0.23),
        (&[(54.0, 147.0), (145.0, 125.0), (170.0, 200.0), (77.0, 200.0)], 0.16),
        (&[(33.0, 246.0), (77.0, 200.0), (170.0, 200.0), (180.0, 288.0)], 0.20),
        (&[(170.0, 200.0), (258.0, 228.0), (231.0, 303.0), (180.0, 288.0)], 0.22),
        (&[(258.0, 228.0), (364.0, 183.0), (335.0, 258.0), (231.0, 303.0)], 0.19),
        (&[(55.0, 274.0), (180.0, 288.0), (130.0, 359.0), (77.0, 360.0)], 0.18),
        (&[(180.0, 288.0), (155.0, 350.0), (130.0, 359.0)], 0.12),
        (&[(231.0, 303.0), (335.0, 258.0), (300.0, 360.0), (241.0, 360.0)], 0.21),
        (&[(335.0, 258.0), (325.0, 350.0), (300.0, 360.0)], 0.11),
        (&[(0.0, 278.0), (22.0, 283.0), (35.0, 302.0)], 0.16),
        (&[(22.0, 283.0), (33.0, 246.0), (37.0, 302.0)], 0.18),
    ];
    let k = width / 500.0;
    let origin = center - egui::vec2(250.0, 180.0) * k;
    let (bg, fg) = (c32(&c.content_bg), c32(&c.text));
    let mix = |t: f32| {
        let f = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Color32::from_rgb(f(bg.r(), fg.r()), f(bg.g(), fg.g()), f(bg.b(), fg.b()))
    };
    for (pts, shade) in F {
        let poly: Vec<egui::Pos2> = pts.iter().map(|&(x, y)| origin + egui::vec2(x, y) * k).collect();
        p.add(egui::Shape::convex_polygon(poly, mix(*shade * 0.7), Stroke::NONE));
    }
}

/// A modal dialog with the app's frame.
pub fn modal(_ctx: &egui::Context, c: &Colors, id: &'static str) -> egui::Modal {
    egui::Modal::new(egui::Id::new(id)).frame(
        egui::Frame::new()
            .fill(c32(&c.menu_bg))
            .stroke(Stroke::new(1.0, c32(&c.control_border)))
            .corner_radius(CornerRadius::same(12))
            .inner_margin(egui::Margin::same(20)),
    )
}

/// `1234567` -> `1,234,567`.
pub fn fmt_count(n: i64) -> String {
    let s = n.abs().to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    if n < 0 { format!("-{out}") } else { out }
}

pub fn fmt_ms(ms: f64) -> String {
    if ms < 1.0 {
        format!("{ms:.2} ms")
    } else if ms < 1000.0 {
        format!("{ms:.0} ms")
    } else {
        format!("{:.2} s", ms / 1000.0)
    }
}

pub fn fmt_bytes(n: i64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut u = 0;
    while v >= 1024.0 && u < units.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 { format!("{n} B") } else { format!("{v:.1} {}", units[u]) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatting() {
        assert_eq!(fmt_count(0), "0");
        assert_eq!(fmt_count(999), "999");
        assert_eq!(fmt_count(1000), "1,000");
        assert_eq!(fmt_count(2_000_000), "2,000,000");
        assert_eq!(fmt_ms(23.0), "23 ms");
        assert_eq!(fmt_ms(1500.0), "1.50 s");
        assert_eq!(fmt_bytes(512), "512 B");
        assert_eq!(fmt_bytes(2048), "2.0 KB");
    }
}
