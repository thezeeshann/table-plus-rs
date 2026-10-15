//! End-to-end backend checks against a live PostgreSQL. Skipped unless
//! TABLE_PLUS_RS_TEST_HOST/PORT are set (see docs). Uses the public modules only.

use std::collections::BTreeMap;
use table_plus_rs_lib::model::*;
use table_plus_rs_lib::pg::{catalog, query, session::SessionManager, sqlgen};

fn cfg() -> Option<ConnectionConfig> {
    let host = std::env::var("TABLE_PLUS_RS_TEST_HOST").ok()?;
    let port: u16 = std::env::var("TABLE_PLUS_RS_TEST_PORT").ok()?.parse().ok()?;
    Some(ConnectionConfig {
        id: "test".into(), name: "test".into(), group_id: None, color: "1".into(), host, port,
        user: std::env::var("TABLE_PLUS_RS_TEST_USER").unwrap_or_else(|_| "postgres".into()),
        password: std::env::var("TABLE_PLUS_RS_TEST_PASSWORD").ok(),
        database: std::env::var("TABLE_PLUS_RS_TEST_DB").unwrap_or_else(|_| "tusk_demo".into()),
        ssl_mode: std::env::var("TABLE_PLUS_RS_TEST_SSL").unwrap_or_else(|_| "prefer".into()), ssl_root_cert: None, startup_commands: String::new(), limit_rows_returned: 300,
        show_system_schemas: false, keep_alive: true, environment: "Local".into(), safe_mode: 0,
        last_opened_schema: None, last_opened_database: None, created_at: None,
    })
}

