//! DDL generation for the Structure tab and Create Table: each op maps to one statement.

use crate::pg::sqlgen::{qualified, quote_ident, quote_literal};

/// Structure-tab DDL. Each op maps to one ALTER/COMMENT statement.
#[derive(Debug, Clone)]
pub enum DdlOp {
    AddColumn { name: String, data_type: String, nullable: bool, default: Option<String>, comment: Option<String> },
    DropColumn { name: String },
    RenameColumn { name: String, new_name: String },
    ChangeType { name: String, data_type: String, using: Option<String> },
    SetNullable { name: String, nullable: bool },
    SetDefault { name: String, default: Option<String> },
    SetComment { name: String, comment: Option<String> },
    SetTableComment { comment: Option<String> },
    RenameTable { new_name: String },
    AddIndex { name: String, columns: Vec<String>, unique: bool, method: Option<String>, condition: Option<String> },
    DropIndex { name: String },
    AddPrimaryKey { columns: Vec<String> },
    DropConstraint { name: String },
    AddForeignKey { name: Option<String>, columns: Vec<String>, ref_schema: String, ref_table: String, ref_columns: Vec<String>, on_update: Option<String>, on_delete: Option<String> },
    Truncate { cascade: bool, restart_identity: bool },
    DropTable { cascade: bool },
    CreateTable { name: String, columns: Vec<NewColumn> },
    Raw { sql: String },
}

#[derive(Debug, Clone)]
pub struct NewColumn {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub default: Option<String>,
    pub primary_key: bool,
}

