//! Live PostgreSQL sessions. One tokio-postgres client per open workspace.

use crate::error::AppResult;
use crate::model::{ConnectionConfig, SessionInfo};
use crate::pg::tls::make_tls;
use std::sync::Arc;
use crate::storage::Storage;
use tokio::sync::Mutex;
use tokio_postgres::{CancelToken, Client, Config, NoTls};

pub struct Session {
    pub info: SessionInfo,
    pub config: ConnectionConfig,
    pub client: Arc<Client>,
    pub cancel: CancelToken,
    pub tls_used: bool,
    /// False behind transaction-mode poolers, where named prepared statements hang.
    pub supports_prepare: bool,
    /// Serialises query execution per session so a long-running query and a
    /// cancel request don't race; mirrors TablePlus' single connection per tab.
    pub exec_lock: Arc<Mutex<()>>,
    /// Where query history is written (None in tests).
    pub storage: Option<Arc<Storage>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Session {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn build_config(cfg: &ConnectionConfig, database: &str) -> Config {
    let mut pg = Config::new();
    pg.host(&cfg.host);
    pg.port(cfg.port);
    pg.user(&cfg.user);
    if let Some(p) = &cfg.password {
        pg.password(p);
    }
    pg.dbname(if database.is_empty() { "postgres" } else { database });
    pg.application_name("table-plus-rs");
    pg.connect_timeout(std::time::Duration::from_secs(15));
    if cfg.keep_alive {
        pg.keepalives(true);
        pg.keepalives_idle(std::time::Duration::from_secs(30));
    }
    pg.ssl_mode(match cfg.ssl_mode.as_str() {
        "disable" => tokio_postgres::config::SslMode::Disable,
        "require" | "verify-ca" | "verify-full" => tokio_postgres::config::SslMode::Require,
        _ => tokio_postgres::config::SslMode::Prefer,
    });
    pg
}

/// Probes named prepared-statement support on a *throwaway* connection.
/// Transaction poolers (Supabase's Supavisor on 6543, PgBouncer) accept `Parse`
/// but wedge the connection on the following `Close`, so the probe must never
/// touch the real session. Any failure or timeout counts as unsupported.
async fn probe_prepare(cfg: &ConnectionConfig, database: &str) -> bool {
    let Ok((client, task, _)) = connect_raw(cfg, database).await else { return false };
    let probe = async {
        let stmt = client.prepare("SELECT 1").await.ok()?;
        drop(stmt);
        client.simple_query("SELECT 1").await.ok()?;
        Some(())
    };
    let ok = tokio::time::timeout(std::time::Duration::from_secs(3), probe).await.ok().flatten().is_some();
    task.abort();
    if !ok {
        log::info!("named prepared statements unsupported (transaction pooler?) – column type lookup disabled");
    }
    ok
}

/// Connects and returns (client, connection task, tls_used).
async fn connect_raw(
    cfg: &ConnectionConfig,
    database: &str,
) -> AppResult<(Client, tokio::task::JoinHandle<()>, bool)> {
    let pg = build_config(cfg, database);
    if cfg.ssl_mode == "disable" {
        let (client, conn) = pg.connect(NoTls).await?;
        let task = tokio::spawn(async move {
            if let Err(e) = conn.await {
                log::warn!("connection closed: {e}");
            }
        });
        return Ok((client, task, false));
    }
    let tls = make_tls(&cfg.ssl_mode, cfg.ssl_root_cert.as_deref())?;
    let (client, conn) = pg.connect(tls).await?;
    let task = tokio::spawn(async move {
        if let Err(e) = conn.await {
            log::warn!("connection closed: {e}");
        }
    });
    // require/verify-* fail without TLS, so they're certain. For prefer, tokio-postgres doesn't
    // expose the outcome; ask the server (behind a pooler this reflects the pooler->server leg).
    let tls_used = if cfg.ssl_mode == "prefer" {
        client
            .query_typed_one("SELECT ssl FROM pg_stat_ssl WHERE pid = pg_backend_pid()", &[])
            .await
            .map(|r| r.get::<_, bool>(0))
            .unwrap_or(false)
    } else {
        true
    };
    Ok((client, task, tls_used))
}

/// Connects with the saved settings and returns a ready session.
pub async fn test(cfg: &ConnectionConfig) -> AppResult<String> {
    let (client, task, tls) = connect_raw(cfg, &cfg.database).await?;
    let row = client.query_typed_one("SELECT version()", &[]).await?;
    let v: String = row.get(0);
    task.abort();
    Ok(format!("{v}{}", if tls { " (TLS)" } else { "" }))
}

impl Session {
    pub async fn connect(cfg: ConnectionConfig, database: Option<String>, storage: Option<Arc<Storage>>) -> AppResult<Arc<Session>> {
        let db = database.unwrap_or_else(|| cfg.database.clone());
        let (client, task, tls_used) = connect_raw(&cfg, &db).await?;
        let cancel = client.cancel_token();

        let row = client
            .query_typed_one(
                "SELECT version(), current_setting('server_version_num')::int, current_database(), current_user, \
                 (SELECT version FROM pg_stat_ssl WHERE pid = pg_backend_pid())",
                &[],
            )
            .await?;
        let version: String = row.get(0);
        let version_num: i32 = row.get(1);
        let dbname: String = row.get(2);
        let user: String = row.get(3);
        let tls_version: Option<String> = row.get(4);

        if !cfg.startup_commands.trim().is_empty() {
            client.batch_execute(&cfg.startup_commands).await?;
        }

        let supports_prepare = probe_prepare(&cfg, &db).await;
        let short_version = version.split_whitespace().nth(1).map(|s| s.to_string()).unwrap_or_else(|| version.clone());
        let info = SessionInfo {
            session_id: uuid::Uuid::new_v4().to_string(),
            connection_id: cfg.id.clone(),
            connection_name: cfg.name.clone(),
            database: dbname,
            user,
            host: cfg.host.clone(),
            server_version: short_version,
            server_version_num: version_num,
            tls: if tls_used { tls_version.filter(|v| !v.is_empty()).or(Some("TLS".into())) } else { None },
            environment: cfg.environment.clone(),
            color: cfg.color.clone(),
            limit_rows_returned: cfg.limit_rows_returned,
        };
        Ok(Arc::new(Session {
            info,
            config: cfg,
            client: Arc::new(client),
            cancel,
            tls_used,
            supports_prepare,
            exec_lock: Arc::new(Mutex::new(())),
            storage,
            task,
        }))
    }

    /// Opens a fresh connection with the same settings (⌘⇧R).
    pub async fn reconnect(&self) -> AppResult<Arc<Session>> {
        Session::connect(self.config.clone(), Some(self.info.database.clone()), self.storage.clone()).await
    }

    /// Asks the server to cancel whatever this session is running.
    pub async fn cancel_running(&self) -> AppResult<()> {
        if self.config.ssl_mode == "disable" || !self.tls_used {
            self.cancel.cancel_query(NoTls).await?;
        } else {
            let tls = make_tls(&self.config.ssl_mode, self.config.ssl_root_cert.as_deref())?;
            self.cancel.cancel_query(tls).await?;
        }
        Ok(())
    }

    pub fn is_alive(&self) -> bool {
        !self.client.is_closed()
    }
}
