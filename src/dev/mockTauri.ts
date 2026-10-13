// Browser-only stand-in for the Tauri IPC bridge so the UI can be exercised
// headlessly (Playwright) without the Rust shell. Enabled by VITE_MOCK=1.
import type * as T from "@/lib/types";

const connections: T.ConnectionConfig[] = [
  { id: "c1", name: "dev-db", groupId: "g1", color: "1", host: "127.0.0.1", port: 5432, user: "postgres", database: "tusk_demo", sslMode: "prefer", startupCommands: "", limitRowsReturned: 300, showSystemSchemas: false, keepAlive: true, environment: "Local", safeMode: 0 },
  { id: "c2", name: "prod-replica", groupId: null, color: "5", host: "db.example.com", port: 5432, user: "readonly", database: "app", sslMode: "require", startupCommands: "", limitRowsReturned: 300, showSystemSchemas: false, keepAlive: true, environment: "Production", safeMode: 1 },
];
const groups: T.ConnectionGroup[] = [{ id: "g1", name: "Work" }];
const session: T.SessionInfo = { sessionId: "s1", connectionId: "c1", connectionName: "dev-db", database: "tusk_demo", user: "postgres", host: "127.0.0.1", serverVersion: "17.6", serverVersionNum: 170006, tls: "TLSv1.3", environment: "Local", color: "1", limitRowsReturned: 300 };

const usersCols: T.ColumnInfo[] = [
  { position: 1, name: "id", dataType: "int8", formatType: "bigint", nullable: false, default: null, comment: null, isPrimaryKey: true, foreignKey: null, isIdentity: true, isGenerated: false, generationExpression: null, characterMaximumLength: null, numericPrecision: 64, numericScale: 0, enumValues: null },
  { position: 2, name: "email", dataType: "text", formatType: "text", nullable: false, default: null, comment: "Login email", isPrimaryKey: false, foreignKey: null, isIdentity: false, isGenerated: false, generationExpression: null, characterMaximumLength: null, numericPrecision: null, numericScale: null, enumValues: null },
  { position: 3, name: "name", dataType: "text", formatType: "text", nullable: true, default: null, comment: null, isPrimaryKey: false, foreignKey: null, isIdentity: false, isGenerated: false, generationExpression: null, characterMaximumLength: null, numericPrecision: null, numericScale: null, enumValues: null },
  { position: 4, name: "mood", dataType: "mood", formatType: "mood", nullable: true, default: "'neutral'::mood", comment: null, isPrimaryKey: false, foreignKey: null, isIdentity: false, isGenerated: false, generationExpression: null, characterMaximumLength: null, numericPrecision: null, numericScale: null, enumValues: ["happy", "sad", "neutral"] },
  { position: 5, name: "active", dataType: "bool", formatType: "boolean", nullable: false, default: "true", comment: null, isPrimaryKey: false, foreignKey: null, isIdentity: false, isGenerated: false, generationExpression: null, characterMaximumLength: null, numericPrecision: null, numericScale: null, enumValues: null },
  { position: 6, name: "created_at", dataType: "timestamptz", formatType: "timestamp with time zone", nullable: false, default: "now()", comment: null, isPrimaryKey: false, foreignKey: null, isIdentity: false, isGenerated: false, generationExpression: null, characterMaximumLength: null, numericPrecision: null, numericScale: null, enumValues: null },
];
const postsFk: T.ForeignKeyInfo = { constraintName: "posts_user_id_fkey", childColumns: ["user_id"], parentSchema: "public", parentTable: "users", parentColumns: ["id"], onUpdate: "NO ACTION", onDelete: "CASCADE" };
const postsCols: T.ColumnInfo[] = [
  { ...usersCols[0], name: "id", dataType: "uuid", formatType: "uuid", isIdentity: false, default: "gen_random_uuid()" },
  { ...usersCols[2], name: "user_id", dataType: "int8", formatType: "bigint", nullable: false, foreignKey: postsFk },
  { ...usersCols[2], name: "title", nullable: false },
  { ...usersCols[2], name: "score", dataType: "numeric", formatType: "numeric(10,2)", numericPrecision: 10, numericScale: 2 },
];
const structures: Record<string, T.TableStructure> = {
  users: { schema: "public", table: "users", kind: "table", columns: usersCols, primaryKey: ["id"], foreignKeys: [], indexes: [{ name: "users_pkey", algorithm: "BTREE", isUnique: true, isPrimary: true, columns: ["id"], condition: null, include: null, definition: "CREATE UNIQUE INDEX users_pkey ON public.users USING btree (id)", comment: null, size: 65536 }, { name: "users_email_key", algorithm: "BTREE", isUnique: true, isPrimary: false, columns: ["email"], condition: null, include: null, definition: "CREATE UNIQUE INDEX users_email_key ON public.users USING btree (email)", comment: null, size: 81920 }], checks: [], triggers: [{ name: "trg_users", event: "UPDATE", timing: "BEFORE", statement: "CREATE TRIGGER trg_users BEFORE UPDATE ON public.users FOR EACH ROW EXECUTE FUNCTION suppress_redundant_updates_trigger()" }], comment: "Application users" },
  posts: { schema: "public", table: "posts", kind: "table", columns: postsCols, primaryKey: ["id"], foreignKeys: [postsFk], indexes: [{ name: "posts_published_idx", algorithm: "BTREE", isUnique: false, isPrimary: false, columns: ["published_at"], condition: "(published_at IS NOT NULL)", include: null, definition: "CREATE INDEX posts_published_idx ON public.posts USING btree (published_at) WHERE (published_at IS NOT NULL)", comment: null, size: 40960 }], checks: [{ name: "posts_score_check", condition: "CHECK ((score >= (0)::numeric))" }], triggers: [], comment: null },
  logs: { schema: "public", table: "logs", kind: "table", columns: [{ ...usersCols[5], name: "ts", isPrimaryKey: false }, { ...usersCols[2], name: "level" }, { ...usersCols[2], name: "message" }], primaryKey: [], foreignKeys: [], indexes: [], checks: [], triggers: [], comment: null },
};
const moods = ["happy", "sad", "neutral"];
function userRows(limit: number, offset: number): (string | null)[][] {
  return Array.from({ length: Math.min(limit, 1200 - offset) }, (_, i) => { const g = offset + i + 1; return [String(g), `user${g}@example.com`, g % 7 === 0 ? null : `User ${g}`, moods[g % 3], g % 5 ? "t" : "f", `2025-06-18 12:03:${String(g % 60).padStart(2, "0")}.72+00`]; });
}
const applied: string[] = [];
const history: T.HistoryEntry[] = [{ id: "h1", connectionId: "c1", database: "tusk_demo", sql: "SELECT * FROM users LIMIT 100;", at: new Date().toISOString(), durationMs: 12, ok: true, message: null, rows: 100 }];
const favorites: T.FavoriteQuery[] = [{ id: "f1", name: "Active users", sql: "SELECT * FROM users WHERE active;", connectionId: null, createdAt: new Date().toISOString() }];

