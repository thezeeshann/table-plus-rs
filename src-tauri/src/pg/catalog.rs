//! Catalog introspection. The queries are ports of the ones TablePlus ships in
//! PostgreSQL.framework (observed as string literals), adjusted to read from
//! pg_catalog directly so types are stable across versions.

use crate::error::AppResult;
use crate::model::*;
use crate::pg::sqlgen::{qualified, quote_ident};
use std::collections::HashMap;
use tokio_postgres::Client;

const REGCLASS: &str = "(quote_ident($1::text) || '.' || quote_ident($2::text))::regclass";

pub async fn list_databases(client: &Client) -> AppResult<Vec<String>> {
    let rows = client
        .query("SELECT datname::text FROM pg_database WHERE NOT datistemplate AND datallowconn ORDER BY 1", &[])
        .await?;
    Ok(rows.iter().map(|r| r.get(0)).collect())
}

pub async fn list_schemas(client: &Client, show_system: bool) -> AppResult<Vec<String>> {
    let sql = if show_system {
        "SELECT nspname::text FROM pg_catalog.pg_namespace WHERE nspname NOT LIKE 'pg_temp_%' AND nspname NOT LIKE 'pg_toast_temp_%' ORDER BY (nspname = 'public') DESC, 1"
    } else {
        "SELECT nspname::text FROM pg_catalog.pg_namespace WHERE nspname NOT IN ('pg_catalog','information_schema','pg_toast') AND nspname NOT LIKE 'pg_temp_%' AND nspname NOT LIKE 'pg_toast_temp_%' ORDER BY (nspname = 'public') DESC, 1"
    };
    let rows = client.query(sql, &[]).await?;
    Ok(rows.iter().map(|r| r.get(0)).collect())
}

fn relkind_name(k: &str) -> &'static str {
    match k {
        "r" => "table",
        "p" => "partitioned",
        "v" => "view",
        "m" => "matview",
        "f" => "foreign",
        _ => "table",
    }
}

pub async fn list_items(client: &Client, schema: &str, version_num: i32) -> AppResult<SchemaListing> {
    let rows = client
        .query(
            "SELECT c.relname::text, c.relkind::text, obj_description(c.oid, 'pg_class'), c.reltuples::bigint, \
             pg_total_relation_size(c.oid), pg_table_size(c.oid), pg_indexes_size(c.oid), pg_get_userbyid(c.relowner)::text \
             FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
             WHERE n.nspname = $1 AND c.relkind IN ('r','v','m','f','p') ORDER BY c.relname",
            &[&schema],
        )
        .await?;
    let items = rows
        .iter()
        .map(|r| {
            let kind: String = r.get(1);
            let est: i64 = r.get(3);
            SchemaItem {
                schema: schema.to_string(),
                name: r.get(0),
                kind: relkind_name(&kind).to_string(),
                comment: r.get(2),
                estimated_rows: if est < 0 { None } else { Some(est) },
                total_size: r.get(4),
                data_size: r.get(5),
                index_size: r.get(6),
                owner: r.get(7),
            }
        })
        .collect();

    let fn_sql = if version_num >= 110000 {
        "SELECT p.oid, p.proname::text, pg_get_function_identity_arguments(p.oid), \
         CASE WHEN p.prokind = 'a' THEN 'aggregate' WHEN p.prokind = 'w' THEN 'window' WHEN p.prokind = 'p' THEN 'procedure' \
         WHEN p.prorettype = 'pg_catalog.trigger'::pg_catalog.regtype THEN 'trigger' ELSE 'function' END, \
         pg_get_userbyid(p.proowner)::text, obj_description(p.oid, 'pg_proc') \
         FROM pg_catalog.pg_proc p JOIN pg_catalog.pg_namespace n ON n.oid = p.pronamespace WHERE n.nspname = $1 ORDER BY p.proname"
    } else {
        "SELECT p.oid, p.proname::text, pg_get_function_identity_arguments(p.oid), \
         CASE WHEN p.proisagg THEN 'aggregate' WHEN p.proiswindow THEN 'window' \
         WHEN p.prorettype = 'pg_catalog.trigger'::pg_catalog.regtype THEN 'trigger' ELSE 'function' END, \
         pg_get_userbyid(p.proowner)::text, obj_description(p.oid, 'pg_proc') \
         FROM pg_catalog.pg_proc p JOIN pg_catalog.pg_namespace n ON n.oid = p.pronamespace WHERE n.nspname = $1 ORDER BY p.proname"
    };
    let frows = client.query(fn_sql, &[&schema]).await?;
    let functions = frows
        .iter()
        .map(|r| FunctionItem {
            oid: r.get::<_, u32>(0),
            schema: schema.to_string(),
            name: r.get(1),
            args: r.get(2),
            kind: r.get(3),
            owner: r.get(4),
            comment: r.get(5),
        })
        .collect();

    let sequences = if version_num >= 100000 {
        let srows = client
            .query(
                "SELECT s.sequencename::text, s.start_value::text, s.min_value::text, s.max_value::text, s.increment_by::text, \
                 s.last_value::text, s.cache_size::text, \
                 obj_description((quote_ident(s.schemaname) || '.' || quote_ident(s.sequencename))::regclass, 'pg_class') \
                 FROM pg_sequences s WHERE s.schemaname = $1 ORDER BY 1",
                &[&schema],
            )
            .await?;
        srows
            .iter()
            .map(|r| SequenceItem {
                schema: schema.to_string(),
                name: r.get(0),
                start_value: r.get(1),
                min_value: r.get(2),
                max_value: r.get(3),
                increment_by: r.get(4),
                last_value: r.get(5),
                cache_size: r.get(6),
                comment: r.get(7),
            })
            .collect()
    } else {
        Vec::new()
    };

    Ok(SchemaListing { items, functions, sequences })
}

