//! Export grid rows to CSV, JSON, SQL INSERTs or a Markdown table.

use crate::error::{AppError, AppResult};
use crate::pg::sqlgen::{quote_ident, quote_literal};

pub struct ExportRequest<'a> {
    pub path: &'a str,
    pub format: &'a str, // csv | json | sql | markdown
    pub columns: &'a [String],
    pub rows: &'a [Vec<Option<String>>],
    pub table: Option<&'a str>,
    pub header: bool,
}

pub fn export_rows(req: &ExportRequest) -> AppResult<usize> {
    let mut f = std::fs::File::create(req.path)?;
    use std::io::Write;
    match req.format {
        "csv" => {
            let mut w = csv::Writer::from_writer(&mut f);
            if req.header {
                w.write_record(req.columns).map_err(|e| AppError::msg(e.to_string()))?;
            }
            for r in req.rows {
                w.write_record(r.iter().map(|c| c.as_deref().unwrap_or(""))).map_err(|e| AppError::msg(e.to_string()))?;
            }
            w.flush()?;
        }
        "json" => {
            let objs: Vec<serde_json::Map<String, serde_json::Value>> = req
                .rows
                .iter()
                .map(|r| {
                    req.columns
                        .iter()
                        .zip(r.iter())
                        .map(|(c, v)| (c.clone(), v.clone().map(serde_json::Value::String).unwrap_or(serde_json::Value::Null)))
                        .collect()
                })
                .collect();
            f.write_all(serde_json::to_string_pretty(&objs)?.as_bytes())?;
        }
        "sql" => {
            let table = req.table.unwrap_or("table");
            for r in req.rows {
                let vals: Vec<String> = r.iter().map(|v| match v { None => "NULL".into(), Some(s) => quote_literal(s) }).collect();
                writeln!(
                    f,
                    "INSERT INTO {} ({}) VALUES ({});",
                    table,
                    req.columns.iter().map(|c| quote_ident(c)).collect::<Vec<_>>().join(", "),
                    vals.join(", ")
                )?;
            }
        }
        "markdown" => {
            writeln!(f, "| {} |", req.columns.join(" | "))?;
            writeln!(f, "|{}|", req.columns.iter().map(|_| " --- ").collect::<Vec<_>>().join("|"))?;
            for r in req.rows {
                writeln!(f, "| {} |", r.iter().map(|v| v.as_deref().unwrap_or("NULL").replace('|', "\\|")).collect::<Vec<_>>().join(" | "))?;
            }
        }
        other => return Err(AppError::msg(format!("unknown export format {other}"))),
    }
    Ok(req.rows.len())
}
