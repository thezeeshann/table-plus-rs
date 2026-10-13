import { useMemo, useState, type ReactNode } from "react";
import { ChevronRight, ChevronDown, Search, Plus, Table2, Eye, Layers, FunctionSquare, Hash, Loader2, SlidersHorizontal, Trash2, Star } from "lucide-react";
import { useApp } from "@/store/app";
import { ContextMenu, useContextMenu, type MenuItem } from "@/components/common/ContextMenu";
import { Confirm } from "@/components/common/Confirm";
import { ipc, errorMessage } from "@/lib/ipc";
import { quoteIdent } from "@/lib/sql";
import { formatBytes, formatCount, relativeDay, timeOf } from "@/lib/format";
import type { FunctionItem, ItemKind, SchemaItem, SequenceItem } from "@/lib/types";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";

type Section = "tables" | "views" | "matviews" | "functions" | "sequences";

function kindIcon(kind: ItemKind, size = 13) {
  switch (kind) {
    case "view": return <Eye size={size} />;
    case "matview": return <Layers size={size} />;
    case "function": return <FunctionSquare size={size} />;
    case "sequence": return <Hash size={size} />;
    default: return <Table2 size={size} />;
  }
}

export function Sidebar() {
  const section = useApp((s) => s.sidebarSection);
  const setSection = useApp((s) => s.setSidebarSection);
  return (
    <>
      <div className="flex items-center justify-center gap-1 h-9 shrink-0 border-b border-border">
        {(["items", "queries", "history"] as const).map((s) => (
          <button key={s} className={`px-3 py-0.5 rounded text-[12px] ${section === s ? "bg-tab-selected text-white" : "text-tab-text hover:bg-tab-hover"}`} onClick={() => setSection(s)}>
            {s === "items" ? "Items" : s === "queries" ? "Queries" : "History"}
          </button>
        ))}
      </div>
      {section === "items" && <ItemsPanel />}
      {section === "queries" && <QueriesPanel />}
      {section === "history" && <HistoryPanel />}
    </>
  );
}

