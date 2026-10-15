use serde::{Deserialize, Serialize};

/// Mirrors the subset of TablePlus' Connections.plist keys that matter for PostgreSQL.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionConfig {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub group_id: Option<String>,
    #[serde(default = "default_color")]
    pub color: String,
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub user: String,
    /// Stored by `Storage` in connections.json (mode 0600), separate from the connection record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(default)]
    pub database: String,
    #[serde(default = "default_ssl_mode")]
    pub ssl_mode: String,
    #[serde(default)]
    pub ssl_root_cert: Option<String>,
    #[serde(default)]
    pub startup_commands: String,
    #[serde(default = "default_limit")]
    pub limit_rows_returned: u32,
    #[serde(default)]
    pub show_system_schemas: bool,
    #[serde(default)]
    pub keep_alive: bool,
    #[serde(default)]
    pub environment: String,
    #[serde(default)]
    pub safe_mode: u8,
    #[serde(default)]
    pub last_opened_schema: Option<String>,
    #[serde(default)]
    pub last_opened_database: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
}

impl ConnectionConfig {
    /// A new, unsaved connection with the form's defaults.
    pub fn blank() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            group_id: None,
            color: default_color(),
            host: default_host(),
            port: default_port(),
            user: "postgres".into(),
            password: None,
            database: "postgres".into(),
            ssl_mode: default_ssl_mode(),
            ssl_root_cert: None,
            startup_commands: String::new(),
            limit_rows_returned: default_limit(),
            show_system_schemas: false,
            keep_alive: true,
            environment: String::new(),
            safe_mode: 0,
            last_opened_schema: None,
            last_opened_database: None,
            created_at: None,
        }
    }
}