async fn enum_values(client: &Client) -> AppResult<HashMap<u32, Vec<String>>> {
    let rows = client
        .query(
            "SELECT t.oid, e.enumlabel::text FROM pg_type t JOIN pg_enum e ON e.enumtypid = t.oid ORDER BY t.oid, e.enumsortorder",
            &[],
        )
        .await?;
    let mut map: HashMap<u32, Vec<String>> = HashMap::new();
    for r in rows {
        map.entry(r.get::<_, u32>(0)).or_default().push(r.get(1));
    }
    Ok(map)
}

fn fk_action(code: &str) -> Option<String> {
    match code {
        "r" => Some("RESTRICT".into()),
        "c" => Some("CASCADE".into()),
        "n" => Some("SET NULL".into()),
        "d" => Some("SET DEFAULT".into()),
        "a" => Some("NO ACTION".into()),
        _ => None,
    }
}

pub async fn primary_key(client: &Client, schema: &str, table: &str) -> AppResult<Vec<String>> {
    let sql = format!(
        "SELECT a.attname::text FROM pg_index i JOIN pg_attribute a ON a.attrelid = i.indrelid AND a.attnum = ANY(i.indkey) \
         WHERE i.indrelid = {REGCLASS} AND i.indisprimary ORDER BY array_position(i.indkey, a.attnum)"
    );
    let rows = client.query(&sql, &[&schema, &table]).await?;
    Ok(rows.iter().map(|r| r.get(0)).collect())
}

pub async fn foreign_keys(client: &Client, schema: &str, table: &str) -> AppResult<Vec<ForeignKeyInfo>> {
    let sql = format!(
        "SELECT c.conname::text, \
         (SELECT array_agg(a.attname::text ORDER BY k.ord) FROM unnest(c.conkey) WITH ORDINALITY k(attnum, ord) \
            JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attnum = k.attnum), \
         pn.nspname::text, pc.relname::text, \
         (SELECT array_agg(a.attname::text ORDER BY k.ord) FROM unnest(c.confkey) WITH ORDINALITY k(attnum, ord) \
            JOIN pg_attribute a ON a.attrelid = c.confrelid AND a.attnum = k.attnum), \
         c.confupdtype::text, c.confdeltype::text \
         FROM pg_constraint c JOIN pg_class pc ON pc.oid = c.confrelid JOIN pg_namespace pn ON pn.oid = pc.relnamespace \
         WHERE c.contype = 'f' AND c.conrelid = {REGCLASS} ORDER BY c.conname"
    );
    let rows = client.query(&sql, &[&schema, &table]).await?;
    Ok(rows
        .iter()
        .map(|r| ForeignKeyInfo {
            constraint_name: r.get(0),
            child_columns: r.get::<_, Option<Vec<String>>>(1).unwrap_or_default(),
            parent_schema: r.get(2),
            parent_table: r.get(3),
            parent_columns: r.get::<_, Option<Vec<String>>>(4).unwrap_or_default(),
            on_update: fk_action(&r.get::<_, String>(5)),
            on_delete: fk_action(&r.get::<_, String>(6)),
        })
        .collect())
}