function ItemsPanel() {
  const listing = useApp((s) => s.listing);
  const loading = useApp((s) => s.listingLoading);
  const schemas = useApp((s) => s.schemas);
  const schema = useApp((s) => s.schema);
  const setSchema = useApp((s) => s.setSchema);
  const openItem = useApp((s) => s.openItem);
  const openQueryTab = useApp((s) => s.openQueryTab);
  const session = useApp((s) => s.session);
  const toast = useApp((s) => s.toast);
  const refresh = useApp((s) => s.refreshListing);
  const tabs = useApp((s) => s.tabs);
  const activeTabId = useApp((s) => s.activeTabId);
  const setCreateTable = useApp((s) => s.setCreateTableDialog);
  const setRename = useApp((s) => s.setRenameDialog);
  const [search, setSearch] = useState("");
  const [open, setOpen] = useState<Record<Section, boolean>>({ tables: true, views: true, matviews: true, functions: false, sequences: false });
  const [confirm, setConfirm] = useState<{ title: string; message: string; sql: string; label: string } | null>(null);
  const [newSchema, setNewSchema] = useState(false);
  const { menu, open: openMenu, close } = useContextMenu();

  const active = tabs.find((t) => t.id === activeTabId);
  const q = search.trim().toLowerCase();
  const match = (n: string) => !q || n.toLowerCase().includes(q);

  const groups = useMemo(() => {
    const items = listing?.items ?? [];
    return {
      tables: items.filter((i) => (i.kind === "table" || i.kind === "partitioned" || i.kind === "foreign") && match(i.name)),
      views: items.filter((i) => i.kind === "view" && match(i.name)),
      matviews: items.filter((i) => i.kind === "matview" && match(i.name)),
      functions: (listing?.functions ?? []).filter((f) => match(f.name)),
      sequences: (listing?.sequences ?? []).filter((s) => match(s.name)),
    };
  }, [listing, q]);

  const run = async (sql: string, okMsg: string) => {
    if (!session) return;
    try { await ipc.execute(session.sessionId, sql); toast("ok", okMsg); refresh(); } catch (e) { toast("err", errorMessage(e)); }
  };

  const exportTable = async (it: SchemaItem, format: "csv" | "json" | "sql") => {
    if (!session) return;
    const path = await saveDialog({ defaultPath: `${it.name}.${format}`, title: `Export ${it.name}` });
    if (!path) return;
    try {
      const res = await ipc.runQuery({ sessionId: session.sessionId, sql: `SELECT * FROM ${quoteIdent(it.schema)}.${quoteIdent(it.name)}`, split: false, record: false });
      const r = res.find((x) => x.columns.length) ?? res[0];
      if (r.error) throw r.error;
      const n = await ipc.exportRows({ path, format, columns: r.columns.map((c) => c.name), rows: r.rows, table: `${quoteIdent(it.schema)}.${quoteIdent(it.name)}`, header: true });
      toast("ok", `Exported ${n} rows to ${path}`);
    } catch (e) { toast("err", errorMessage(e)); }
  };

  const itemMenu = (it: SchemaItem): MenuItem[] => {
    const rel = `${quoteIdent(it.schema)}.${quoteIdent(it.name)}`;
    const isTable = it.kind === "table" || it.kind === "partitioned";
    return [
      { label: "Open in new tab", onClick: () => openItem(it.schema, it.name, it.kind, { pinned: true }) },
      { label: "Open structure", onClick: () => openItem(it.schema, it.name, it.kind, { pinned: true, view: "structure" }) },
      { label: "Show definition (DDL)", onClick: () => openItem(it.schema, it.name, it.kind, { pinned: true, view: "definition" }) },
      { separator: true },
      { label: "Copy name", onClick: () => navigator.clipboard.writeText(it.name) },
      { label: "Copy SELECT statement", onClick: () => navigator.clipboard.writeText(`SELECT * FROM ${rel} LIMIT 100;`) },
      { label: "Query in SQL editor", onClick: () => openQueryTab(`SELECT * FROM ${rel}\nLIMIT 100;`, it.name) },
      { separator: true },
      { label: "Export as CSV...", onClick: () => exportTable(it, "csv") },
      { label: "Export as JSON...", onClick: () => exportTable(it, "json") },
      { label: "Export as SQL inserts...", onClick: () => exportTable(it, "sql") },
      { separator: true },
      { label: "Rename...", onClick: () => setRename({ open: true, schema: it.schema, name: it.name, kind: it.kind }) },
      ...(it.kind === "matview" ? [{ label: "Refresh materialized view", onClick: () => run(`REFRESH MATERIALIZED VIEW ${rel}`, `${it.name} refreshed`) }] : []),
      ...(isTable ? [{ label: "Truncate...", danger: true, onClick: () => setConfirm({ title: `Truncate ${it.name}?`, message: "All rows will be removed. This cannot be undone.", sql: `TRUNCATE TABLE ${rel}`, label: "Truncate" }) }] : []),
      { label: "Delete...", danger: true, onClick: () => setConfirm({ title: `Drop ${it.kind === "view" ? "view" : it.kind === "matview" ? "materialized view" : "table"} ${it.name}?`, message: "This cannot be undone. Dependent objects will block the drop unless they're removed first.", sql: `DROP ${it.kind === "view" ? "VIEW" : it.kind === "matview" ? "MATERIALIZED VIEW" : "TABLE"} ${rel}`, label: "Drop" }) },
    ];
  };

  const functionMenu = (f: FunctionItem): MenuItem[] => [
    { label: "Show definition", onClick: () => openItem(f.schema, f.name, "function", { pinned: true, view: "definition" }) },
    { label: "Copy name", onClick: () => navigator.clipboard.writeText(f.name) },
    { label: "Copy call", onClick: () => navigator.clipboard.writeText(`SELECT ${quoteIdent(f.schema)}.${quoteIdent(f.name)}(${f.args ? "..." : ""});`) },
    { separator: true },
    { label: "Delete...", danger: true, onClick: () => setConfirm({ title: `Drop ${f.kind} ${f.name}?`, message: `${f.name}(${f.args})`, sql: `DROP ${f.kind === "procedure" ? "PROCEDURE" : f.kind === "aggregate" ? "AGGREGATE" : "FUNCTION"} ${quoteIdent(f.schema)}.${quoteIdent(f.name)}(${f.args})`, label: "Drop" }) },
  ];

  const sequenceMenu = (s: SequenceItem): MenuItem[] => [
    { label: "Show details", onClick: () => openItem(s.schema, s.name, "sequence", { pinned: true, view: "definition" }) },
    { label: "Copy name", onClick: () => navigator.clipboard.writeText(s.name) },
    { label: "Copy nextval()", onClick: () => navigator.clipboard.writeText(`nextval('${s.schema}.${s.name}'::regclass)`) },
    { separator: true },
    { label: "Delete...", danger: true, onClick: () => setConfirm({ title: `Drop sequence ${s.name}?`, message: "", sql: `DROP SEQUENCE ${quoteIdent(s.schema)}.${quoteIdent(s.name)}`, label: "Drop" }) },
  ];

  const Group = ({ id, label, count, children }: { id: Section; label: string; count: number; children: ReactNode }) => (
    <div>
      <div className="tree-row pl-1 text-dim font-semibold" onClick={() => setOpen({ ...open, [id]: !open[id] })}>
        {open[id] ? <ChevronDown size={13} /> : <ChevronRight size={13} />}
        <span className="name">{label}</span>
        <span className="text-xs opacity-70">{count}</span>
      </div>
      {open[id] && children}
    </div>
  );

  const isActive = (schemaName: string, name: string) => active?.type === "table" && active.schema === schemaName && active.name === name;

  return (
    <>
      <div className="flex items-center gap-1 p-2 shrink-0">
        <div className="relative flex-1">
          <Search size={12} className="absolute left-2 top-[7px] text-dim" />
          <input type="text" className="w-full !pl-6 !py-[3px]" placeholder="Search for item..." value={search} onChange={(e) => setSearch(e.target.value)} />
        </div>
        <button className="icon-btn" title="Filter options" onClick={() => setOpen({ tables: true, views: true, matviews: true, functions: true, sequences: true })}><SlidersHorizontal size={13} /></button>
      </div>
      <div className="flex-1 overflow-auto px-1 pb-2 min-h-0">
        {loading && !listing && <div className="p-3 text-dim flex items-center gap-2"><Loader2 size={13} className="animate-spin" /> Loading schema...</div>}
        {listing && (
          <>
            <Group id="tables" label="Tables" count={groups.tables.length}>
              {groups.tables.map((it) => (
                <ItemRow key={it.name} icon={kindIcon(it.kind)} name={it.name} selected={isActive(it.schema, it.name)}
                  hint={it.estimatedRows != null ? `~${formatCount(it.estimatedRows)} rows · ${formatBytes(it.totalSize)}${it.comment ? ` · ${it.comment}` : ""}` : it.comment ?? undefined}
                  onClick={() => openItem(it.schema, it.name, it.kind)} onDoubleClick={() => openItem(it.schema, it.name, it.kind, { pinned: true })}
                  onContextMenu={(e) => openMenu(e, itemMenu(it))} />
              ))}
            </Group>
            <Group id="views" label="Views" count={groups.views.length}>
              {groups.views.map((it) => (
                <ItemRow key={it.name} icon={kindIcon("view")} name={it.name} selected={isActive(it.schema, it.name)} hint={it.comment ?? undefined}
                  onClick={() => openItem(it.schema, it.name, it.kind)} onDoubleClick={() => openItem(it.schema, it.name, it.kind, { pinned: true })}
                  onContextMenu={(e) => openMenu(e, itemMenu(it))} />
              ))}
            </Group>
            <Group id="matviews" label="Materialized Views" count={groups.matviews.length}>
              {groups.matviews.map((it) => (
                <ItemRow key={it.name} icon={kindIcon("matview")} name={it.name} selected={isActive(it.schema, it.name)} hint={formatBytes(it.totalSize)}
                  onClick={() => openItem(it.schema, it.name, it.kind)} onDoubleClick={() => openItem(it.schema, it.name, it.kind, { pinned: true })}
                  onContextMenu={(e) => openMenu(e, itemMenu(it))} />
              ))}
            </Group>
            <Group id="functions" label="Functions" count={groups.functions.length}>
              {groups.functions.map((f) => (
                <ItemRow key={`${f.oid}`} icon={kindIcon("function")} name={f.name} hint={`${f.kind}(${f.args})`} selected={isActive(f.schema, f.name)}
                  onClick={() => openItem(f.schema, f.name, "function")} onDoubleClick={() => openItem(f.schema, f.name, "function", { pinned: true })}
                  onContextMenu={(e) => openMenu(e, functionMenu(f))} />
              ))}
            </Group>
            <Group id="sequences" label="Sequences" count={groups.sequences.length}>
              {groups.sequences.map((s) => (
                <ItemRow key={s.name} icon={kindIcon("sequence")} name={s.name} hint={s.lastValue != null ? `last ${s.lastValue}` : undefined} selected={isActive(s.schema, s.name)}
                  onClick={() => openItem(s.schema, s.name, "sequence")} onDoubleClick={() => openItem(s.schema, s.name, "sequence", { pinned: true })}
                  onContextMenu={(e) => openMenu(e, sequenceMenu(s))} />
              ))}
            </Group>
          </>
        )}
      </div>
      <div className="h-9 flex items-center gap-1 px-2 border-t border-border shrink-0">
        <button className="icon-btn" title="Create" onClick={(e) => openMenu(e, [
          { label: "Create new table", onClick: () => setCreateTable(true) },
          { label: "Create new schema", onClick: () => setNewSchema(true) },
          { label: "New SQL query", shortcut: "⌘T", onClick: () => openQueryTab() },
        ])}><Plus size={14} /></button>
        <select className="flex-1 !py-[3px] text-xs" value={schema} onChange={(e) => setSchema(e.target.value)} title="Schema">
          {schemas.map((s) => <option key={s} value={s}>{s}</option>)}
        </select>
      </div>
      <ContextMenu menu={menu} onClose={close} />
      {confirm && (
        <Confirm title={confirm.title} message={confirm.message} confirmLabel={confirm.label} danger onCancel={() => setConfirm(null)} onConfirm={async () => {
          const c = confirm; setConfirm(null);
          await run(c.sql, `${c.label} done`);
          const s = useApp.getState();
          s.tabs.filter((t) => t.type === "table" && c.sql.startsWith("DROP") && c.sql.includes(quoteIdent(t.name!))).forEach((t) => s.closeTab(t.id));
        }} />
      )}
      {newSchema && (
        <Confirm title="Create new schema" confirmLabel="Create" onCancel={() => setNewSchema(false)} onConfirm={async () => {
          const input = document.getElementById("new-schema-name") as HTMLInputElement | null;
          const n = input?.value.trim();
          setNewSchema(false);
          if (!n || !session) return;
          try {
            await ipc.execute(session.sessionId, `CREATE SCHEMA ${quoteIdent(n)}`);
            const schemasNow = await ipc.listSchemas(session.sessionId);
            useApp.setState({ schemas: schemasNow });
            setSchema(n);
          } catch (e) { toast("err", errorMessage(e)); }
        }}>
          <input id="new-schema-name" type="text" autoFocus className="w-full mono" placeholder="schema_name" />
        </Confirm>
      )}
    </>
  );
}

