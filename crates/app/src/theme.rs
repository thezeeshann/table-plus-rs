//! Theme system.
//!
//! - `default` is index 0: the original Jade Dark look ported from TablePlus. Its colors are a
//!   fixed table (`JADE`), not derived from anything.
//! - The other four are Omarchy-derived palettes. A theme is just ten role colors;
//!   `derive()` expands them into every color the UI paints with.
//! - Selection precedence: `--theme NAME`, then `$TABLE_PLUS_RS_THEME`, then the saved
//!   setting, then default. A bad flag is an error; a bad env var or saved name falls
//!   through with a warning, so a typo can never lock you out.

use eframe::egui::{self, Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle};

pub type Rgb = [u8; 3];

/// Ten role colors, as `#rrggbb`.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub accent: &'static str,
    pub good: &'static str,
    pub warn: &'static str,
    pub bad: &'static str,
    pub info: &'static str,
    pub dim: &'static str,
    pub track: &'static str,
    pub cursor: &'static str,
    pub text: &'static str,
    pub bg: &'static str,
}

pub struct Theme {
    pub id: &'static str,
    pub name: &'static str,
    pub about: &'static str,
    /// `None` = the fixed Jade Dark table.
    pub palette: Option<Palette>,
}

const fn themed(
    accent: &'static str, good: &'static str, warn: &'static str, bad: &'static str, info: &'static str,
    dim: &'static str, track: &'static str, cursor: &'static str, text: &'static str, bg: &'static str,
) -> Palette {
    Palette { accent, good, warn, bad, info, dim, track, cursor, text, bg }
}

pub const THEMES: [Theme; 5] = [
    Theme {
        id: "default",
        name: "Default",
        about: "The original table-plus-rs look: Jade Dark, ported from TablePlus",
        palette: None,
    },
    Theme {
        id: "one-dark",
        name: "One Dark",
        about: "Atom's classic dark theme, with the brighter colors for status and values",
        palette: Some(themed("#61afef", "#a9d98a", "#f0d399", "#ff7b86", "#66d9e8", "#7f848e", "#3e4451", "#333842", "#abb2bf", "#282c34")),
    },
    Theme {
        id: "one-dark-pro",
        name: "One Dark Pro",
        about: "The VS Code take on One Dark: same base, standard colors, softer surfaces",
        palette: Some(themed("#61afef", "#98c379", "#e5c07b", "#e06c75", "#56b6c2", "#7a828e", "#4b5263", "#3e4451", "#abb2bf", "#282c34")),
    },
    Theme {
        id: "arc-blueberry",
        name: "Arc Blueberry",
        about: "Deep blue-black with soft periwinkle accents and neon green and cyan",
        palette: Some(themed("#8eb0e6", "#3cec85", "#eacd61", "#e35535", "#69c3ff", "#6f7594", "#424761", "#313b62", "#bcc1dc", "#111422")),
    },
    Theme {
        id: "atelier",
        name: "Atelier",
        about: "Warm olive-black with earthy greens, ochre and clay",
        palette: Some(themed("#36a166", "#a7c957", "#c2b53c", "#ba6236", "#5f9182", "#7d7c6b", "#3b3a2e", "#302f27", "#dcd9d6", "#22221b")),
    },
];

// ---------------------------------------------------------------- color math

pub fn parse_hex(h: &str) -> Rgb {
    let s = h.trim_start_matches('#');
    let p = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).unwrap_or(0);
    [p(0), p(2), p(4)]
}

pub fn to_hex(c: [f32; 3]) -> String {
    let f = |n: f32| n.round().clamp(0.0, 255.0) as u8;
    format!("#{:02x}{:02x}{:02x}", f(c[0]), f(c[1]), f(c[2]))
}

/// Linear blend: t=0 is `a`, t=1 is `b`.
pub fn mix(a: &str, b: &str, t: f32) -> String {
    let (x, y) = (parse_hex(a), parse_hex(b));
    let ch = |i: usize| x[i] as f32 + (y[i] as f32 - x[i] as f32) * t;
    to_hex([ch(0), ch(1), ch(2)])
}