#[tokio::test]
async fn catalog_and_queries() {
    let Some(cfg) = cfg() else { eprintln!("skipping: TABLE_PLUS_RS_TEST_HOST not set"); return; };
    let mgr = SessionManager::default();
    let info = mgr.open(cfg, None).await.expect("connect");
    assert!(info.server_version_num >= 100000);
    let s = mgr.get(&info.session_id).await.unwrap();

    let dbs = catalog::list_databases(&s.client).await.unwrap();
    assert!(dbs.iter().any(|d| d == "tusk_demo"), "{dbs:?}");
    let schemas = catalog::list_schemas(&s.client, false).await.unwrap();
    assert_eq!(schemas[0], "public");

    let listing = catalog::list_items(&s.client, "public", info.server_version_num).await.unwrap();
    let names: Vec<_> = listing.items.iter().map(|i| (i.name.as_str(), i.kind.as_str())).collect();
    assert!(names.contains(&("users", "table")));
    assert!(names.contains(&("active_users", "view")));
    assert!(names.contains(&("post_counts", "matview")));
    assert!(listing.functions.iter().any(|f| f.name == "add_one" && f.kind == "function"));
    assert!(listing.sequences.iter().any(|q| q.name == "order_seq"));
    let users = listing.items.iter().find(|i| i.name == "users").unwrap();
    assert_eq!(users.comment.as_deref(), Some("Application users"));
    assert!(users.estimated_rows.unwrap_or(0) > 1000);

    let st = catalog::table_structure(&s.client, "public", "posts", "table", info.server_version_num).await.unwrap();
    assert_eq!(st.primary_key, vec!["id"]);
    let fk = st.foreign_keys.iter().find(|f| f.child_columns == vec!["user_id"]).expect("fk");
    assert_eq!(fk.parent_table, "users");
    assert_eq!(fk.on_delete.as_deref(), Some("CASCADE"));
    assert!(st.indexes.iter().any(|i| i.name == "posts_published_idx" && i.condition.is_some()));
    assert!(st.checks.iter().any(|c| c.condition.contains("score")));
    let score = st.columns.iter().find(|c| c.name == "score").unwrap();
    assert_eq!(score.format_type, "numeric(10,2)");
    assert_eq!(score.numeric_precision, Some(10));

    let ust = catalog::table_structure(&s.client, "public", "users", "table", info.server_version_num).await.unwrap();
    let mood = ust.columns.iter().find(|c| c.name == "mood").unwrap();
    assert_eq!(mood.enum_values.as_deref(), Some(&["happy".to_string(), "sad".into(), "neutral".into()][..]));
    assert!(ust.columns.iter().find(|c| c.name == "id").unwrap().is_identity);
    assert_eq!(ust.triggers.len(), 1);
    assert_eq!(ust.columns.iter().find(|c| c.name == "email").unwrap().comment.as_deref(), Some("Login email"));

    let ddl = catalog::object_ddl(&s.client, "public", "users", "table", info.server_version_num).await.unwrap();
    assert!(ddl.starts_with("CREATE TABLE \"public\".\"users\""), "{ddl}");
    assert!(ddl.contains("PRIMARY KEY (\"id\")"));
    let vdef = catalog::object_ddl(&s.client, "public", "active_users", "view", info.server_version_num).await.unwrap();
    assert!(vdef.contains("CREATE OR REPLACE VIEW"));
    let fdef = catalog::object_ddl(&s.client, "public", "add_one", "function", info.server_version_num).await.unwrap();
    assert!(fdef.contains("FUNCTION public.add_one"));

    // simple-query execution: text rendering, multiple statements, errors with position
    let res = query::run_script(&s.client, "SELECT 1 AS a, 'x' AS b, NULL::int AS c; SELECT count(*) FROM users; SELEC oops;", 1000, false, s.supports_prepare).await.unwrap();
    assert_eq!(res.len(), 3);
    assert_eq!(res[0].rows[0], vec![Some("1".into()), Some("x".into()), None]);
    assert_eq!(res[0].columns.iter().map(|c| c.type_name.as_str()).collect::<Vec<_>>(), vec!["int4", "text", "int4"]);
    assert_eq!(res[1].rows[0][0].as_deref(), Some("1200"));
    assert!(res[2].error.is_some());
    assert_eq!(res[2].error.as_ref().unwrap()["code"], "42601");

    // DML generation + transaction apply (clean residue from earlier runs first)
    s.client.batch_execute("DELETE FROM logs WHERE level IN ('warn','error')").await.unwrap();
    let mut vals = BTreeMap::new();
    vals.insert("level".to_string(), CellValue::Text { value: "warn".into() });
    vals.insert("message".to_string(), CellValue::Expr { value: "'generated at ' || now()::text".into() });
    let sql = sqlgen::generate_change_sql("public", "logs", &[], &[RowChange::Insert { values: vals }]).unwrap();
    assert_eq!(sql[0], "INSERT INTO \"public\".\"logs\" (\"level\", \"message\") VALUES ('warn', 'generated at ' || now()::text)");
    s.client.batch_execute(&sql[0]).await.unwrap();

    // ctid-based update on a table without a primary key
    let row = s.client.query_one("SELECT ctid::text FROM logs WHERE level = 'warn' LIMIT 1", &[]).await.unwrap();
    let ctid: String = row.get(0);
    let mut upd = BTreeMap::new();
    upd.insert("level".to_string(), CellValue::Text { value: "error".into() });
    let sql = sqlgen::generate_change_sql("public", "logs", &[], &[RowChange::Update {
        identity: RowIdentity { key_values: BTreeMap::new(), ctid: Some(ctid.clone()) }, values: upd,
    }]).unwrap();
    assert!(sql[0].contains("ctid = "), "{}", sql[0]);
    s.client.batch_execute(&sql[0]).await.unwrap();
    let n: i64 = s.client.query_one("SELECT count(*) FROM logs WHERE level = 'error'", &[]).await.unwrap().get(0);
    assert_eq!(n, 1);

    // filter builder
    let w = sqlgen::build_where(&[FilterSpec { column: "name".into(), op: "contains".into(), value: Some("User 1".into()), value2: None, enabled: true }], Some("example"), &["email".into()]).unwrap();
    let rows = s.client.query(&format!("SELECT count(*) FROM users WHERE {w}"), &[]).await.unwrap();
    assert!(rows[0].get::<_, i64>(0) > 100);

    // cancel token path shouldn't error on an idle connection
    mgr.cancel(&info.session_id).await.unwrap();
    let procs = catalog::processes(&s.client).await.unwrap();
    assert!(procs.iter().all(|p| p.pid != 0));
    mgr.close(&info.session_id).await.unwrap();
}

