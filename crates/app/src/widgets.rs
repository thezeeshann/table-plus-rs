//! Small reusable widgets with the app's look.

use eframe::egui::{self, Color32, CornerRadius, Response, RichText, Sense, Stroke, StrokeKind, Ui};

use crate::theme::{c32, Colors};

/// Accent-colored call-to-action button.
pub fn primary_button(ui: &mut Ui, c: &Colors, text: &str, enabled: bool) -> Response {
    let fill = if enabled { c32(&c.tab_selected) } else { c32(&c.tab_selected).gamma_multiply(0.45) };
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
    let font = egui::FontId::proportional(12.0);
    let widths: Vec<f32> = options
        .iter()
        .map(|(_, l)| ui.painter().layout_no_wrap((*l).to_string(), font.clone(), Color32::WHITE).size().x + 22.0)
        .collect();
    let total = egui::vec2(widths.iter().sum::<f32>() + 4.0, 24.0);
    let (rect, _) = ui.allocate_exact_size(total, Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(7), c32(&c.control_bg));
    ui.painter().rect_stroke(rect, CornerRadius::same(7), Stroke::new(1.0, c32(&c.control_border)), StrokeKind::Inside);
    let mut x = rect.left() + 2.0;
    let mut changed = false;
    for ((opt, label), w) in options.iter().zip(widths) {
        let seg = egui::Rect::from_min_size(egui::pos2(x, rect.top() + 2.0), egui::vec2(w, rect.height() - 4.0));
        let resp = ui.interact(seg, ui.id().with(("seg", *label)), Sense::click());
        let selected = *value == *opt;
        if selected {
            ui.painter().rect_filled(seg, CornerRadius::same(5), c32(&c.tab_selected));
        } else if resp.hovered() {
            ui.painter().rect_filled(seg, CornerRadius::same(5), c32(&c.tab_hover));
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