function ItemRow({ icon, name, hint, selected, onClick, onDoubleClick, onContextMenu }: {
  icon: ReactNode; name: string; hint?: string; selected?: boolean; onClick: () => void; onDoubleClick: () => void; onContextMenu: (e: React.MouseEvent) => void;
}) {
  return (
    <div className={`tree-row pl-6 ${selected ? "selected" : ""}`} title={hint ? `${name}\n${hint}` : name} onClick={onClick} onDoubleClick={onDoubleClick} onContextMenu={onContextMenu}>
      <span className="opacity-60 shrink-0">{icon}</span>
      <span className="name mono">{name}</span>
    </div>
  );
}

function QueriesPanel() {
  const favorites = useApp((s) => s.favorites);
  const session = useApp((s) => s.session);
  const openQueryTab = useApp((s) => s.openQueryTab);
  const loadFavorites = useApp((s) => s.loadFavorites);
  const [search, setSearch] = useState("");
  const { menu, open, close } = useContextMenu();
  const q = search.toLowerCase();
  const list = favorites.filter((f) => (!f.connectionId || f.connectionId === session?.connectionId) && (f.name.toLowerCase().includes(q) || f.sql.toLowerCase().includes(q)));
  return (
    <>
      <div className="p-2 shrink-0 relative">
        <Search size={12} className="absolute left-4 top-[15px] text-dim" />
        <input type="text" className="w-full !pl-6 !py-[3px]" placeholder="Search saved queries..." value={search} onChange={(e) => setSearch(e.target.value)} />
      </div>
      <div className="flex-1 overflow-auto px-1 min-h-0">
        {list.length === 0 && <div className="p-3 text-dim text-xs">No saved queries. Use ★ in a SQL tab to save one.</div>}
        {list.map((f) => (
          <div key={f.id} className="tree-row pl-2" title={f.sql} onDoubleClick={() => openQueryTab(f.sql, f.name)} onClick={() => openQueryTab(f.sql, f.name)}
            onContextMenu={(e) => open(e, [
              { label: "Open in new tab", onClick: () => openQueryTab(f.sql, f.name) },
              { label: "Copy SQL", onClick: () => navigator.clipboard.writeText(f.sql) },
              { separator: true },
              { label: "Delete", danger: true, onClick: async () => { await ipc.deleteFavorite(f.id); loadFavorites(); } },
            ])}>
            <Star size={12} className="opacity-60 shrink-0" />
            <span className="name">{f.name}</span>
            {f.connectionId && <span className="text-[10px] opacity-60">this db</span>}
          </div>
        ))}
      </div>
      <ContextMenu menu={menu} onClose={close} />
    </>
  );
}

