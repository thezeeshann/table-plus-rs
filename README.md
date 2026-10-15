# table-plus-rs

A fast, native PostgreSQL desktop client for macOS, built with **Rust + Tauri 2**
and modelled on TablePlus (reverse-engineered with REA; see
[`docs/REVERSE_ENGINEERING.md`](docs/REVERSE_ENGINEERING.md)).

## Features

- Connections: grouped list, colour tags, environment badges, import from `postgresql://` URL, test before save, passwords stored in `connections.json` (mode 0600), never in the Keychain, so no macOS password prompts
- Workspace: items sidebar (tables, views, materialized views, functions, sequences) with search and schema switcher; saved queries; per-connection history
- Table data: virtualized grid, sort (shift-click for multi), filters (16 operators + raw SQL), quick filter, pagination, column hide, FK follow (`→`)
- Editing: inline cell edit, add / duplicate / delete rows, set NULL / DEFAULT / expression, preview SQL, commit in one transaction (PK or ctid identity)
- Structure: edit name / type / nullable / default / comment, add/drop columns, PK, foreign keys, checks, triggers, table comment — all as previewable DDL
- Indexes: list with size/predicate, create (btree/hash/gin/gist/brin), drop
- DDL view for tables, views, functions; sequence details
- SQL editor: CodeMirror 6 with PostgreSQL dialect, schema-aware autocompletion, run current statement (`⌘⏎`) / run all (`⌘⇧⏎`), cancel, multi-result tabs, error detail/hint/position, export CSV/JSON/SQL/Markdown, save to favourites, open/save `.sql`
- Tools: process list (cancel/terminate backends), create table, create schema, rename/truncate/drop, refresh materialized view
- Themes: `default` (TablePlus "Jade Dark") plus One Dark, One Dark Pro, Arc Blueberry and Atelier; live preview in Settings, see [Themes](#themes)

## Development

```sh
pnpm install
pnpm tauri dev      # runs Vite + the Rust shell
pnpm tauri build    # produces table-plus-rs.app / .dmg in src-tauri/target/release/bundle
cd src-tauri && cargo test
pnpm test           # theme tests (node's built-in runner)
```

Data lives in `~/Library/Application Support/dev.zeeshan.table-plus-rs/`
(`connections.json`, `settings.json`, `favorites.json`, `history/<connection>.jsonl`).

## Themes

| id | look |
|---|---|
| `default` | The original table-plus-rs look: Jade Dark, ported from TablePlus |
| `one-dark` | Atom's classic dark theme, with the brighter colors for status and values |
| `one-dark-pro` | The VS Code take on One Dark: same base, standard colors, softer surfaces |
| `arc-blueberry` | Deep blue-black with soft periwinkle accents and neon green and cyan |
| `atelier` | Warm olive-black with earthy greens, ochre and clay |

Pick one in **Settings** (`⌘,`). Moving through the list previews it live, **Save** keeps it, and
Cancel / Esc puts the old one back. A theme only changes how table-plus-rs looks.

The theme is chosen by, in order: `--theme NAME` on the command line, `$TABLE_PLUS_RS_THEME`, the saved setting,
then `default`.

```sh
open -n table-plus-rs.app --args --theme atelier        # one run, not saved
TABLE_PLUS_RS_THEME=arc-blueberry src-tauri/target/release/table-plus-rs
```

A bad `--theme` is reported in an error toast and the saved theme is used; a bad `$TABLE_PLUS_RS_THEME` or
stale saved name falls back to the default with a warning, so a typo never locks you out. Names are
forgiving: `Omarchy_One Dark Pro_Theme`, `arc blueberry` and `ATELIER` all resolve.

How it works: a theme is ten role colors (`accent good warn bad info dim track cursor text bg`).
`src/lib/theme.ts` expands them into every CSS variable the UI reads, and the default theme writes no
overrides at all, so it cannot drift from the stylesheet. `pnpm test` checks name matching, selection
order and WCAG contrast for the palettes and for the derived colors (text on every surface, white on
selected tabs and edited rows). To add a theme, append one entry to `THEMES` (default stays first), run
`pnpm test`, and if contrast fails lift `dim` or `track` rather than lowering the thresholds.

Palettes are ported from the Omarchy theme repos' `colors.toml` (https://omarchy.org/themes/).

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
