// Mirrors src-tauri/src/model.rs (serde camelCase).

export interface ConnectionConfig {
  id: string;
  name: string;
  groupId?: string | null;
  color: string;
  host: string;
  port: number;
  user: string;
  password?: string;
  database: string;
  sslMode: "disable" | "prefer" | "require" | "verify-ca" | "verify-full";
  sslRootCert?: string | null;
  startupCommands: string;
  limitRowsReturned: number;
  showSystemSchemas: boolean;
  keepAlive: boolean;
  environment: string;
  safeMode: number;
  lastOpenedSchema?: string | null;
  lastOpenedDatabase?: string | null;
  createdAt?: string | null;
}

export interface ConnectionGroup {
  id: string;
  name: string;
}

export interface SessionInfo {
  sessionId: string;
  connectionId: string;
  connectionName: string;
  database: string;
  user: string;
  host: string;
  serverVersion: string;
  serverVersionNum: number;
  tls: string | null;
  environment: string;
  color: string;
  limitRowsReturned: number;
}

export type ItemKind = "table" | "view" | "matview" | "foreign" | "partitioned" | "function" | "sequence";

export interface SchemaItem {
  schema: string;
  name: string;
  kind: ItemKind;
  comment: string | null;
  estimatedRows: number | null;
  totalSize: number | null;
  dataSize: number | null;
  indexSize: number | null;
  owner: string | null;
}

export interface FunctionItem {
  oid: number;
  schema: string;
  name: string;
  args: string;
  kind: string;
  owner: string | null;
  comment: string | null;
}

export interface SequenceItem {
  schema: string;
  name: string;
  startValue: string | null;
  minValue: string | null;
  maxValue: string | null;
  incrementBy: string | null;
  lastValue: string | null;
  cacheSize: string | null;
  comment: string | null;
}

export interface SchemaListing {
  items: SchemaItem[];
  functions: FunctionItem[];
  sequences: SequenceItem[];
}

export interface ForeignKeyInfo {
  constraintName: string;
  childColumns: string[];
  parentSchema: string;
  parentTable: string;
  parentColumns: string[];
  onUpdate: string | null;
  onDelete: string | null;
}

export interface ColumnInfo {
  position: number;
  name: string;
  dataType: string;
  formatType: string;
  nullable: boolean;
  default: string | null;
  comment: string | null;
  isPrimaryKey: boolean;
  foreignKey: ForeignKeyInfo | null;
  isIdentity: boolean;
  isGenerated: boolean;
  generationExpression: string | null;
  characterMaximumLength: number | null;
  numericPrecision: number | null;
  numericScale: number | null;
  enumValues: string[] | null;
}

export interface IndexInfo {
  name: string;
  algorithm: string;
  isUnique: boolean;
  isPrimary: boolean;
  columns: string[];
  condition: string | null;
  include: string | null;
  definition: string;
  comment: string | null;
  size: number | null;
}

export interface CheckConstraint { name: string; condition: string }
export interface TriggerInfo { name: string; event: string; timing: string; statement: string }

export interface TableStructure {
  schema: string;
  table: string;
  kind: string;
  columns: ColumnInfo[];
  primaryKey: string[];
  foreignKeys: ForeignKeyInfo[];
  indexes: IndexInfo[];
  checks: CheckConstraint[];
  triggers: TriggerInfo[];
  comment: string | null;
}

export interface ResultColumn { name: string; typeName: string; typeOid: number }

export interface DbError {
  message: string;
  code?: string | null;
  detail?: string | null;
  hint?: string | null;
  position?: number | null;
}

export interface QueryResult {
  statement: string;
  columns: ResultColumn[];
  rows: (string | null)[][];
  rowsAffected: number | null;
  command: string | null;
  durationMs: number;
  error: DbError | null;
  truncated: boolean;
}

export interface SortSpec { column: string; direction: "asc" | "desc" }