pub fn luminance(h: &str) -> f32 {
    let [r, g, b] = parse_hex(h).map(|v| {
        let c = v as f32 / 255.0;
        if c <= 0.03928 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    });
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// WCAG contrast ratio.
pub fn contrast(a: &str, b: &str) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// `color` blended into `base`, as strongly as `target` allows while `fg` stays readable on the
/// result. Used for filled areas so a bright palette color never ends up behind text it cannot carry.
pub fn shade(color: &str, base: &str, fg: &str, target: f32, min: f32) -> String {
    let mut t = target;
    while t > 0.0 && contrast(fg, &mix(base, color, t)) < min {
        t -= 0.03;
    }
    mix(base, color, t.max(0.0))
}

const WHITE: &str = "#ffffff";
const BLACK: &str = "#000000";

/// Every color the UI paints with, as `#rrggbb` (alpha values are applied at paint time).
/// A few are only exercised by the contrast tests so far.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Colors {
    pub app_bg: String,
    pub content_bg: String,
    pub sidebar_bg: String,
    pub control_bg: String,
    pub control_border: String,
    pub separator: String,
    pub tab_bg: String,
    pub tab_hover: String,
    pub tab_selected: String,
    pub tab_text: String,
    pub text: String,
    pub text_dim: String,
    pub accent: String,
    pub selection: String,
    pub selection_border: String,
    pub grid_even: String,
    pub grid_header: String,
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
    pub ed_keyword: String,
    pub ed_string: String,
    pub ed_number: String,
    pub ed_comment: String,
    pub ed_function: String,
    pub ed_ident: String,
    pub ed_type: String,
    pub ed_punct: String,
    pub ed_current: String,
    /// Connection color tags 1..=5.
    pub conn: [String; 5],
}

const CONN: [&str; 5] = ["#007f3d", "#006978", "#005392", "#a3793e", "#6d0000"];

fn conn() -> [String; 5] {
    CONN.map(String::from)
}

/// The original Jade Dark look, straight from TablePlus' `jade-dark.json`.
pub fn jade() -> Colors {
    let s = String::from;
    Colors {
        app_bg: s("#0b2127"), content_bg: s("#002b36"), sidebar_bg: s("#00212b"), control_bg: s("#002e39"),
        control_border: s("#1e4550"), separator: s("#000000"), tab_bg: s("#003b4a"), tab_hover: s("#005167"),
        tab_selected: s("#216c82"), tab_text: s("#b2b2b2"), text: s("#d5d5d5"), text_dim: s("#7e7e7e"),
        accent: s("#529bd8"), selection: s("#004f6c"), selection_border: s("#ff4245"), grid_even: s("#003644"),
        grid_header: s("#002b36"), row_inserted: s("#558c3a"), row_modified: s("#8b6826"), row_deleted: s("#8f2f31"),
        status_ok: s("#558c3a"), status_warn: s("#8b6826"), status_err: s("#8f2f31"), status_border: s("#525252"),
        menu_bg: s("#0f2f3a"), btn_primary_hover: s("#2a86a1"), danger_text: s("#ff7b7b"), success_text: s("#8bd36a"),
        key_color: s("#f9b746"), ed_keyword: s("#529bd8"), ed_string: s("#ca6e6e"), ed_number: s("#b57aff"),
        ed_comment: s("#009b31"), ed_function: s("#7cb342"), ed_ident: s("#c0c0c0"), ed_type: s("#7cb342"),
        ed_punct: s("#9aa5a8"), ed_current: s("#003745"),
        conn: conn(),
    }
}

