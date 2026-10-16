//! SQL text: syntax highlighting, a read-only viewer and the editor with schema-aware autocomplete.

use std::collections::BTreeMap;

use eframe::egui::{
    self, text::{CCursor, CCursorRange, LayoutJob}, text_edit::TextEditState, Align2, CornerRadius, FontId, Id, Key,
    Modifiers, Order, Stroke, TextFormat, TextEdit, Ui,
};
use table_plus_core::pg::sqlgen::split_statements;

use crate::theme::{c32, mono, Colors};

const KEYWORDS: &[&str] = &[
    "SELECT", "FROM", "WHERE", "AND", "OR", "NOT", "NULL", "INSERT", "INTO", "VALUES", "UPDATE", "SET", "DELETE", "CREATE",
    "TABLE", "ALTER", "DROP", "INDEX", "VIEW", "MATERIALIZED", "JOIN", "LEFT", "RIGHT", "INNER", "OUTER", "FULL", "CROSS",
    "ON", "USING", "GROUP", "BY", "ORDER", "HAVING", "LIMIT", "OFFSET", "AS", "DISTINCT", "UNION", "ALL", "INTERSECT",
    "EXCEPT", "CASE", "WHEN", "THEN", "ELSE", "END", "IN", "IS", "LIKE", "ILIKE", "BETWEEN", "EXISTS", "ANY", "WITH",
    "RECURSIVE", "RETURNING", "PRIMARY", "KEY", "FOREIGN", "REFERENCES", "UNIQUE", "CHECK", "DEFAULT", "CONSTRAINT",
    "CASCADE", "RESTRICT", "BEGIN", "COMMIT", "ROLLBACK", "TRANSACTION", "EXPLAIN", "ANALYZE", "VACUUM", "TRUNCATE",
    "GRANT", "REVOKE", "SCHEMA", "SEQUENCE", "FUNCTION", "TRIGGER", "RETURNS", "LANGUAGE", "REPLACE", "ASC", "DESC",
    "TRUE", "FALSE", "COLUMN", "ADD", "RENAME", "TO", "IF", "OVER", "PARTITION", "WINDOW", "FETCH", "NEXT", "ROWS", "ONLY",
    "FILTER", "LATERAL", "NULLS", "FIRST", "LAST", "COMMENT", "COPY", "SHOW", "DO", "FOR", "EACH", "ROW", "EXECUTE",
];

const TYPES: &[&str] = &[
    "int", "integer", "bigint", "smallint", "serial", "bigserial", "text", "varchar", "char", "boolean", "bool", "numeric",
    "decimal", "real", "float", "double", "precision", "date", "time", "timestamp", "timestamptz", "interval", "uuid",
    "json", "jsonb", "bytea", "inet", "cidr", "int4", "int8", "int2", "float4", "float8",
];

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