function HistoryPanel() {
  const history = useApp((s) => s.history);
  const loadHistory = useApp((s) => s.loadHistory);
  const openQueryTab = useApp((s) => s.openQueryTab);
  const session = useApp((s) => s.session);
  const [search, setSearch] = useState("");
  const { menu, open, close } = useContextMenu();
  const grouped = useMemo(() => {
    const m = new Map<string, typeof history>();
    for (const h of history) { const k = relativeDay(h.at); m.set(k, [...(m.get(k) ?? []), h]); }
    return [...m.entries()];
  }, [history]);
  return (
    <>
      <div className="p-2 shrink-0 flex gap-1">
        <div className="relative flex-1">
          <Search size={12} className="absolute left-2 top-[7px] text-dim" />
          <input type="text" className="w-full !pl-6 !py-[3px]" placeholder="Search history..." value={search}
            onChange={(e) => { setSearch(e.target.value); loadHistory(e.target.value); }} />
        </div>
        <button className="icon-btn" title="Clear history" onClick={async () => { if (session && confirm("Clear history for this connection?")) { await ipc.clearHistory(session.connectionId); loadHistory(); } }}><Trash2 size={13} /></button>
      </div>
      <div className="flex-1 overflow-auto px-1 min-h-0">
        {history.length === 0 && <div className="p-3 text-dim text-xs">No queries yet.</div>}
        {grouped.map(([day, items]) => (
          <div key={day}>
            <div className="text-dim text-xs font-semibold px-2 py-1">{day}</div>
            {items.map((h) => (
              <div key={h.id} className="px-2 py-1 rounded hover:bg-tab-hover cursor-default" title={h.message ?? h.sql} onDoubleClick={() => openQueryTab(h.sql)}
                onContextMenu={(e) => open(e, [
                  { label: "Open in new tab", onClick: () => openQueryTab(h.sql) },
                  { label: "Copy SQL", onClick: () => navigator.clipboard.writeText(h.sql) },
                ])}>
                <div className="flex items-center gap-2 text-[11px] text-dim">
                  <span className={`conn-dot ${h.ok ? "" : ""}`} style={{ background: h.ok ? "var(--status-ok)" : "var(--status-err)", width: 7, height: 7 }} />
                  <span>{timeOf(h.at)}</span>
                  <span>{h.durationMs ? `${Math.round(h.durationMs)} ms` : ""}</span>
                  {h.rows != null && <span>{h.rows} rows</span>}
                </div>
                <div className="mono text-xs truncate-1 opacity-90">{h.sql.replace(/\s+/g, " ").slice(0, 200)}</div>
              </div>
            ))}
          </div>
        ))}
      </div>
      <ContextMenu menu={menu} onClose={close} />
    </>
  );
}

export { kindIcon };