pub async fn indexes(client: &Client, schema: &str, table: &str, version_num: i32) -> AppResult<Vec<IndexInfo>> {
    let key_cols = if version_num >= 110000 {
        "(SELECT string_agg(pg_get_indexdef(i.indexrelid, k, true), ', ' ORDER BY k) FROM generate_series(1, i.indnkeyatts) k)"
    } else {
        "(SELECT string_agg(pg_get_indexdef(i.indexrelid, k, true), ', ' ORDER BY k) FROM generate_series(1, i.indnatts) k)"
    };
    let include = if version_num >= 110000 {
        "(SELECT string_agg(pg_get_indexdef(i.indexrelid, k, true), ', ' ORDER BY k) FROM generate_series(i.indnkeyatts + 1, i.indnatts) k)"
    } else {
        "NULL::text"
    };
    let sql = format!(
        "SELECT ix.relname::text, upper(am.amname)::text, i.indisunique, i.indisprimary, pg_get_indexdef(i.indexrelid), \
         {key_cols}, pg_get_expr(i.indpred, i.indrelid), {include}, obj_description(i.indexrelid, 'pg_class'), pg_relation_size(i.indexrelid) \
         FROM pg_index i JOIN pg_class ix ON ix.oid = i.indexrelid JOIN pg_am am ON am.oid = ix.relam \
         WHERE i.indrelid = {REGCLASS} ORDER BY i.indisprimary DESC, ix.relname"
    );
    let rows = client.query(&sql, &[&schema, &table]).await?;
    Ok(rows
        .iter()
        .map(|r| {
            let cols: Option<String> = r.get(5);
            IndexInfo {
                name: r.get(0),
                algorithm: r.get(1),
                is_unique: r.get(2),
                is_primary: r.get(3),
                definition: r.get(4),
                columns: cols
                    .unwrap_or_default()
                    .split(", ")
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect(),
                condition: r.get(6),
                include: r.get(7),
                comment: r.get(8),
                size: r.get(9),
            }
        })
        .collect())
}