async function invoke(cmd: string, args: any = {}): Promise<any> {
  (window as any).__mockCalls = [...((window as any).__mockCalls ?? []), { cmd, args }];
  switch (cmd) {
    case "get_settings": return { defaultLimit: 300, estimateCountThreshold: 100000, theme: "jade-dark", editorFontSize: 13, historyKeepDays: 0, autoCloseBrackets: true };
    case "save_settings": return null;
    case "list_connections": return [connections, groups];
    case "get_connection": return connections.find((c) => c.id === args.id);
    case "save_connection": { const c = { ...args.config, id: args.config.id || "c3" }; const i = connections.findIndex((x) => x.id === c.id); if (i >= 0) connections[i] = c; else connections.push(c); return c; }
    case "delete_connection": { const i = connections.findIndex((x) => x.id === args.id); if (i >= 0) connections.splice(i, 1); return null; }
    case "test_connection": return "PostgreSQL 17.6 on aarch64 (TLS)";
    case "list_favorites": return favorites;
    case "save_favorite": { const f = { ...args.favorite, id: args.favorite.id || `f${favorites.length + 1}` }; favorites.push(f); return f; }
    case "list_history": return history;
    case "open_session": return { ...session, database: args.database ?? session.database };
    case "close_session": case "cancel_query": return null;
    case "list_schemas": return ["public", "audit"];
    case "list_databases": return ["postgres", "tusk_demo", "analytics"];
    case "list_schema_items": return {
      items: [
        { schema: "public", name: "users", kind: "table", comment: "Application users", estimatedRows: 1200, totalSize: 229376, dataSize: 114688, indexSize: 114688, owner: "postgres" },
        { schema: "public", name: "posts", kind: "table", comment: null, estimatedRows: 3000, totalSize: 1105920, dataSize: 950272, indexSize: 155648, owner: "postgres" },
        { schema: "public", name: "logs", kind: "table", comment: null, estimatedRows: 50, totalSize: 16384, dataSize: 8192, indexSize: 0, owner: "postgres" },
        { schema: "public", name: "active_users", kind: "view", comment: null, estimatedRows: null, totalSize: 0, dataSize: 0, indexSize: 0, owner: "postgres" },
        { schema: "public", name: "post_counts", kind: "matview", comment: null, estimatedRows: 1200, totalSize: 65536, dataSize: 65536, indexSize: 0, owner: "postgres" },
      ],
      functions: [{ oid: 1, schema: "public", name: "add_one", args: "x integer", kind: "function", owner: "postgres", comment: null }],
      sequences: [{ schema: "public", name: "order_seq", startValue: "100", minValue: "1", maxValue: "9223372036854775807", incrementBy: "1", lastValue: null, cacheSize: "1", comment: null }],
    };
    case "schema_columns": return { users: usersCols.map((c) => c.name), posts: postsCols.map((c) => c.name) };
    case "table_structure": return structures[args.table] ?? { ...structures.users, table: args.table, kind: args.kind };
    case "object_ddl": return `CREATE TABLE "public"."${args.name}" (\n    "id" bigint NOT NULL,\n    PRIMARY KEY ("id")\n);`;
    case "fetch_rows": {
      const r: T.FetchRowsRequest = args.req;
      const cols = (structures[r.table] ?? structures.users).columns.map((c) => ({ name: c.name, typeName: c.dataType, typeOid: 0 }));
      const rows = r.table === "users" ? userRows(r.limit, Number(r.offset)) : Array.from({ length: 25 }, (_, i) => cols.map((c, j) => (j === 0 ? String(i + 1) : `${c.name} ${i + 1}`)));
      const filtered = r.filters.length ? rows.slice(0, 3) : rows;
      return { columns: cols, rows: filtered, ctids: r.kind === "table" ? filtered.map((_, i) => `(0,${i + 1})`) : null, total: r.table === "users" && !r.filters.length ? 1200 : filtered.length, totalIsEstimate: false, durationMs: 4.2, sql: "SELECT ..." } as T.FetchRowsResponse;
    }
    case "preview_changes": { const r: T.ApplyChangesRequest = args.req; return r.changes.map((c) => c.kind === "update" ? `UPDATE "public"."${r.table}" SET ${Object.entries(c.values).map(([k, v]) => `"${k}" = ${v.kind === "null" ? "NULL" : `'${(v as any).value}'`}`).join(", ")} WHERE "id" = '${c.identity.keyValues.id}'` : c.kind === "insert" ? `INSERT INTO "public"."${r.table}" DEFAULT VALUES` : `DELETE FROM "public"."${r.table}" WHERE "id" = '${c.identity.keyValues.id}'`); }
    case "apply_changes": { const s = await invoke("preview_changes", args); applied.push(...s); return s; }
    case "preview_ddl": return (args.ops as T.DdlOp[]).map((o) => `-- ${o.op}`);
    case "apply_ddl": return (args.ops as T.DdlOp[]).map((o) => `-- ${o.op}`);
    case "run_query": {
      if (args.req.split) {
        const parts = (args.req.sql as string).split(";").map((x: string) => x.trim()).filter(Boolean);
        const out: any[] = [];
        for (const p of parts) { out.push(...(await invoke("run_query", { req: { ...args.req, sql: p, split: false }}))); if (out[out.length - 1].error) break; }
        return out;
      }
      const sql: string = args.req.sql;
      if (/oops/i.test(sql)) return [{ statement: sql, columns: [], rows: [], rowsAffected: null, command: "SELEC", durationMs: 1, error: { message: 'syntax error at or near "SELEC"', code: "42601", position: 1 }, truncated: false }];
      return [{ statement: sql, columns: [{ name: "id", typeName: "int4", typeOid: 23 }, { name: "label", typeName: "text", typeOid: 25 }], rows: [["1", "alpha"], ["2", null], ["3", "gamma"]], rowsAffected: 3, command: "SELECT", durationMs: 3.1, error: null, truncated: false }];
    }
    case "execute": return 1;
    case "export_rows": return args.req.rows.length;
    case "list_processes": return [{ pid: 4242, user: "postgres", database: "tusk_demo", application: "psql", client: "127.0.0.1", state: "active", waitEvent: null, queryStart: "2026-10-13 21:00:00", query: "SELECT pg_sleep(100)" }];
    default:
      if (cmd.startsWith("plugin:dialog|save")) return "/tmp/export.csv";
      if (cmd.startsWith("plugin:")) return null;
      throw { message: `mock: unknown command ${cmd}` };
  }
}

(window as any).__TAURI_INTERNALS__ = {
  invoke,
  transformCallback: (cb: any) => { const id = Math.random(); (window as any)[`_${id}`] = cb; return id; },
  metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main", windowLabel: "main" } },
  plugins: {},
  convertFileSrc: (p: string) => p,
};
(window as any).__TAURI_OS_PLUGIN_INTERNALS__ = { eol: "\n" };
(window as any).__mockApplied = applied;
