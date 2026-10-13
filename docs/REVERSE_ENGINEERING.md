# TablePlus 27.0.2 — reverse-engineering notes

Target: `/Applications/TablePlus.app` (build 818, arm64 slice, SHA-256
`4547216d…c05a`), observed on 2026-10-13 with REA while the app was running
(PID 1891, window "dev-db : postgres - PostgreSQL 17.6").

No decompiler provider was configured (Ghidra/Hopper/IDA absent), so everything
below is based on **shipped-artifact evidence** (Info.plist, nib inventory,
Mach-O symbol tables, string tables, bundled JSON themes, local data files) and
**passive runtime observation** (accessibility tree + screenshot of the live
window, menu bar enumeration). Nothing was decompiled; pseudocode claims are not
made.

## Architecture (observed)

| Fact | Evidence |
|---|---|
| Main binary is Swift/ObjC AppKit; UI is nib-driven (234 resources, one `.nib` per window/controller: `DatabaseWindow`, `QueryController`, `TableDataController`, `TableStructureController`, `TableIndexController`, `LeftSidebarController`, `FilterViewController`, `PostgreSQLForm`, …) | `ls Contents/Resources` |
| One framework per database driver; PostgreSQL support = `PostgreSQL.framework` + shared `SQL.framework` | `Contents/Frameworks`, `otool -L` |
| PostgreSQL driver links **libpq** (bundled `libpq.5.17.dylib`, `libpq.5.14.dylib`) + OpenSSL 3; version-specific `dump_pg_*`/`restore_pg_*` helpers for 10.5→18.0 | Resources inventory |
| Swift types in the driver: `PGConnection`, `PGConnectionRef`, `PGDatabase`, `PGQueryResult`, `PGQueryResultRow`; Redshift/Cockroach/Greenplum/Vertica subclass the same driver (`RSDatabase`, `CRDatabase`, `GPDatabase`, `VEDatabase`) | `nm` + `swift-demangle` |
| Shared model in `SQL.framework`: `SQLDriver`, `SQLDatabase`, `SQLItem`, `SQLItemGroup`, `SQLNamespace`, `TableData`, `ColumnData`, `ForeignKey`, `SQLRowData`, `RowState`, `QueryType`, `ConnectionParams`, `DataType.build(pgRawValue:driver:)` (maps PG type OIDs) | demangled symbols |
| Row updates use PK values first, then fall back to physical identity: `(tableoid, ctid) = (SELECT tableoid, ctid FROM …)`; UI string "Save Again with Row Identity" | PostgreSQL.framework strings, Localizable.strings |
| Pending-edit colouring: inserted = green, modified = amber, deleted = red (`ContentInsertedBackground`, `ContentModifiedBackground`, `ContentDeletedRowBackground`) | `jade-dark.json` |
| Row count strategy: `reltuples::int8` estimate, exact `COUNT(*)` under a threshold ("Estimate count when table has more than …") | strings |
| Safe mode / alert mode (prompt before non-SELECT), keep-alive ping every 30 s, per-connection `LimitRowsReturned` | Localizable.strings, `Connections.plist` keys |
| Local data: `~/Library/Application Support/com.tinyapp.TablePlus/Data/{Connections.plist, ConnectionGroups.plist, Settings.json, Favorite/, History/<conn-id>/<day>/<ts>.sql}`; passwords in Keychain | filesystem |

## Catalog queries ported verbatim-in-spirit

All of these were recovered as string literals in `PostgreSQL.framework` and are
re-implemented in `src-tauri/src/pg/catalog.rs`:

- databases: `SELECT datname FROM pg_database`
- schemas: `SELECT nspname FROM pg_catalog.pg_namespace`
- relations: `information_schema.tables UNION pg_class WHERE relkind='m'` (materialized views), sizes from `pg_statio_user_tables` + `pg_total_relation_size/pg_table_size/pg_indexes_size`, owners via `pg_get_userbyid(relowner)`, estimates via `reltuples::int8`
- columns: `information_schema.columns` joined to `pg_attribute` for `format_type(atttypid, atttypmod)` and `col_description`
- primary key: `table_constraints` × `key_column_usage` where `constraint_type='PRIMARY KEY'`
- foreign keys: `pg_constraint` with `unnest(conkey/confkey)` and `confupdtype/confdeltype` → restrict/cascade/set null/set default/no action
- indexes: `pg_index` + `pg_get_indexdef(indexrelid, k, …)` over `generate_series(1, indnkeyatts)`, `INCLUDE` columns from `indnkeyatts+1..indnatts`, partial predicate via `pg_get_expr(indpred, indrelid)`, `obj_description(indexrelid,'pg_class')`
- checks: `pg_constraint` + `pg_get_expr(conbin, conrelid)`
- triggers: `information_schema.triggers`
- generated columns: `is_generated`, `generation_expression`, `attgenerated`
- enums: `pg_type ⋈ pg_enum ORDER BY enumsortorder`
- functions: `pg_proc` with `prokind` (≥11) or `proisagg/proiswindow` (<11), `pg_get_function_identity_arguments`, `pg_get_functiondef`
- sequences: `pg_sequences` (≥10)
- views: `pg_get_viewdef`
- process list: `pg_stat_activity … state!='idle'`, `pg_cancel_backend`
- misc: `SELECT version()`, `SET CLIENT_ENCODING TO`, `SHOW transaction_read_only`, `pg_is_in_recovery()`, PostGIS detection `typname='geometry' OR 'geography'`

## UI layout (observed from the live window)

- Top toolbar (overlay titlebar): sidebar toggle, lock (safe mode), DB switcher, SQL button, centre **status pill** `LOCAL | PostgreSQL 17.6 : TLSv1.3 : dev-db : postgres : public.conversation_notes`, reload, metrics, shortcuts, split buttons.
- Left sidebar: segmented `Items | Queries | History`, search field, outline with `Functions`, `Tables`, …, schema popup (`public`) + `+` at the bottom.
- Tab bar with pinned/preview tabs (`openedSplits[].pinned`, `type: item | query_editor` in Settings.json).
- Data grid: row numbers, sortable headers, FK cells show `→`, bottom bar `Data | Structure | Index`, `+ Row`, `1-25 of 25 rows`, `Filters`, `Columns`, pager.
- Menu shortcuts: ⌘S commit, ⌘⇧P preview, ⌘I add row, ⌘D duplicate, ⌘⏎ run current, ⌘⇧⏎ run all, ⌘R reload, ⌘⇧R reconnect, ⌘K open database, ⌘T new tab, ⌘. process list, ⌘L plugins.

## Not ported (out of scope for a PostgreSQL-only v1)

SSH tunnelling (`SSH011/012/095.framework`), backup/restore via bundled `pg_dump`, Metrics Board, ER Diagram, AI assistant / MCP server, plugins, user management, import wizards, Touch ID passcode, Sparkle updater, licensing.