pub async fn table_structure(client: &Client, schema: &str, table: &str, kind: &str, version_num: i32) -> AppResult<TableStructure> {
    let identity_col = if version_num >= 100000 { "a.attidentity::text" } else { "''::text" };
    let generated_col = if version_num >= 120000 { "a.attgenerated::text" } else { "''::text" };
    let sql = format!(
        "SELECT a.attnum::int, a.attname::text, t.typname::text, format_type(a.atttypid, a.atttypmod), NOT a.attnotnull, \
         pg_get_expr(d.adbin, d.adrelid), col_description(a.attrelid, a.attnum), {identity_col}, {generated_col}, t.typtype::text, t.oid, \
         information_schema._pg_char_max_length(a.atttypid, a.atttypmod)::int, \
         information_schema._pg_numeric_precision(a.atttypid, a.atttypmod)::int, \
         information_schema._pg_numeric_scale(a.atttypid, a.atttypmod)::int \
         FROM pg_attribute a JOIN pg_class c ON c.oid = a.attrelid JOIN pg_namespace n ON n.oid = c.relnamespace \
         JOIN pg_type t ON t.oid = a.atttypid LEFT JOIN pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum \
         WHERE n.nspname = $1 AND c.relname = $2 AND a.attnum > 0 AND NOT a.attisdropped ORDER BY a.attnum"
    );
    let rows = client.query(&sql, &[&schema, &table]).await?;
    let enums = enum_values(client).await?;
    let pk = primary_key(client, schema, table).await.unwrap_or_default();
    let fks = foreign_keys(client, schema, table).await.unwrap_or_default();
    let idx = if kind == "table" || kind == "partitioned" || kind == "matview" {
        indexes(client, schema, table, version_num).await.unwrap_or_default()
    } else {
        Vec::new()
    };

    let columns = rows
        .iter()
        .map(|r| {
            let name: String = r.get(1);
            let typtype: String = r.get(9);
            let toid: u32 = r.get(10);
            let identity: String = r.get(7);
            let generated: String = r.get(8);
            let gen_expr: Option<String> = r.get(5);
            ColumnInfo {
                position: r.get(0),
                is_primary_key: pk.contains(&name),
                foreign_key: fks.iter().find(|f| f.child_columns.contains(&name)).cloned(),
                data_type: r.get(2),
                format_type: r.get(3),
                nullable: r.get(4),
                default: if generated.is_empty() { gen_expr.clone() } else { None },
                comment: r.get(6),
                is_identity: !identity.is_empty(),
                is_generated: !generated.is_empty(),
                generation_expression: if generated.is_empty() { None } else { gen_expr },
                character_maximum_length: r.get(11),
                numeric_precision: r.get(12),
                numeric_scale: r.get(13),
                enum_values: if typtype == "e" { enums.get(&toid).cloned() } else { None },
                name,
            }
        })
        .collect();

    let checks_sql = format!(
        "SELECT conname::text, pg_get_constraintdef(oid) FROM pg_constraint WHERE contype = 'c' AND conrelid = {REGCLASS} ORDER BY conname"
    );
    let checks = client
        .query(&checks_sql, &[&schema, &table])
        .await
        .map(|rows| rows.iter().map(|r| CheckConstraint { name: r.get(0), condition: r.get(1) }).collect())
        .unwrap_or_default();

    let trig_sql = format!(
        "SELECT t.tgname::text, pg_get_triggerdef(t.oid) FROM pg_trigger t WHERE t.tgrelid = {REGCLASS} AND NOT t.tgisinternal ORDER BY t.tgname"
    );
    let triggers = client
        .query(&trig_sql, &[&schema, &table])
        .await
        .map(|rows| {
            rows.iter()
                .map(|r| {
                    let def: String = r.get(1);
                    let up = def.to_ascii_uppercase();
                    let timing = ["BEFORE", "AFTER", "INSTEAD OF"]
                        .iter()
                        .find(|t| up.contains(&format!(" {} ", t)))
                        .map(|s| s.to_string())
                        .unwrap_or_default();
                    let event = ["INSERT", "UPDATE", "DELETE", "TRUNCATE"]
                        .iter()
                        .filter(|e| up.contains(&format!(" {} ", e)) || up.contains(&format!(" {} OR", e)))
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>()
                        .join(" OR ");
                    TriggerInfo { name: r.get(0), event, timing, statement: def }
                })
                .collect()
        })
        .unwrap_or_default();

    let comment_sql = format!("SELECT obj_description({REGCLASS}, 'pg_class')");
    let comment: Option<String> = client
        .query_one(&comment_sql, &[&schema, &table])
        .await
        .ok()
        .and_then(|r| r.get(0));

    Ok(TableStructure {
        schema: schema.to_string(),
        table: table.to_string(),
        kind: kind.to_string(),
        columns,
        primary_key: pk,
        foreign_keys: fks,
        indexes: idx,
        checks,
        triggers,
        comment,
    })
}

pub async fn estimated_rows(client: &Client, schema: &str, table: &str) -> AppResult<Option<i64>> {
    let sql = format!("SELECT reltuples::bigint FROM pg_class WHERE oid = {REGCLASS}");
    let row = client.query_opt(&sql, &[&schema, &table]).await?;
    Ok(row.map(|r| r.get::<_, i64>(0)).filter(|n| *n >= 0))
}