/// Read-only smoke test for any reachable database (e.g. a Supabase pooler).
#[tokio::test]
async fn readonly_smoke() {
    let Some(cfg) = cfg() else { return; };
    let mgr = SessionManager::default();
    eprintln!("connecting to {}:{} ssl={} ...", cfg.host, cfg.port, cfg.ssl_mode);
    let info = mgr.open(cfg, None).await.expect("connect");
    let s = mgr.get(&info.session_id).await.unwrap();
    eprintln!("server {} ({}) db={} tls={:?} prepare={}", info.server_version, info.server_version_num, info.database, info.tls, s.supports_prepare);
    let schemas = catalog::list_schemas(&s.client, false).await.unwrap();
    eprintln!("schemas: {schemas:?}");
    assert!(!schemas.is_empty());
    let schema = if schemas.iter().any(|x| x == "public") { "public".to_string() } else { schemas[0].clone() };
    let listing = catalog::list_items(&s.client, &schema, info.server_version_num).await.unwrap();
    eprintln!("{} relations, {} functions, {} sequences in {schema}", listing.items.len(), listing.functions.len(), listing.sequences.len());
    for it in listing.items.iter().take(3) {
        let st = catalog::table_structure(&s.client, &it.schema, &it.name, &it.kind, info.server_version_num).await.unwrap();
        eprintln!("  {}.{} ({}) cols={} pk={:?} fks={} idx={}", it.schema, it.name, it.kind, st.columns.len(), st.primary_key, st.foreign_keys.len(), st.indexes.len());
        let ddl = catalog::object_ddl(&s.client, &it.schema, &it.name, &it.kind, info.server_version_num).await.unwrap();
        assert!(ddl.contains(&it.name));
        let rel = sqlgen::qualified(&it.schema, &it.name);
        let res = query::run_statement(&s.client, &format!("SELECT * FROM {rel} LIMIT 3"), 10, s.supports_prepare).await.unwrap();
        assert!(res[0].error.is_none(), "{:?}", res[0].error);
        eprintln!("    {} rows, types {:?}", res[0].rows.len(), res[0].columns.iter().map(|c| c.type_name.as_str()).collect::<Vec<_>>());
    }
    let res = query::run_script(&s.client, "SELECT 1 AS one; SELECT now()::text;", 10, true, s.supports_prepare).await.unwrap();
    assert_eq!(res.len(), 2);
    let procs = catalog::processes(&s.client).await;
    eprintln!("processes query ok: {}", procs.is_ok());
    mgr.close(&info.session_id).await.unwrap();
}

/// Reproduces the pooler failure: pipelining catalog and data queries on one
/// connection (what the UI does when a table opens) versus serialising them.
#[tokio::test]
async fn pooler_concurrency() {
    let Some(cfg) = cfg() else { return; };
    let mgr = SessionManager::default();
    let info = mgr.open(cfg, None).await.expect("connect");
    let s = mgr.get(&info.session_id).await.unwrap();
    let schema = "public".to_string();
    let listing = catalog::list_items(&s.client, &schema, info.server_version_num).await.unwrap();
    let Some(it) = listing.items.iter().find(|i| i.kind == "table") else { return; };
    let rel = sqlgen::qualified(&it.schema, &it.name);
    let sql = format!("SELECT ctid::text AS \"__table_plus_rs_ctid\", * FROM {rel} LIMIT 300 OFFSET 0");

    // serialised: structure -> rows -> estimate -> count (what the app does now)
    let t = std::time::Instant::now();
    for round in 0..3 {
        let _g = s.exec_lock.lock().await;
        catalog::table_structure(&s.client, &it.schema, &it.name, &it.kind, info.server_version_num).await.expect("structure");
        let r = query::run_statement(&s.client, &sql, 301, s.supports_prepare).await.expect("rows");
        assert!(r[0].error.is_none(), "{:?}", r[0].error);
        let _ = catalog::estimated_rows(&s.client, &it.schema, &it.name).await.expect("estimate");
        let c = s.client.query_typed_one(&format!("SELECT count(*) FROM {rel}"), &[]).await.expect("count");
        eprintln!("serialised round {round}: {} rows, count {} ({:?} total)", r[0].rows.len(), c.get::<_, i64>(0), t.elapsed());
    }

    // pipelined: structure and rows concurrently on the same connection
    let (a, b) = tokio::join!(
        catalog::table_structure(&s.client, &it.schema, &it.name, &it.kind, info.server_version_num),
        query::run_statement(&s.client, &sql, 301, s.supports_prepare),
    );
    eprintln!("pipelined: structure ok={} rows ok={}", a.is_ok(), b.as_ref().map(|r| r[0].error.is_none()).unwrap_or(false));
    if let Err(e) = &a { eprintln!("  structure error: {e:?}"); }
    mgr.close(&info.session_id).await.unwrap();
}