/// Colors a SQL string. Pure function so it can be reused by the editor and read-only views.
pub fn highlight(colors: &Colors, text: &str, size: f32, wrap_width: f32) -> LayoutJob {
    let font = mono(size);
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap_width;
    let fmt = |hex: &str| TextFormat { font_id: font.clone(), color: c32(hex), ..Default::default() };
    let (kw, st, num, cm, func, ident, ty, punct, quoted) = (
        fmt(&colors.ed_keyword), fmt(&colors.ed_string), fmt(&colors.ed_number), fmt(&colors.ed_comment),
        fmt(&colors.ed_function), fmt(&colors.ed_ident), fmt(&colors.ed_type), fmt(&colors.ed_punct),
        fmt(&colors.ed_quoted),
    );
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let n = chars.len();
    let byte = |i: usize| chars.get(i).map(|c| c.0).unwrap_or(text.len());
    let mut i = 0;
    while i < n {
        let c = chars[i].1;
        let start = i;
        if c == '-' && i + 1 < n && chars[i + 1].1 == '-' {
            while i < n && chars[i].1 != '\n' {
                i += 1;
            }
            job.append(&text[byte(start)..byte(i)], 0.0, cm.clone());
        } else if c == '/' && i + 1 < n && chars[i + 1].1 == '*' {
            i += 2;
            while i + 1 < n && !(chars[i].1 == '*' && chars[i + 1].1 == '/') {
                i += 1;
            }
            i = (i + 2).min(n);
            job.append(&text[byte(start)..byte(i)], 0.0, cm.clone());
        } else if c == '\'' || c == '"' {
            i += 1;
            while i < n {
                if chars[i].1 == c {
                    if i + 1 < n && chars[i + 1].1 == c {
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                i += 1;
            }
            job.append(&text[byte(start)..byte(i)], 0.0, if c == '"' { quoted.clone() } else { st.clone() });
        } else if c.is_ascii_digit() {
            while i < n && (chars[i].1.is_ascii_digit() || chars[i].1 == '.') {
                i += 1;
            }
            job.append(&text[byte(start)..byte(i)], 0.0, num.clone());
        } else if is_word(c) {
            while i < n && is_word(chars[i].1) {
                i += 1;
            }
            let word = &text[byte(start)..byte(i)];
            let up = word.to_ascii_uppercase();
            let next_paren = text[byte(i)..].trim_start_matches(' ').starts_with('(');
            let f = if KEYWORDS.contains(&up.as_str()) {
                &kw
            } else if TYPES.contains(&word.to_ascii_lowercase().as_str()) {
                &ty
            } else if next_paren {
                &func
            } else {
                &ident
            };
            job.append(word, 0.0, f.clone());
        } else {
            i += 1;
            job.append(&text[byte(start)..byte(i)], 0.0, if c.is_whitespace() { ident.clone() } else { punct.clone() });
        }
    }
    job
}

/// Read-only, highlighted SQL (selectable for copying).
pub fn sql_view(ui: &mut Ui, colors: &Colors, sql: &str, max_height: f32) {
    let mut text = sql.to_string();
    let colors_c = colors.clone();
    let mut layouter = move |ui: &Ui, buf: &dyn egui::TextBuffer, wrap: f32| ui.painter().layout_job(highlight(&colors_c, buf.as_str(), 12.5, wrap));
    egui::Frame::new()
        .fill(c32(&colors.control_bg))
        .stroke(Stroke::new(1.0, c32(&colors.control_border)))
        .corner_radius(CornerRadius::same(5))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().max_height(max_height).auto_shrink([false, true]).show(ui, |ui| {
                ui.add(TextEdit::multiline(&mut text).code_editor().frame(egui::Frame::NONE).desired_width(f32::INFINITY).layouter(&mut layouter));
            });
        });
}

/// Names the autocompleter knows about.
#[derive(Default, Clone)]
pub struct CompletionSchema {
    pub columns: BTreeMap<String, Vec<String>>,
}

pub struct EditorOutput {
    /// Cursor position as a byte offset into the text.
    pub cursor_byte: usize,
}

fn byte_of_char(text: &str, ci: usize) -> usize {
    text.char_indices().nth(ci).map(|c| c.0).unwrap_or(text.len())
}

/// The statement under the cursor (⌘⏎), falling back to the whole text.
pub fn current_statement(sql: &str, cursor_byte: usize) -> String {
    let stmts = split_statements(sql);
    if stmts.is_empty() {
        return sql.trim().to_string();
    }
    for (i, (s, start, end)) in stmts.iter().enumerate() {
        if cursor_byte >= *start && cursor_byte <= *end + 1 {
            return s.clone();
        }
        if i + 1 < stmts.len() && cursor_byte < stmts[i + 1].1 {
            return s.clone();
        }
    }
    stmts.last().map(|s| s.0.clone()).unwrap_or_default()
}