/// CREATE statement for the object, as TablePlus' "Definition" tab shows.
pub async fn object_ddl(client: &Client, schema: &str, name: &str, kind: &str, version_num: i32) -> AppResult<String> {
    let rel = qualified(schema, name);
    match kind {
        "view" | "matview" => {
            let sql = format!("SELECT pg_get_viewdef({REGCLASS}, true)");
            let row = client.query_one(&sql, &[&schema, &name]).await?;
            let body: String = row.get(0);
            let kw = if kind == "matview" { "MATERIALIZED VIEW" } else { "VIEW" };
            Ok(format!("CREATE OR REPLACE {kw} {rel} AS\n{body}"))
        }
        "function" => {
            let row = client
                .query_one(
                    "SELECT pg_get_functiondef(p.oid) FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace \
                     WHERE n.nspname = $1 AND p.proname = $2 LIMIT 1",
                    &[&schema, &name],
                )
                .await?;
            Ok(row.get(0))
        }
        _ => {
            let st = table_structure(client, schema, name, kind, version_num).await?;
            let mut lines: Vec<String> = Vec::new();
            for c in &st.columns {
                let mut l = format!("    {} {}", quote_ident(&c.name), c.format_type);
                if c.is_identity {
                    l.push_str(" GENERATED BY DEFAULT AS IDENTITY");
                } else if let Some(g) = &c.generation_expression {
                    l.push_str(&format!(" GENERATED ALWAYS AS ({g}) STORED"));
                } else if let Some(d) = &c.default {
                    l.push_str(&format!(" DEFAULT {d}"));
                }
                if !c.nullable {
                    l.push_str(" NOT NULL");
                }
                lines.push(l);
            }
            if !st.primary_key.is_empty() {
                lines.push(format!(
                    "    PRIMARY KEY ({})",
                    st.primary_key.iter().map(|c| quote_ident(c)).collect::<Vec<_>>().join(", ")
                ));
            }
            for fk in &st.foreign_keys {
                let mut l = format!(
                    "    CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {} ({})",
                    quote_ident(&fk.constraint_name),
                    fk.child_columns.iter().map(|c| quote_ident(c)).collect::<Vec<_>>().join(", "),
                    qualified(&fk.parent_schema, &fk.parent_table),
                    fk.parent_columns.iter().map(|c| quote_ident(c)).collect::<Vec<_>>().join(", ")
                );
                if let Some(u) = &fk.on_update {
                    if u != "NO ACTION" {
                        l.push_str(&format!(" ON UPDATE {u}"));
                    }
                }
                if let Some(d) = &fk.on_delete {
                    if d != "NO ACTION" {
                        l.push_str(&format!(" ON DELETE {d}"));
                    }
                }
                lines.push(l);
            }
            for ck in &st.checks {
                lines.push(format!("    CONSTRAINT {} {}", quote_ident(&ck.name), ck.condition));
            }
            let mut out = format!("CREATE TABLE {rel} (\n{}\n);", lines.join(",\n"));
            for ix in st.indexes.iter().filter(|i| !i.is_primary) {
                out.push_str(&format!("\n{};", ix.definition));
            }
            if let Some(c) = &st.comment {
                out.push_str(&format!("\nCOMMENT ON TABLE {rel} IS {};", crate::pg::sqlgen::quote_literal(c)));
            }
            for c in st.columns.iter().filter(|c| c.comment.is_some()) {
                out.push_str(&format!(
                    "\nCOMMENT ON COLUMN {rel}.{} IS {};",
                    quote_ident(&c.name),
                    crate::pg::sqlgen::quote_literal(c.comment.as_deref().unwrap_or(""))
                ));
            }
            for t in &st.triggers {
                out.push_str(&format!("\n{};", t.statement));
            }
            Ok(out)
        }
    }
}

pub async fn processes(client: &Client) -> AppResult<Vec<ProcessInfo>> {
    let rows = client
        .query(
            "SELECT pid, usename::text, datname::text, application_name::text, client_addr::text, state::text, \
             wait_event::text, query_start::text, query::text FROM pg_stat_activity \
             WHERE pid <> pg_backend_pid() ORDER BY state = 'active' DESC, query_start DESC NULLS LAST",
            &[],
        )
        .await?;
    Ok(rows
        .iter()
        .map(|r| ProcessInfo {
            pid: r.get(0),
            user: r.get(1),
            database: r.get(2),
            application: r.get(3),
            client: r.get(4),
            state: r.get(5),
            wait_event: r.get(6),
            query_start: r.get(7),
            query: r.get(8),
        })
        .collect())
}