/// Expand ten role colors into every themeable color.
pub fn derive(p: &Palette) -> Colors {
    let app_bg = mix(p.bg, BLACK, 0.28);
    let sidebar = mix(p.bg, BLACK, 0.14);
    let control = mix(p.bg, p.cursor, 0.55);
    let tab_text = p.text; // no headroom to dim it: One Dark Pro text/cursor is barely 4.5:1
    // Hover lifts toward the accent; when the tab color has no headroom left it sinks toward the background.
    let lifted = shade(p.accent, p.cursor, tab_text, 0.2, 4.5);
    let tab_hover = if lifted != p.cursor { lifted } else { mix(p.cursor, p.bg, 0.45) };
    let tab_selected = shade(p.accent, p.cursor, WHITE, 0.4, 4.5);
    let selection = shade(p.accent, p.bg, p.text, 0.32, 4.5);
    let grid_even = mix(p.bg, p.cursor, 0.5);
    let current = mix(p.bg, p.cursor, 0.6);
    let ok = shade(p.good, p.bg, WHITE, 0.55, 4.5);
    let warn = shade(p.warn, p.bg, WHITE, 0.55, 4.5);
    let err = shade(p.bad, p.bg, WHITE, 0.55, 4.5);
    Colors {
        app_bg,
        content_bg: p.bg.into(),
        sidebar_bg: sidebar,
        control_bg: control,
        control_border: p.track.into(),
        separator: mix(p.bg, BLACK, 0.5),
        tab_bg: p.cursor.into(),
        tab_hover,
        tab_selected: tab_selected.clone(),
        tab_text: tab_text.into(),
        text: p.text.into(),
        text_dim: p.dim.into(),
        accent: p.accent.into(),
        selection,
        selection_border: p.bad.into(),
        grid_even,
        grid_header: p.bg.into(),
        row_inserted: ok.clone(),
        row_modified: warn.clone(),
        row_deleted: err.clone(),
        status_ok: ok,
        status_warn: warn,
        status_err: err,
        status_border: p.track.into(),
        menu_bg: p.cursor.into(),
        btn_primary_hover: mix(&tab_selected, WHITE, 0.12),
        danger_text: p.bad.into(),
        success_text: p.good.into(),
        key_color: p.warn.into(),
        ed_keyword: p.accent.into(),
        ed_string: p.good.into(),
        ed_number: p.warn.into(),
        ed_comment: p.dim.into(),
        ed_function: p.info.into(),
        ed_ident: p.text.into(),
        ed_type: p.info.into(),
        ed_punct: mix(p.dim, p.text, 0.35),
        ed_current: current,
        conn: conn(),
    }
}

pub fn colors_for(index: usize) -> Colors {
    match THEMES[index.min(THEMES.len() - 1)].palette {
        None => jade(),
        Some(p) => derive(&p),
    }
}

// ------------------------------------------------------ names and selection

/// Trim, lowercase, '_' and ' ' to '-', strip an "omarchy-" prefix and a "-theme" suffix.
pub fn normalize(name: &str) -> String {
    let mut n = name.trim().to_lowercase().replace(['_', ' '], "-");
    if let Some(r) = n.strip_prefix("omarchy-") {
        n = r.to_string();
    }
    if let Some(r) = n.strip_suffix("-theme") {
        n = r.to_string();
    }
    if n == "jade-dark" { "default".into() } else { n } // name used before themes existed
}

pub fn find(name: &str) -> Option<usize> {
    let id = normalize(name);
    THEMES.iter().position(|t| t.id == id)
}

pub fn ids() -> String {
    THEMES.iter().map(|t| t.id).collect::<Vec<_>>().join(", ")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Flag,
    Env,
    Saved,
    Default,
}

impl Source {
}

#[derive(Debug, Clone, PartialEq)]
pub struct Choice {
    pub index: usize,
    pub source: Source,
    pub warning: Option<String>,
}

fn present(s: Option<&str>) -> Option<&str> {
    s.filter(|v| !v.trim().is_empty())
}

pub fn resolve(flag: Option<&str>, env: Option<&str>, saved: Option<&str>) -> Result<Choice, String> {
    if let Some(f) = present(flag) {
        return match find(f) {
            Some(index) => Ok(Choice { index, source: Source::Flag, warning: None }),
            None => Err(format!("unknown theme `{f}`; choose one of: {}", ids())),
        };
    }
    let mut warning: Option<String> = None;
    if let Some(e) = present(env) {
        match find(e) {
            Some(index) => return Ok(Choice { index, source: Source::Env, warning: None }),
            None => warning = Some(format!("unknown theme `{e}` in $TABLE_PLUS_RS_THEME; ignoring it")),
        }
    }
    if let Some(s) = present(saved) {
        match find(s) {
            Some(index) => return Ok(Choice { index, source: Source::Saved, warning }),
            None => {
                let w = format!("unknown saved theme `{s}`; using the default");
                warning = Some(match warning {
                    Some(prev) => format!("{prev}; {w}"),
                    None => w,
                });
            }
        }
    }
    Ok(Choice { index: 0, source: Source::Default, warning })
}

/// `--theme NAME` or `--theme=NAME` from the command line.
pub fn flag_from_args<I: IntoIterator<Item = String>>(args: I) -> Option<String> {
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        if a == "--theme" {
            return it.next();
        }
        if let Some(v) = a.strip_prefix("--theme=") {
            return Some(v.to_string());
        }
    }
    None
}

// ------------------------------------------------------------- egui styling

pub fn c32(hex: &str) -> Color32 {
    let [r, g, b] = parse_hex(hex);
    Color32::from_rgb(r, g, b)
}

