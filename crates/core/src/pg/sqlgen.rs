//! SQL text helpers: identifier/literal quoting, statement splitting, and
//! generation of the DML TablePlus emits when you commit grid edits.

use crate::error::{AppError, AppResult};
use crate::model::{CellValue, FilterSpec, RowChange, RowIdentity, SortSpec};
use std::collections::BTreeMap;

pub fn quote_ident(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

pub fn qualified(schema: &str, name: &str) -> String {
    format!("{}.{}", quote_ident(schema), quote_ident(name))
}

/// Standard-conforming string literal ('' doubling). Backslashes are literal
/// because standard_conforming_strings has defaulted to on since 9.1.
pub fn quote_literal(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

pub fn literal_opt(v: &Option<String>) -> String {
    match v {
        None => "NULL".into(),
        Some(s) => quote_literal(s),
    }
}

pub fn cell_sql(v: &CellValue) -> String {
    match v {
        CellValue::Null => "NULL".into(),
        CellValue::Default => "DEFAULT".into(),
        CellValue::Text { value } => quote_literal(value),
        CellValue::Expr { value } => value.clone(),
    }
}

/// Split a script into statements, honouring quotes, dollar quoting and comments.
/// Returns (statement_text, byte_start, byte_end).
pub fn split_statements(sql: &str) -> Vec<(String, usize, usize)> {
    let bytes = sql.as_bytes();
    let n = bytes.len();
    let mut out = Vec::new();
    let mut i = 0usize;
    let mut start = 0usize;
    let mut block_depth = 0usize;

    while i < n {
        let c = bytes[i];
        // line comment
        if c == b'-' && i + 1 < n && bytes[i + 1] == b'-' {
            while i < n && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        // block comment (nestable)
        if c == b'/' && i + 1 < n && bytes[i + 1] == b'*' {
            block_depth += 1;
            i += 2;
            while i < n && block_depth > 0 {
                if bytes[i] == b'/' && i + 1 < n && bytes[i + 1] == b'*' {
                    block_depth += 1;
                    i += 2;
                } else if bytes[i] == b'*' && i + 1 < n && bytes[i + 1] == b'/' {
                    block_depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            continue;
        }
        // single-quoted (and E'' with backslash escapes)
        if c == b'\'' {
            let escaped = i > 0 && (bytes[i - 1] == b'E' || bytes[i - 1] == b'e')
                && (i < 2 || !bytes[i - 2].is_ascii_alphanumeric());
            i += 1;
            while i < n {
                if escaped && bytes[i] == b'\\' {
                    i += 2;
                    continue;
                }
                if bytes[i] == b'\'' {
                    if i + 1 < n && bytes[i + 1] == b'\'' {
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }
        // double-quoted identifier
        if c == b'"' {
            i += 1;
            while i < n {
                if bytes[i] == b'"' {
                    if i + 1 < n && bytes[i + 1] == b'"' {
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }
        // dollar quoting $tag$ ... $tag$
        if c == b'$' {
            let mut j = i + 1;
            while j < n && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                j += 1;
            }
            if j < n && bytes[j] == b'$' && (j == i + 1 || !bytes[i + 1].is_ascii_digit()) {
                let tag = &sql[i..=j];
                if let Some(pos) = sql[j + 1..].find(tag) {
                    i = j + 1 + pos + tag.len();
                } else {
                    i = n;
                }
                continue;
            }
        }
        if c == b';' {
            let text = sql[start..i].trim();
            if !text.is_empty() {
                out.push((text.to_string(), start, i + 1));
            }
            i += 1;
            start = i;
            continue;
        }
        i += 1;
    }
    let text = sql[start..].trim();
    if !text.is_empty() {
        out.push((text.to_string(), start, n));
    }
    out
}

/// First keyword of a statement, uppercased (SELECT, INSERT, ...).
pub fn leading_keyword(sql: &str) -> String {
    let mut s = sql.trim_start();
    loop {
        if s.starts_with("--") {
            s = s.split_once('\n').map(|x| x.1).unwrap_or("").trim_start();
        } else if s.starts_with("/*") {
            s = s.split_once("*/").map(|x| x.1).unwrap_or("").trim_start();
        } else {
            break;
        }
    }
    s.split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .next()
        .unwrap_or("")
        .to_ascii_uppercase()
}

pub fn build_where(filters: &[FilterSpec], quick: Option<&str>, text_columns: &[String]) -> AppResult<String> {
    let mut parts: Vec<String> = Vec::new();
    for f in filters.iter().filter(|f| f.enabled) {
        let col = quote_ident(&f.column);
        let v = f.value.clone().unwrap_or_default();
        let lit = quote_literal(&v);
        let part = match f.op.as_str() {
            "eq" => format!("{col} = {lit}"),
            "neq" => format!("{col} <> {lit}"),
            "gt" => format!("{col} > {lit}"),
            "gte" => format!("{col} >= {lit}"),
            "lt" => format!("{col} < {lit}"),
            "lte" => format!("{col} <= {lit}"),
            "contains" => format!("{col}::text ILIKE {}", quote_literal(&format!("%{}%", like_escape(&v)))),
            "not_contains" => format!("{col}::text NOT ILIKE {}", quote_literal(&format!("%{}%", like_escape(&v)))),
            "starts_with" => format!("{col}::text ILIKE {}", quote_literal(&format!("{}%", like_escape(&v)))),
            "ends_with" => format!("{col}::text ILIKE {}", quote_literal(&format!("%{}", like_escape(&v)))),
            "like" => format!("{col}::text LIKE {lit}"),
            "not_like" => format!("{col}::text NOT LIKE {lit}"),
            "ilike" => format!("{col}::text ILIKE {lit}"),
            "in" | "not_in" => {
                let items: Vec<String> = v
                    .split(',')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .map(quote_literal)
                    .collect();
                if items.is_empty() {
                    continue;
                }
                let neg = if f.op == "not_in" { "NOT " } else { "" };
                format!("{col} {neg}IN ({})", items.join(", "))
            }
            "is_null" => format!("{col} IS NULL"),
            "is_not_null" => format!("{col} IS NOT NULL"),
            "is_true" => format!("{col} IS TRUE"),
            "is_false" => format!("{col} IS FALSE"),
            "between" => format!(
                "{col} BETWEEN {lit} AND {}",
                quote_literal(f.value2.as_deref().unwrap_or(""))
            ),
            "raw" => {
                if v.trim().is_empty() {
                    continue;
                }
                format!("({v})")
            }
            other => return Err(AppError::msg(format!("unknown filter operator {other}"))),
        };
        parts.push(part);
    }
    if let Some(q) = quick.map(|s| s.trim()).filter(|s| !s.is_empty()) {
        if !text_columns.is_empty() {
            let pat = quote_literal(&format!("%{}%", like_escape(q)));
            let ors: Vec<String> = text_columns
                .iter()
                .map(|c| format!("{}::text ILIKE {pat}", quote_ident(c)))
                .collect();
            parts.push(format!("({})", ors.join(" OR ")));
        }
    }
    Ok(parts.join(" AND "))
}

fn like_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

pub fn build_order(sort: &[SortSpec]) -> String {
    let parts: Vec<String> = sort
        .iter()
        .map(|s| {
            let dir = if s.direction.eq_ignore_ascii_case("desc") { "DESC" } else { "ASC" };
            format!("{} {dir}", quote_ident(&s.column))
        })
        .collect();
    parts.join(", ")
}

fn identity_where(schema: &str, table: &str, id: &RowIdentity, pk: &[String]) -> AppResult<String> {
    if !pk.is_empty() && pk.iter().all(|k| id.key_values.contains_key(k)) {
        let parts: Vec<String> = pk
            .iter()
            .map(|k| match id.key_values.get(k).and_then(|v| v.clone()) {
                Some(v) => format!("{} = {}", quote_ident(k), quote_literal(&v)),
                None => format!("{} IS NULL", quote_ident(k)),
            })
            .collect();
        return Ok(parts.join(" AND "));
    }
    if let Some(ctid) = &id.ctid {
        // TablePlus: "(tableoid, ctid) = (SELECT tableoid, ctid FROM ...)" row identity fallback
        return Ok(format!(
            "ctid = {}::tid AND tableoid = {}::regclass",
            quote_literal(ctid),
            quote_literal(&qualified(schema, table))
        ));
    }
    if !id.key_values.is_empty() {
        // Last resort: match on every original value (what TablePlus does first).
        let parts: Vec<String> = id
            .key_values
            .iter()
            .map(|(k, v)| match v {
                Some(v) => format!("{} = {}", quote_ident(k), quote_literal(v)),
                None => format!("{} IS NULL", quote_ident(k)),
            })
            .collect();
        return Ok(parts.join(" AND "));
    }
    Err(AppError::msg("row has no primary key or physical identity; cannot build WHERE clause"))
}

pub fn generate_change_sql(schema: &str, table: &str, pk: &[String], changes: &[RowChange]) -> AppResult<Vec<String>> {
    let rel = qualified(schema, table);
    let mut out = Vec::new();
    for ch in changes {
        match ch {
            RowChange::Insert { values } => {
                let filtered: BTreeMap<&String, &CellValue> = values.iter().collect();
                if filtered.is_empty() {
                    out.push(format!("INSERT INTO {rel} DEFAULT VALUES"));
                } else {
                    let cols: Vec<String> = filtered.keys().map(|k| quote_ident(k)).collect();
                    let vals: Vec<String> = filtered.values().map(|v| cell_sql(v)).collect();
                    out.push(format!(
                        "INSERT INTO {rel} ({}) VALUES ({})",
                        cols.join(", "),
                        vals.join(", ")
                    ));
                }
            }
            RowChange::Update { identity, values } => {
                if values.is_empty() {
                    continue;
                }
                let sets: Vec<String> = values
                    .iter()
                    .map(|(k, v)| format!("{} = {}", quote_ident(k), cell_sql(v)))
                    .collect();
                let where_ = identity_where(schema, table, identity, pk)?;
                out.push(format!("UPDATE {rel} SET {} WHERE {where_}", sets.join(", ")));
            }
            RowChange::Delete { identity } => {
                let where_ = identity_where(schema, table, identity, pk)?;
                out.push(format!("DELETE FROM {rel} WHERE {where_}"));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_basic() {
        let v = split_statements("select 1; select 'a;b'; -- c;\nselect $$x;y$$;");
        assert_eq!(v.len(), 3);
        assert_eq!(v[1].0, "select 'a;b'");
        assert!(v[2].0.ends_with("select $$x;y$$"));
    }

    #[test]
    fn keyword() {
        assert_eq!(leading_keyword("  -- hi\n SELECT 1"), "SELECT");
        assert_eq!(leading_keyword("/* x */ with a as (select 1) select * from a"), "WITH");
    }
}