export type FilterOp =
  | "eq" | "neq" | "gt" | "gte" | "lt" | "lte"
  | "contains" | "not_contains" | "starts_with" | "ends_with"
  | "like" | "not_like" | "ilike" | "in" | "not_in"
  | "is_null" | "is_not_null" | "is_true" | "is_false" | "between" | "raw";

export interface FilterSpec {
  column: string;
  op: FilterOp;
  value?: string | null;
  value2?: string | null;
  enabled: boolean;
}

export interface FetchRowsRequest {
  sessionId: string;
  schema: string;
  table: string;
  kind: string;
  limit: number;
  offset: number;
  sort: SortSpec[];
  filters: FilterSpec[];
  quickFilter?: string | null;
  countMode: "auto" | "exact" | "estimate" | "none";
}

export interface FetchRowsResponse {
  columns: ResultColumn[];
  rows: (string | null)[][];
  ctids: string[] | null;
  total: number | null;
  totalIsEstimate: boolean;
  durationMs: number;
  sql: string;
}

export type CellValue =
  | { kind: "null" }
  | { kind: "text"; value: string }
  | { kind: "default" }
  | { kind: "expr"; value: string };

export interface RowIdentity {
  keyValues: Record<string, string | null>;
  ctid: string | null;
}

export type RowChange =
  | { kind: "insert"; values: Record<string, CellValue> }
  | { kind: "update"; identity: RowIdentity; values: Record<string, CellValue> }
  | { kind: "delete"; identity: RowIdentity };

export interface ApplyChangesRequest {
  sessionId: string;
  schema: string;
  table: string;
  changes: RowChange[];
  primaryKey: string[];
}

export interface HistoryEntry {
  id: string;
  connectionId: string;
  database: string | null;
  sql: string;
  at: string;
  durationMs: number;
  ok: boolean;
  message: string | null;
  rows: number | null;
}

export interface FavoriteQuery {
  id: string;
  name: string;
  sql: string;
  connectionId: string | null;
  createdAt: string;
}

export interface ProcessInfo {
  pid: number;
  user: string | null;
  database: string | null;
  application: string | null;
  client: string | null;
  state: string | null;
  waitEvent: string | null;
  queryStart: string | null;
  query: string | null;
}

export interface AppSettings {
  defaultLimit: number;
  estimateCountThreshold: number;
  theme: string;
  editorFontSize: number;
  historyKeepDays: number;
  autoCloseBrackets: boolean;
  lastConnectionId?: string | null;
}

export type DdlOp =
  | { op: "add_column"; name: string; data_type: string; nullable: boolean; default?: string | null; comment?: string | null }
  | { op: "drop_column"; name: string }
  | { op: "rename_column"; name: string; new_name: string }
  | { op: "change_type"; name: string; data_type: string; using?: string | null }
  | { op: "set_nullable"; name: string; nullable: boolean }
  | { op: "set_default"; name: string; default?: string | null }
  | { op: "set_comment"; name: string; comment?: string | null }
  | { op: "set_table_comment"; comment?: string | null }
  | { op: "rename_table"; new_name: string }
  | { op: "add_index"; name: string; columns: string[]; unique: boolean; method?: string | null; condition?: string | null }
  | { op: "drop_index"; name: string }
  | { op: "add_primary_key"; columns: string[] }
  | { op: "drop_constraint"; name: string }
  | { op: "add_foreign_key"; name?: string | null; columns: string[]; ref_schema: string; ref_table: string; ref_columns: string[]; on_update?: string | null; on_delete?: string | null }
  | { op: "truncate"; cascade: boolean; restart_identity: boolean }
  | { op: "drop_table"; cascade: boolean }
  | { op: "create_table"; name: string; columns: { name: string; dataType: string; nullable: boolean; default?: string | null; primaryKey: boolean }[] }
  | { op: "raw"; sql: string };

export interface ExportRequest {
  path: string;
  format: "csv" | "json" | "sql" | "markdown";
  columns: string[];
  rows: (string | null)[][];
  table?: string | null;
  header: boolean;
}