fn suggestions(schema: &CompletionSchema, prefix: &str) -> Vec<(String, &'static str)> {
    let lower = prefix.to_lowercase();
    let mut out: Vec<(String, &'static str)> = vec![];
    if let Some((qual, part)) = lower.rsplit_once('.') {
        let q = qual.rsplit('.').next().unwrap_or(qual);
        for (t, cols) in &schema.columns {
            if t.to_lowercase() == q {
                for c in cols {
                    if c.to_lowercase().starts_with(part) {
                        out.push((c.clone(), "column"));
                    }
                }
            }
        }
        return out;
    }
    for t in schema.columns.keys() {
        if t.to_lowercase().starts_with(&lower) {
            out.push((t.clone(), "table"));
        }
    }
    let mut seen = std::collections::BTreeSet::new();
    for cols in schema.columns.values() {
        for c in cols {
            if c.to_lowercase().starts_with(&lower) && seen.insert(c.clone()) {
                out.push((c.clone(), "column"));
            }
        }
    }
    for k in KEYWORDS {
        if k.to_lowercase().starts_with(&lower) {
            out.push(((*k).to_string(), "keyword"));
        }
    }
    out.truncate(40);
    out
}

/// The multi-line SQL editor with highlighting and a completion popup.
pub fn editor(ui: &mut Ui, colors: &Colors, id: Id, text: &mut String, size: f32, schema: &CompletionSchema) -> EditorOutput {
    let popup_id = id.with("completion");
    let open: Option<(usize, Vec<(String, &'static str)>, usize)> = ui.memory(|m| m.data.get_temp(popup_id));
    // Keys that drive the popup have to be taken before the TextEdit sees them.
    let mut accept = false;
    let mut close = false;
    let mut sel_delta: i32 = 0;
    if open.as_ref().map(|o| !o.1.is_empty()).unwrap_or(false) {
        ui.input_mut(|i| {
            if i.consume_key(Modifiers::NONE, Key::ArrowDown) { sel_delta = 1 }
            if i.consume_key(Modifiers::NONE, Key::ArrowUp) { sel_delta = -1 }
            if i.consume_key(Modifiers::NONE, Key::Tab) || i.consume_key(Modifiers::NONE, Key::Enter) { accept = true }
            if i.consume_key(Modifiers::NONE, Key::Escape) { close = true }
        });
    }

    let colors_c = colors.clone();
    let mut layouter = move |ui: &Ui, buf: &dyn egui::TextBuffer, wrap: f32| ui.painter().layout_job(highlight(&colors_c, buf.as_str(), size, wrap));
    let out = TextEdit::multiline(text)
        .id(id)
        .code_editor()
        .frame(egui::Frame::NONE)
        .desired_width(f32::INFINITY)
        .desired_rows(8)
        .lock_focus(true)
        .layouter(&mut layouter)
        .show(ui);

    let cursor_char = out.cursor_range.map(|r| usize::from(r.primary.index)).unwrap_or(0);
    let cursor_byte = byte_of_char(text, cursor_char);

    // Apply an accepted completion.
    if accept {
        if let Some((_, items, sel)) = &open {
            if let Some((item, _)) = items.get(*sel) {
                let before = &text[..cursor_byte];
                let word_start = before.rfind(|c: char| !(is_word(c))).map(|i| i + before[i..].chars().next().unwrap().len_utf8()).unwrap_or(0);
                text.replace_range(word_start..cursor_byte, item);
                let new_char = text[..word_start].chars().count() + item.chars().count();
                let mut state = TextEditState::load(ui.ctx(), id).unwrap_or_default();
                state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(new_char))));
                state.store(ui.ctx(), id);
                ui.memory_mut(|m| m.data.remove::<(usize, Vec<(String, &'static str)>, usize)>(popup_id));
                ui.ctx().request_repaint();
                return EditorOutput { cursor_byte: word_start + item.len() };
            }
        }
    }

    // Recompute suggestions while typing.
    let mut next = open.clone();
    if close {
        next = None;
    } else if out.response.changed() && out.response.has_focus() {
        let before = &text[..cursor_byte];
        let word_start = before.rfind(|c: char| !(is_word(c) || c == '.')).map(|i| i + before[i..].chars().next().unwrap().len_utf8()).unwrap_or(0);
        let prefix = &before[word_start..];
        if prefix.is_empty() || prefix.chars().all(|c| c == '.') {
            next = None;
        } else {
            let items = suggestions(schema, prefix);
            let exact_only = items.len() == 1 && items[0].0.eq_ignore_ascii_case(prefix.rsplit('.').next().unwrap_or(prefix));
            next = if items.is_empty() || exact_only { None } else { Some((cursor_char, items, 0)) };
        }
    } else if !out.response.has_focus() {
        next = None;
    }
    if let Some((c, items, sel)) = &mut next {
        if sel_delta != 0 && !items.is_empty() {
            *sel = ((*sel as i32 + sel_delta).rem_euclid(items.len() as i32)) as usize;
        }
        // keep the popup only while the cursor stays where it was computed
        if *c != cursor_char && !out.response.changed() {
            next = None;
        }
    }
    match &next {
        Some(v) => {
            ui.memory_mut(|m| m.data.insert_temp(popup_id, v.clone()));
        }
        None => ui.memory_mut(|m| m.data.remove::<(usize, Vec<(String, &'static str)>, usize)>(popup_id)),
    }

    if let Some((_, items, sel)) = &next {
        let cursor_rect = out.galley.pos_from_cursor(out.cursor_range.map(|r| r.primary).unwrap_or(CCursor::new(0)));
        let pos = out.galley_pos + egui::vec2(cursor_rect.min.x, cursor_rect.max.y + 4.0);
        egui::Area::new(popup_id).order(Order::Foreground).fixed_pos(pos).show(ui.ctx(), |ui| {
            egui::Frame::new()
                .fill(c32(&colors.menu_bg))
                .stroke(Stroke::new(1.0, c32(&colors.control_border)))
                .corner_radius(CornerRadius::same(6))
                .inner_margin(egui::Margin::same(4))
                .show(ui, |ui| {
                    ui.set_min_width(240.0);
                    for (i, (name, kind)) in items.iter().take(8).enumerate() {
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 20.0), egui::Sense::hover());
                        if i == *sel {
                            ui.painter().rect_filled(rect, CornerRadius::same(4), c32(&colors.selection));
                        }
                        ui.painter().text(rect.left_center() + egui::vec2(8.0, 0.0), Align2::LEFT_CENTER, name, FontId::monospace(12.5), c32(&colors.text));
                        ui.painter().text(rect.right_center() - egui::vec2(8.0, 0.0), Align2::RIGHT_CENTER, *kind, FontId::proportional(10.0), c32(&colors.text_dim));
                    }
                });
        });
    }

    EditorOutput { cursor_byte }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_statement_under_cursor() {
        let sql = "SELECT 1;\nSELECT 2;\nSELECT 3";
        assert_eq!(current_statement(sql, 3), "SELECT 1");
        assert_eq!(current_statement(sql, 14), "SELECT 2");
        assert_eq!(current_statement(sql, 100), "SELECT 3");
        assert_eq!(current_statement("  ", 0), "");
    }

    #[test]
    fn suggests_columns_after_a_table_qualifier() {
        let mut s = CompletionSchema::default();
        s.columns.insert("users".into(), vec!["id".into(), "email".into()]);
        let got: Vec<_> = suggestions(&s, "users.e").into_iter().map(|x| x.0).collect();
        assert_eq!(got, vec!["email"]);
        let t: Vec<_> = suggestions(&s, "us").into_iter().map(|x| x.0).collect();
        assert!(t.contains(&"users".to_string()));
    }

    #[test]
    fn highlight_covers_the_whole_text() {
        let c = crate::theme::tableplus();
        let sql = "SELECT 'it''s', 42 -- note\nFROM t /* c */ WHERE \"A\" = now();";
        let job = highlight(&c, sql, 12.0, f32::INFINITY);
        assert_eq!(job.text, sql);
    }
}
