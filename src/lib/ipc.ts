import { invoke } from "@tauri-apps/api/core";
import type * as T from "./types";

export function errorMessage(e: unknown): string {
  if (!e) return "Unknown error";
  if (typeof e === "string") return e;
  if (typeof e === "object" && e && "message" in e) return String((e as { message: unknown }).message);
  return String(e);
}

export function asDbError(e: unknown): T.DbError {
  if (typeof e === "object" && e && "message" in e) return e as T.DbError;
  return { message: errorMessage(e) };
}

export const ipc = {
  listConnections: () => invoke<[T.ConnectionConfig[], T.ConnectionGroup[]]>("list_connections"),
  getConnection: (id: string) => invoke<T.ConnectionConfig>("get_connection", { id }),
  saveConnection: (config: T.ConnectionConfig) => invoke<T.ConnectionConfig>("save_connection", { config }),
  deleteConnection: (id: string) => invoke<void>("delete_connection", { id }),
  saveGroup: (group: T.ConnectionGroup) => invoke<T.ConnectionGroup>("save_group", { group }),
  deleteGroup: (id: string) => invoke<void>("delete_group", { id }),
  testConnection: (config: T.ConnectionConfig) => invoke<string>("test_connection", { config }),
  getSettings: () => invoke<T.AppSettings>("get_settings"),
  saveSettings: (settings: T.AppSettings) => invoke<void>("save_settings", { settings }),
  listFavorites: () => invoke<T.FavoriteQuery[]>("list_favorites"),
  saveFavorite: (favorite: T.FavoriteQuery) => invoke<T.FavoriteQuery>("save_favorite", { favorite }),
  deleteFavorite: (id: string) => invoke<void>("delete_favorite", { id }),
  listHistory: (connectionId: string, limit?: number, search?: string) =>
    invoke<T.HistoryEntry[]>("list_history", { connectionId, limit, search }),
  clearHistory: (connectionId: string) => invoke<void>("clear_history", { connectionId }),

  openSession: (connectionId: string, database?: string) =>
    invoke<T.SessionInfo>("open_session", { connectionId, database }),
  closeSession: (sessionId: string) => invoke<void>("close_session", { sessionId }),
  reconnectSession: (sessionId: string) => invoke<T.SessionInfo>("reconnect_session", { sessionId }),
  cancelQuery: (sessionId: string) => invoke<void>("cancel_query", { sessionId }),
  sessionAlive: (sessionId: string) => invoke<boolean>("session_alive", { sessionId }),

  listDatabases: (sessionId: string) => invoke<string[]>("list_databases", { sessionId }),
  listSchemas: (sessionId: string, showSystem?: boolean) => invoke<string[]>("list_schemas", { sessionId, showSystem }),
  listSchemaItems: (sessionId: string, schema: string) => invoke<T.SchemaListing>("list_schema_items", { sessionId, schema }),
  tableStructure: (sessionId: string, schema: string, table: string, kind: string) =>
    invoke<T.TableStructure>("table_structure", { sessionId, schema, table, kind }),
  objectDdl: (sessionId: string, schema: string, name: string, kind: string) =>
    invoke<string>("object_ddl", { sessionId, schema, name, kind }),
  listProcesses: (sessionId: string) => invoke<T.ProcessInfo[]>("list_processes", { sessionId }),
  killProcess: (sessionId: string, pid: number, terminate: boolean) =>
    invoke<boolean>("kill_process", { sessionId, pid, terminate }),
  schemaColumns: (sessionId: string, schema: string) =>
    invoke<Record<string, string[]>>("schema_columns", { sessionId, schema }),

  fetchRows: (req: T.FetchRowsRequest) => invoke<T.FetchRowsResponse>("fetch_rows", { req }),
  previewChanges: (req: T.ApplyChangesRequest) => invoke<string[]>("preview_changes", { req }),
  applyChanges: (req: T.ApplyChangesRequest) => invoke<string[]>("apply_changes", { req }),
  previewDdl: (schema: string, table: string, ops: T.DdlOp[]) => invoke<string[]>("preview_ddl", { schema, table, ops }),
  applyDdl: (sessionId: string, schema: string, table: string, ops: T.DdlOp[]) =>
    invoke<string[]>("apply_ddl", { sessionId, schema, table, ops }),
  exportRows: (req: T.ExportRequest) => invoke<number>("export_rows", { req }),
  writeTextFile: (path: string, content: string) => invoke<void>("write_text_file", { path, content }),
  readTextFile: (path: string) => invoke<string>("read_text_file", { path }),

  runQuery: (req: { sessionId: string; sql: string; split: boolean; rowLimit?: number; stopOnError?: boolean; record?: boolean }) =>
    invoke<T.QueryResult[]>("run_query", { req }),
  execute: (sessionId: string, sql: string) => invoke<number>("execute", { sessionId, sql }),
};
