//! End-to-end backend checks against a live PostgreSQL. Skipped unless
//! TUSK_TEST_HOST/PORT are set (see docs). Uses the public modules only.

use std::collections::BTreeMap;
use tusk_lib::model::*;
use tusk_lib::pg::{catalog, query, session::SessionManager, sqlgen};

fn cfg() -> Option<ConnectionConfig> {
    let host = std::env::var("TUSK_TEST_HOST").ok()?;
    let port: u16 = std::env::var("TUSK_TEST_PORT").ok()?.parse().ok()?;
    Some(ConnectionConfig {
        id: "test".into(), name: "test".into(), group_id: None, color: "1".into(), host, port,
        user: std::env::var("TUSK_TEST_USER").unwrap_or_else(|_| "postgres".into()),
        password: std::env::var("TUSK_TEST_PASSWORD").ok(),
        database: std::env::var("TUSK_TEST_DB").unwrap_or_else(|_| "tusk_demo".into()),
        ssl_mode: "prefer".into(), ssl_root_cert: None, startup_commands: String::new(), limit_rows_returned: 300,
        show_system_schemas: false, keep_alive: true, environment: "Local".into(), safe_mode: 0,
        last_opened_schema: None, last_opened_database: None, created_at: None,
    })
}

#[tokio::test]
async fn catalog_and_queries() {
    let Some(cfg) = cfg() else { eprintln!("skipping: TUSK_TEST_HOST not set"); return; };
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
    let res = query::run_script(&s.client, "SELECT 1 AS a, 'x' AS b, NULL::int AS c; SELECT count(*) FROM users; SELEC oops;", 1000, false).await.unwrap();
    assert_eq!(res.len(), 3);
    assert_eq!(res[0].rows[0], vec![Some("1".into()), Some("x".into()), None]);
    assert_eq!(res[0].columns.iter().map(|c| c.type_name.as_str()).collect::<Vec<_>>(), vec!["int4", "text", "int4"]);
    assert_eq!(res[1].rows[0][0].as_deref(), Some("1200"));
    assert!(res[2].error.is_some());
    assert_eq!(res[2].error.as_ref().unwrap()["code"], "42601");

    // DML generation + transaction apply
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
