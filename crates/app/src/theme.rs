//! Colors and fonts. There is one look: TablePlus' built-in Dark theme.
//!
//! The values were measured from TablePlus itself (REA native-UI captures of a live window,
//! converted from Display P3 to sRGB), not taken from a theme file: the built-in theme has no JSON
//! in the bundle. Its SQL colors turned out identical to the ones in TablePlus' `jade-dark.json`.
//! Where TablePlus draws a color it never showed during the capture (edited-row fills, the cell
//! focus ring) the value is a choice and is marked as such.

use eframe::egui::{self, Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle};

pub type Rgb = [u8; 3];

pub fn parse_hex(h: &str) -> Rgb {
    let s = h.trim_start_matches('#');
    let p = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).unwrap_or(0);
    [p(0), p(2), p(4)]
}

#[cfg(test)]
fn luminance(h: &str) -> f32 {
    let [r, g, b] = parse_hex(h).map(|v| {
        let c = v as f32 / 255.0;
        if c <= 0.03928 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    });
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// WCAG contrast ratio.
#[cfg(test)]
fn contrast(a: &str, b: &str) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// Every color the UI paints with, as `#rrggbb` (alpha values are applied at paint time).
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Colors {
    /// Window chrome: toolbar and start screen.
    pub app_bg: String,
    /// Editor, empty workspace, tab strip and bottom bars.
    pub content_bg: String,
    pub sidebar_bg: String,
    /// Buttons, popups and fields.
    pub control_bg: String,
    pub control_border: String,
    /// The near-black line between the sidebar and the content.
    pub separator: String,
    /// Rounded background behind each group of toolbar icons.
    pub toolbar_group: String,
    pub tab_bg: String,
    pub tab_hover: String,
    pub tab_selected: String,
    pub tab_text: String,
    /// Selected word in the sidebar's Items / Queries / History switch.
    pub mode_selected: String,
    pub text: String,
    pub text_dim: String,
    /// macOS control accent: selected segment, primary button.
    pub accent: String,
    /// Selected rows (sidebar and grid).
    pub selection: String,
    pub selection_border: String,
    pub grid_odd: String,
    pub grid_even: String,
    pub grid_header: String,
    pub grid_line: String,
    pub null_text: String,
    pub row_inserted: String,
    pub row_modified: String,
    pub row_deleted: String,
    pub status_ok: String,
    pub status_warn: String,
    pub status_err: String,
    pub status_border: String,
    pub menu_bg: String,
    pub btn_primary_hover: String,
    pub danger_text: String,
    pub success_text: String,
    pub key_color: String,
    pub ed_text: String,
    pub ed_keyword: String,
    pub ed_string: String,
    pub ed_quoted: String,
    pub ed_number: String,
    pub ed_comment: String,
    pub ed_function: String,
    pub ed_ident: String,
    pub ed_type: String,
    pub ed_punct: String,
    pub ed_current: String,
    pub ed_line_number: String,
    /// Connection color tags 1..=5.
    pub conn: [String; 5],
}

/// TablePlus' five connection color presets.
const CONN: [&str; 5] = ["#007f3d", "#006978", "#005392", "#a3793e", "#6d0000"];

/// TablePlus' built-in Dark theme.
pub fn tableplus() -> Colors {
    let s = String::from;
    Colors {
        app_bg: s("#1a1a1a"),
        content_bg: s("#242424"),
        sidebar_bg: s("#1f1f1f"),
        control_bg: s("#343434"),
        control_border: s("#353535"),
        separator: s("#000000"),
        toolbar_group: s("#3a3a3a"),
        tab_bg: s("#343535"),
        tab_hover: s("#454646"),
        tab_selected: s("#6f6f6f"),
        tab_text: s("#b3b3b3"),
        mode_selected: s("#3b3b3b"),
        text: s("#d6d6d6"),
        text_dim: s("#7f7f7f"),
        accent: s("#117dff"),
        selection: s("#005ad1"),
        selection_border: s("#117dff"), // choice: focus ring in the accent
        grid_odd: s("#1e1e1e"),
        grid_even: s("#2e2e2e"),
        grid_header: s("#242424"),
        grid_line: s("#353535"),
        null_text: s("#434343"),
        // not seen in the capture: TablePlus' edit colors from its bundled theme files
        row_inserted: s("#558c3a"),
        row_modified: s("#8b6826"),
        row_deleted: s("#8f2f31"),
        status_ok: s("#558c3a"),
        status_warn: s("#8b6826"),
        status_err: s("#8f2f31"),
        status_border: s("#525252"),
        menu_bg: s("#2a2a2a"),
        btn_primary_hover: s("#3d95ff"),
        danger_text: s("#ff6b6b"),
        success_text: s("#7ec27e"),
        key_color: s("#f9b746"),
        ed_text: s("#c0c0c0"),
        ed_keyword: s("#529bd8"),
        ed_string: s("#ca6e6e"),
        ed_quoted: s("#e04978"),
        ed_number: s("#b57aff"),
        ed_comment: s("#009b31"),
        ed_function: s("#c0c0c0"), // TablePlus leaves calls like now() uncolored
        ed_ident: s("#c0c0c0"),
        ed_type: s("#529bd8"),
        ed_punct: s("#c0c0c0"),
        ed_current: s("#272727"),
        ed_line_number: s("#7f7f7f"),
        conn: CONN.map(String::from),
    }
}

// ------------------------------------------------------------- egui styling

pub fn c32(hex: &str) -> Color32 {
    let [r, g, b] = parse_hex(hex);
    Color32::from_rgb(r, g, b)
}

pub fn conn_color(colors: &Colors, tag: &str) -> Color32 {
    let i = tag.parse::<usize>().unwrap_or(1).clamp(1, 5) - 1;
    c32(&colors.conn[i])
}

/// macOS system fonts, as TablePlus uses them: SF Pro for the interface, SF Mono for data, the
/// sidebar items, the editor and the status pill. Apple Symbols backs both for icon glyphs.
/// Anything missing is skipped and egui's own fonts carry on.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let mut add = |name: &str, path: &str, families: &[(FontFamily, bool)]| {
        let Ok(bytes) = std::fs::read(path) else { return };
        fonts.font_data.insert(name.to_owned(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
        for (fam, front) in families {
            let list = fonts.families.entry(fam.clone()).or_default();
            if *front { list.insert(0, name.to_owned()) } else { list.push(name.to_owned()) }
        }
    };
    add("sf-pro", "/System/Library/Fonts/SFNS.ttf", &[(FontFamily::Proportional, true)]);
    add("sf-mono", "/System/Library/Fonts/SFNSMono.ttf", &[(FontFamily::Monospace, true)]);
    add(
        "apple-symbols",
        "/System/Library/Fonts/Apple Symbols.ttf",
        &[(FontFamily::Proportional, false), (FontFamily::Monospace, false)],
    );
    ctx.set_fonts(fonts);
}

pub fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

/// Writes the colors and text styles into egui's global style.
pub fn apply(ctx: &egui::Context, colors: &Colors) {
    let mut style = (*ctx.global_style()).clone();
    style.text_styles = [
        (TextStyle::Small, FontId::new(11.0, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(13.0, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(13.0, FontFamily::Proportional)),
        (TextStyle::Heading, FontId::new(18.0, FontFamily::Proportional)),
        (TextStyle::Monospace, mono(12.0)),
    ]
    .into();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 4.0);
    style.spacing.interact_size = egui::vec2(24.0, 24.0);
    style.spacing.scroll.bar_width = 8.0;
    style.spacing.scroll.floating = true;

    let mut v = egui::Visuals::dark();
    v.panel_fill = c32(&colors.content_bg);
    v.window_fill = c32(&colors.menu_bg);
    v.window_stroke = Stroke::new(1.0, c32(&colors.control_border));
    v.window_corner_radius = CornerRadius::same(10);
    v.menu_corner_radius = CornerRadius::same(7);
    v.extreme_bg_color = c32("#171717"); // text fields (the sidebar search box)
    v.faint_bg_color = c32(&colors.grid_even);
    v.code_bg_color = c32(&colors.grid_even);
    v.hyperlink_color = c32(&colors.accent);
    v.error_fg_color = c32(&colors.danger_text);
    v.warn_fg_color = c32(&colors.key_color);
    v.selection.bg_fill = c32(&colors.selection);
    v.selection.stroke = Stroke::new(1.0, c32(&colors.accent));
    v.override_text_color = Some(c32(&colors.text));
    v.weak_text_color = Some(c32(&colors.text_dim));

    let r = CornerRadius::same(5);
    let w = &mut v.widgets;
    let border = c32(&colors.control_border);
    w.noninteractive.bg_fill = c32(&colors.content_bg);
    w.noninteractive.weak_bg_fill = c32(&colors.content_bg);
    w.noninteractive.bg_stroke = Stroke::new(1.0, border);
    w.noninteractive.fg_stroke = Stroke::new(1.0, c32(&colors.text));
    w.noninteractive.corner_radius = r;
    for (state, fill) in [
        (&mut w.inactive, c32(&colors.control_bg)),
        (&mut w.hovered, c32(&colors.tab_hover)),
        (&mut w.active, c32(&colors.tab_selected)),
        (&mut w.open, c32(&colors.tab_hover)),
    ] {
        state.bg_fill = fill;
        state.weak_bg_fill = fill;
        state.bg_stroke = Stroke::new(1.0, border);
        state.fg_stroke = Stroke::new(1.0, c32(&colors.text));
        state.corner_radius = r;
        state.expansion = 0.0;
    }
    w.active.bg_stroke = Stroke::new(1.0, c32(&colors.accent));
    style.visuals = v;
    ctx.set_global_style(style);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_measured_tableplus_dark_theme() {
        let c = tableplus();
        assert_eq!(c.app_bg, "#1a1a1a");
        assert_eq!(c.sidebar_bg, "#1f1f1f");
        assert_eq!((c.grid_odd.as_str(), c.grid_even.as_str()), ("#1e1e1e", "#2e2e2e"));
        assert_eq!(c.selection, "#005ad1");
        assert_eq!(c.conn[0], "#007f3d");
    }

    #[test]
    fn every_color_is_rrggbb() {
        let c = tableplus();
        let all = format!("{c:?}");
        for hex in all.split('"').filter(|s| s.starts_with('#')) {
            assert!(hex.len() == 7 && hex[1..].chars().all(|d| d.is_ascii_hexdigit()), "{hex}");
        }
    }

    #[test]
    fn text_stays_readable() {
        let c = tableplus();
        for (n, bg) in [
            ("app", &c.app_bg), ("content", &c.content_bg), ("sidebar", &c.sidebar_bg), ("control", &c.control_bg),
            ("menu", &c.menu_bg), ("grid-odd", &c.grid_odd), ("grid-even", &c.grid_even), ("header", &c.grid_header),
        ] {
            assert!(contrast(&c.text, bg) >= 4.5, "text on {n}: {:.2}", contrast(&c.text, bg));
        }
        // selected rows and segments carry white text, as in TablePlus
        for bg in [&c.selection, &c.accent, &c.tab_selected] {
            assert!(contrast("#ffffff", bg) >= 3.0, "white on {bg}");
        }
        for (n, fg) in [("keyword", &c.ed_keyword), ("string", &c.ed_string), ("number", &c.ed_number), ("comment", &c.ed_comment)] {
            assert!(contrast(fg, &c.content_bg) >= 3.0, "{n}: {:.2}", contrast(fg, &c.content_bg));
        }
    }
}