fn default_color() -> String { "1".into() }
fn default_host() -> String { "127.0.0.1".into() }
fn default_port() -> u16 { 5432 }
fn default_ssl_mode() -> String { "prefer".into() }
fn default_limit() -> u32 { 300 }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionGroup {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub session_id: String,
    pub connection_id: String,
    pub connection_name: String,
    pub database: String,
    pub user: String,
    pub host: String,
    pub server_version: String,
    pub server_version_num: i32,
    pub tls: Option<String>,
    pub environment: String,
    pub color: String,
    pub limit_rows_returned: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaItem {
    pub schema: String,
    pub name: String,
    /// table | view | matview | foreign | partitioned
    pub kind: String,
    pub comment: Option<String>,
    pub estimated_rows: Option<i64>,
    pub total_size: Option<i64>,
    pub data_size: Option<i64>,
    pub index_size: Option<i64>,
    pub owner: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionItem {
    pub oid: u32,
    pub schema: String,
    pub name: String,
    pub args: String,
    /// function | procedure | aggregate | window | trigger
    pub kind: String,
    pub owner: Option<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SequenceItem {
    pub schema: String,
    pub name: String,
    pub start_value: Option<String>,
    pub min_value: Option<String>,
    pub max_value: Option<String>,
    pub increment_by: Option<String>,
    pub last_value: Option<String>,
    pub cache_size: Option<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaListing {
    pub items: Vec<SchemaItem>,
    pub functions: Vec<FunctionItem>,
    pub sequences: Vec<SequenceItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnInfo {
    pub position: i32,
    pub name: String,
    pub data_type: String,
    pub format_type: String,
    pub nullable: bool,
    pub default: Option<String>,
    pub comment: Option<String>,
    pub is_primary_key: bool,
    pub foreign_key: Option<ForeignKeyInfo>,
    pub is_identity: bool,
    pub is_generated: bool,
    pub generation_expression: Option<String>,
    pub character_maximum_length: Option<i32>,
    pub numeric_precision: Option<i32>,
    pub numeric_scale: Option<i32>,
    pub enum_values: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForeignKeyInfo {
    pub constraint_name: String,
    pub child_columns: Vec<String>,
    pub parent_schema: String,
    pub parent_table: String,
    pub parent_columns: Vec<String>,
    pub on_update: Option<String>,
    pub on_delete: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexInfo {
    pub name: String,
    pub algorithm: String,
    pub is_unique: bool,
    pub is_primary: bool,
    pub columns: Vec<String>,
    pub condition: Option<String>,
    pub include: Option<String>,
    pub definition: String,
    pub comment: Option<String>,
    pub size: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckConstraint {
    pub name: String,
    pub condition: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerInfo {
    pub name: String,
    pub event: String,
    pub timing: String,
    pub statement: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableStructure {
    pub schema: String,
    pub table: String,
    pub kind: String,
    pub columns: Vec<ColumnInfo>,
    pub primary_key: Vec<String>,
    pub foreign_keys: Vec<ForeignKeyInfo>,
    pub indexes: Vec<IndexInfo>,
    pub checks: Vec<CheckConstraint>,
    pub triggers: Vec<TriggerInfo>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultColumn {
    pub name: String,
    pub type_name: String,
    pub type_oid: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult {
    pub statement: String,
    pub columns: Vec<ResultColumn>,
    pub rows: Vec<Vec<Option<String>>>,
    pub rows_affected: Option<u64>,
    pub command: Option<String>,
    pub duration_ms: f64,
    pub error: Option<crate::error::ErrorPayload>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SortSpec {
    pub column: String,
    pub direction: String, // asc | desc
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterSpec {
    pub column: String,
    /// eq, neq, gt, gte, lt, lte, contains, not_contains, starts_with, ends_with,
    /// like, not_like, in, not_in, is_null, is_not_null, between, raw
    pub op: String,
    pub value: Option<String>,
    pub value2: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}
fn default_true() -> bool { true }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchRowsRequest {
    pub schema: String,
    pub table: String,
    pub kind: String,
    pub limit: u32,
    pub offset: u64,
    #[serde(default)]
    pub sort: Vec<SortSpec>,
    #[serde(default)]
    pub filters: Vec<FilterSpec>,
    #[serde(default)]
    pub quick_filter: Option<String>,
    #[serde(default)]
    pub count_mode: String, // auto | exact | estimate | none
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchRowsResponse {
    pub columns: Vec<ResultColumn>,
    pub rows: Vec<Vec<Option<String>>>,
    /// Physical row locator (ctid) per row when the relation supports it.
    pub ctids: Option<Vec<String>>,
    pub total: Option<i64>,
    pub total_is_estimate: bool,
    pub duration_ms: f64,
    pub sql: String,
}

/// A cell value coming back from the grid editor.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CellValue {
    Null,
    Text { value: String },
    Default,
    Expr { value: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RowChange {
    Insert { values: std::collections::BTreeMap<String, CellValue> },
    Update {
        identity: RowIdentity,
        values: std::collections::BTreeMap<String, CellValue>,
    },
    Delete { identity: RowIdentity },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RowIdentity {
    /// Primary key column -> original value (None == NULL)
    pub key_values: std::collections::BTreeMap<String, Option<String>>,
    pub ctid: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyChangesRequest {
    pub schema: String,
    pub table: String,
    pub changes: Vec<RowChange>,
    pub primary_key: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: String,
    pub connection_id: String,
    pub database: Option<String>,
    pub sql: String,
    pub at: String,
    pub duration_ms: f64,
    pub ok: bool,
    pub message: Option<String>,
    pub rows: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteQuery {
    pub id: String,
    pub name: String,
    pub sql: String,
    pub connection_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInfo {
    pub pid: i32,
    pub user: Option<String>,
    pub database: Option<String>,
    pub application: Option<String>,
    pub client: Option<String>,
    pub state: Option<String>,
    pub wait_event: Option<String>,
    pub query_start: Option<String>,
    pub query: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    #[serde(default = "default_limit")]
    pub default_limit: u32,
    #[serde(default = "default_estimate_threshold")]
    pub estimate_count_threshold: i64,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_font_size")]
    pub editor_font_size: u8,
    #[serde(default)]
    pub history_keep_days: u32,
    #[serde(default = "default_true")]
    pub auto_close_brackets: bool,
    #[serde(default)]
    pub last_connection_id: Option<String>,
}
fn default_estimate_threshold() -> i64 { 100_000 }
fn default_theme() -> String { "default".into() }
fn default_font_size() -> u8 { 13 }

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            default_limit: default_limit(),
            estimate_count_threshold: default_estimate_threshold(),
            theme: default_theme(),
            editor_font_size: default_font_size(),
            history_keep_days: 0,
            auto_close_brackets: true,
            last_connection_id: None,
        }
    }
}