pub fn ddl_sql(schema: &str, table: &str, ops: &[DdlOp]) -> Vec<String> {
    let rel = qualified(schema, table);
    let mut out = Vec::new();
    for op in ops {
        match op {
            DdlOp::AddColumn { name, data_type, nullable, default, comment } => {
                let mut s = format!("ALTER TABLE {rel} ADD COLUMN {} {data_type}", quote_ident(name));
                if let Some(d) = default.as_deref().filter(|d| !d.trim().is_empty()) {
                    s.push_str(&format!(" DEFAULT {d}"));
                }
                if !nullable {
                    s.push_str(" NOT NULL");
                }
                out.push(s);
                if let Some(c) = comment.as_deref().filter(|c| !c.is_empty()) {
                    out.push(format!("COMMENT ON COLUMN {rel}.{} IS {}", quote_ident(name), quote_literal(c)));
                }
            }
            DdlOp::DropColumn { name } => out.push(format!("ALTER TABLE {rel} DROP COLUMN {}", quote_ident(name))),
            DdlOp::RenameColumn { name, new_name } => {
                out.push(format!("ALTER TABLE {rel} RENAME COLUMN {} TO {}", quote_ident(name), quote_ident(new_name)))
            }
            DdlOp::ChangeType { name, data_type, using } => {
                let mut s = format!("ALTER TABLE {rel} ALTER COLUMN {} TYPE {data_type}", quote_ident(name));
                if let Some(u) = using.as_deref().filter(|u| !u.trim().is_empty()) {
                    s.push_str(&format!(" USING {u}"));
                }
                out.push(s)
            }
            DdlOp::SetNullable { name, nullable } => out.push(format!(
                "ALTER TABLE {rel} ALTER COLUMN {} {} NOT NULL",
                quote_ident(name),
                if *nullable { "DROP" } else { "SET" }
            )),
            DdlOp::SetDefault { name, default } => match default.as_deref().filter(|d| !d.trim().is_empty()) {
                Some(d) => out.push(format!("ALTER TABLE {rel} ALTER COLUMN {} SET DEFAULT {d}", quote_ident(name))),
                None => out.push(format!("ALTER TABLE {rel} ALTER COLUMN {} DROP DEFAULT", quote_ident(name))),
            },
            DdlOp::SetComment { name, comment } => out.push(format!(
                "COMMENT ON COLUMN {rel}.{} IS {}",
                quote_ident(name),
                comment.as_deref().filter(|c| !c.is_empty()).map(quote_literal).unwrap_or_else(|| "NULL".into())
            )),
            DdlOp::SetTableComment { comment } => out.push(format!(
                "COMMENT ON TABLE {rel} IS {}",
                comment.as_deref().filter(|c| !c.is_empty()).map(quote_literal).unwrap_or_else(|| "NULL".into())
            )),
            DdlOp::RenameTable { new_name } => out.push(format!("ALTER TABLE {rel} RENAME TO {}", quote_ident(new_name))),
            DdlOp::AddIndex { name, columns, unique, method, condition } => {
                let mut s = format!(
                    "CREATE {}INDEX {} ON {rel}{} ({})",
                    if *unique { "UNIQUE " } else { "" },
                    quote_ident(name),
                    method.as_deref().filter(|m| !m.is_empty() && !m.eq_ignore_ascii_case("btree")).map(|m| format!(" USING {m}")).unwrap_or_default(),
                    columns.iter().map(|c| if c.contains('(') || c.contains(' ') { c.clone() } else { quote_ident(c) }).collect::<Vec<_>>().join(", ")
                );
                if let Some(w) = condition.as_deref().filter(|c| !c.trim().is_empty()) {
                    s.push_str(&format!(" WHERE {w}"));
                }
                out.push(s)
            }
            DdlOp::DropIndex { name } => out.push(format!("DROP INDEX {}", qualified(schema, name))),
            DdlOp::AddPrimaryKey { columns } => out.push(format!(
                "ALTER TABLE {rel} ADD PRIMARY KEY ({})",
                columns.iter().map(|c| quote_ident(c)).collect::<Vec<_>>().join(", ")
            )),
            DdlOp::DropConstraint { name } => out.push(format!("ALTER TABLE {rel} DROP CONSTRAINT {}", quote_ident(name))),
            DdlOp::AddForeignKey { name, columns, ref_schema, ref_table, ref_columns, on_update, on_delete } => {
                let mut s = format!("ALTER TABLE {rel} ADD ");
                if let Some(n) = name.as_deref().filter(|n| !n.is_empty()) {
                    s.push_str(&format!("CONSTRAINT {} ", quote_ident(n)));
                }
                s.push_str(&format!(
                    "FOREIGN KEY ({}) REFERENCES {} ({})",
                    columns.iter().map(|c| quote_ident(c)).collect::<Vec<_>>().join(", "),
                    qualified(ref_schema, ref_table),
                    ref_columns.iter().map(|c| quote_ident(c)).collect::<Vec<_>>().join(", ")
                ));
                if let Some(u) = on_update.as_deref().filter(|u| !u.is_empty()) {
                    s.push_str(&format!(" ON UPDATE {u}"));
                }
                if let Some(d) = on_delete.as_deref().filter(|d| !d.is_empty()) {
                    s.push_str(&format!(" ON DELETE {d}"));
                }
                out.push(s)
            }
            DdlOp::Truncate { cascade, restart_identity } => out.push(format!(
                "TRUNCATE TABLE {rel}{}{}",
                if *restart_identity { " RESTART IDENTITY" } else { "" },
                if *cascade { " CASCADE" } else { "" }
            )),
            DdlOp::DropTable { cascade } => out.push(format!("DROP TABLE {rel}{}", if *cascade { " CASCADE" } else { "" })),
            DdlOp::CreateTable { name, columns } => {
                let mut lines: Vec<String> = columns
                    .iter()
                    .map(|c| {
                        let mut l = format!("    {} {}", quote_ident(&c.name), c.data_type);
                        if let Some(d) = c.default.as_deref().filter(|d| !d.trim().is_empty()) {
                            l.push_str(&format!(" DEFAULT {d}"));
                        }
                        if !c.nullable {
                            l.push_str(" NOT NULL");
                        }
                        l
                    })
                    .collect();
                let pk: Vec<String> = columns.iter().filter(|c| c.primary_key).map(|c| quote_ident(&c.name)).collect();
                if !pk.is_empty() {
                    lines.push(format!("    PRIMARY KEY ({})", pk.join(", ")));
                }
                out.push(format!("CREATE TABLE {} (\n{}\n)", qualified(schema, name), lines.join(",\n")));
            }
            DdlOp::Raw { sql } => out.push(sql.clone()),
        }
    }
    out
}