pub fn c32a(hex: &str, alpha: f32) -> Color32 {
    let [r, g, b] = parse_hex(hex);
    Color32::from_rgba_unmultiplied(r, g, b, (alpha * 255.0) as u8)
}

pub fn conn_color(colors: &Colors, tag: &str) -> Color32 {
    let i = tag.parse::<usize>().unwrap_or(1).clamp(1, 5) - 1;
    c32(&colors.conn[i])
}

pub fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

/// Writes the theme into egui's global style.
pub fn apply(ctx: &egui::Context, colors: &Colors) {
    let mut style = (*ctx.global_style()).clone();
    style.text_styles = [
        (TextStyle::Small, FontId::new(11.0, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(13.0, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(13.0, FontFamily::Proportional)),
        (TextStyle::Heading, FontId::new(18.0, FontFamily::Proportional)),
        (TextStyle::Monospace, mono(12.5)),
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
    v.extreme_bg_color = c32(&colors.control_bg);
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
        (&mut w.inactive, c32(&colors.tab_bg)),
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

    fn themed_palettes() -> Vec<(&'static str, Palette)> {
        THEMES.iter().filter_map(|t| t.palette.map(|p| (t.id, p))).collect()
    }

    #[test]
    fn ids_are_unique_and_default_is_first() {
        let ids: Vec<_> = THEMES.iter().map(|t| t.id).collect();
        let mut sorted = ids.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len());
        assert_eq!(THEMES[0].id, "default");
        assert!(THEMES[0].palette.is_none());
        assert_eq!(THEMES.len(), 5);
    }

    #[test]
    fn names_resolve() {
        let cases = [
            ("omarchy-one-dark-theme", "one-dark"), ("omarchy-one-dark-pro-theme", "one-dark-pro"),
            ("omarchy-arc-blueberry", "arc-blueberry"), ("omarchy-atelier-theme", "atelier"),
            ("One Dark Pro", "one-dark-pro"), ("ARC_BLUEBERRY", "arc-blueberry"), ("  atelier  ", "atelier"),
            ("default", "default"), ("jade-dark", "default"),
        ];
        for (input, id) in cases {
            assert_eq!(THEMES[find(input).unwrap()].id, id, "{input}");
        }
        assert_eq!(find("nope"), None);
        assert_eq!(find(""), None);
        assert_eq!(normalize(" Omarchy_One Dark_Theme "), "one-dark");
    }

    #[test]
    fn resolution_order_is_flag_env_saved_default() {
        let id = |r: Result<Choice, String>| THEMES[r.unwrap().index].id;
        assert_eq!(id(resolve(Some("atelier"), Some("one-dark"), Some("arc-blueberry"))), "atelier");
        assert_eq!(id(resolve(None, Some("one-dark"), Some("arc-blueberry"))), "one-dark");
        assert_eq!(id(resolve(None, None, Some("arc-blueberry"))), "arc-blueberry");
        assert_eq!(id(resolve(None, None, None)), "default");
        assert_eq!(id(resolve(None, Some("   "), Some("atelier"))), "atelier");
        let r = resolve(None, Some(""), Some("atelier")).unwrap();
        assert_eq!(r.source, Source::Saved);
    }

    #[test]
    fn bad_flag_is_an_error_but_bad_env_or_saved_fall_through() {
        let bad = resolve(Some("bogus"), None, None).unwrap_err();
        assert!(bad.contains(&ids()));
        let env = resolve(None, Some("bogus"), Some("atelier")).unwrap();
        assert_eq!(THEMES[env.index].id, "atelier");
        assert!(env.warning.unwrap().contains("bogus"));
        let saved = resolve(None, None, Some("bogus")).unwrap();
        assert_eq!(saved.index, 0);
        assert!(saved.warning.unwrap().contains("bogus"));
    }

    #[test]
    fn legacy_saved_value_resolves_to_default_without_a_warning() {
        let r = resolve(None, None, Some("jade-dark")).unwrap();
        assert_eq!((r.index, r.source, r.warning), (0, Source::Saved, None));
    }

    #[test]
    fn flag_parsing_accepts_both_forms() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(flag_from_args(a(&["--theme", "atelier"])), Some("atelier".into()));
        assert_eq!(flag_from_args(a(&["x", "--theme=one-dark"])), Some("one-dark".into()));
        assert_eq!(flag_from_args(a(&["--other"])), None);
        assert_eq!(flag_from_args(a(&["--theme"])), None);
    }

    #[test]
    fn palette_contrast() {
        for (id, p) in themed_palettes() {
            assert!(contrast(p.text, p.bg) >= 6.0, "{id} text/bg");
            for (k, c) in [("accent", p.accent), ("good", p.good), ("warn", p.warn), ("bad", p.bad), ("info", p.info), ("dim", p.dim)] {
                assert!(contrast(c, p.bg) >= 3.0, "{id} {k}/bg {}", contrast(c, p.bg));
            }
            assert!(contrast(p.track, p.bg) >= 1.3, "{id} track/bg");
            assert!(contrast(p.cursor, p.bg) >= 1.1, "{id} cursor/bg");
            assert!(contrast(p.text, p.cursor) >= 4.5, "{id} text/cursor");
        }
    }

    #[test]
    fn good_warn_bad_are_pairwise_distinct() {
        for (id, p) in themed_palettes() {
            assert!(p.good != p.warn && p.warn != p.bad && p.good != p.bad, "{id}");
        }
    }

    #[test]
    fn every_palette_role_is_rrggbb() {
        for (id, p) in themed_palettes() {
            for c in [p.accent, p.good, p.warn, p.bad, p.info, p.dim, p.track, p.cursor, p.text, p.bg] {
                assert!(c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|d| d.is_ascii_hexdigit()), "{id} {c}");
            }
        }
    }

    #[test]
    fn derived_ui_colors_keep_text_readable() {
        for (id, p) in themed_palettes() {
            let v = derive(&p);
            for (n, bg) in [
                ("content", &v.content_bg), ("sidebar", &v.sidebar_bg), ("app", &v.app_bg), ("control", &v.control_bg),
                ("menu", &v.menu_bg), ("grid-even", &v.grid_even), ("tab", &v.tab_bg), ("tab-hover", &v.tab_hover),
                ("selection", &v.selection), ("current", &v.ed_current),
            ] {
                assert!(contrast(&v.text, bg) >= 4.5, "{id} text on {n}: {:.2}", contrast(&v.text, bg));
            }
            for bg in [&v.tab_bg, &v.tab_hover] {
                assert!(contrast(&v.tab_text, bg) >= 4.5, "{id} tab-text");
            }
            for (n, fill) in [
                ("tab-selected", &v.tab_selected), ("inserted", &v.row_inserted), ("modified", &v.row_modified),
                ("deleted", &v.row_deleted), ("ok", &v.status_ok), ("warn", &v.status_warn), ("err", &v.status_err),
            ] {
                assert!(contrast(WHITE, fill) >= 4.5, "{id} white on {n}: {:.2}", contrast(WHITE, fill));
            }
            assert!(contrast(WHITE, &v.btn_primary_hover) >= 3.5, "{id} btn-primary-hover");
            for (n, fg) in [
                ("danger", &v.danger_text), ("success", &v.success_text), ("key", &v.key_color), ("keyword", &v.ed_keyword),
                ("string", &v.ed_string), ("number", &v.ed_number), ("comment", &v.ed_comment), ("function", &v.ed_function),
                ("punct", &v.ed_punct),
            ] {
                assert!(contrast(fg, &v.content_bg) >= 3.0, "{id} {n} on content: {:.2}", contrast(fg, &v.content_bg));
            }
        }
    }

    #[test]
    fn row_states_stay_distinguishable() {
        for (id, p) in themed_palettes() {
            let v = derive(&p);
            assert!(v.row_inserted != v.row_modified && v.row_modified != v.row_deleted && v.row_inserted != v.row_deleted, "{id}");
        }
    }

    #[test]
    fn mix_and_shade() {
        assert_eq!(mix("#000000", "#ffffff", 0.0), "#000000");
        assert_eq!(mix("#000000", "#ffffff", 1.0), "#ffffff");
        assert_eq!(mix("#000000", "#ffffff", 0.5), "#808080");
        assert!(contrast("#ffffff", &shade("#ffff00", "#000000", "#ffffff", 0.9, 4.5)) >= 4.5);
    }

    #[test]
    fn jade_matches_the_stylesheet_it_was_ported_from() {
        let j = jade();
        assert_eq!(j.content_bg, "#002b36");
        assert_eq!(j.conn[0], "#007f3d");
        assert_eq!(colors_for(0).tab_selected, "#216c82");
    }
}
