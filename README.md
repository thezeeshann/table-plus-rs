# Tusk

A fast, native PostgreSQL desktop client for macOS, built with **Rust + Tauri 2**
and modelled on TablePlus (reverse-engineered with REA; see
[`docs/REVERSE_ENGINEERING.md`](docs/REVERSE_ENGINEERING.md)).

## Features

- Connections: grouped list, colour tags, environment badges, import from `postgresql://` URL, test before save, passwords in the macOS Keychain
- Workspace: items sidebar (tables, views, materialized views, functions, sequences) with search and schema switcher; saved queries; per-connection history
- Table data: virtualized grid, sort (shift-click for multi), filters (16 operators + raw SQL), quick filter, pagination, column hide, FK follow (`→`)
- Editing: inline cell edit, add / duplicate / delete rows, set NULL / DEFAULT / expression, preview SQL, commit in one transaction (PK or ctid identity)
- Structure: edit name / type / nullable / default / comment, add/drop columns, PK, foreign keys, checks, triggers, table comment — all as previewable DDL
- Indexes: list with size/predicate, create (btree/hash/gin/gist/brin), drop
- DDL view for tables, views, functions; sequence details
- SQL editor: CodeMirror 6 with PostgreSQL dialect, schema-aware autocompletion, run current statement (`⌘⏎`) / run all (`⌘⇧⏎`), cancel, multi-result tabs, error detail/hint/position, export CSV/JSON/SQL/Markdown, save to favourites, open/save `.sql`
- Tools: process list (cancel/terminate backends), create table, create schema, rename/truncate/drop, refresh materialized view
- Theme: TablePlus "Jade Dark" palette

## Development

```sh
pnpm install
pnpm tauri dev      # runs Vite + the Rust shell
pnpm tauri build    # produces Tusk.app / .dmg in src-tauri/target/release/bundle
cd src-tauri && cargo test
```

Data lives in `~/Library/Application Support/dev.zeeshan.tusk/`
(`connections.json`, `settings.json`, `favorites.json`, `history/<connection>.jsonl`).

## Keyboard shortcuts

| Action | Keys |
|---|---|
| Commit changes | ⌘S |
| Preview SQL | ⌘⇧P |
| Discard | ⌘Esc |
| Add row | ⌘I |
| Duplicate rows | ⌘D |
| Delete rows | ⌘⌫ |
| Run current / all | ⌘⏎ / ⌘⇧⏎ |
| Reload | ⌘R |
| Reconnect | ⌘⇧R |
| Open database | ⌘K |
| New SQL tab / close tab | ⌘T / ⌘W |
| Process list | ⌘. |
| Data / Structure / Index | ⌘⇧1 / 2 / 3 |
| Toggle sidebar | ⌘\ |
