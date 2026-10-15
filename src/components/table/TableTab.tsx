import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ChevronLeft, ChevronRight, Plus, Filter, Columns3, RotateCw, Loader2 } from "lucide-react";
import { useApp, type Tab, type TableView } from "@/store/app";
import { ipc, errorMessage } from "@/lib/ipc";
import type { CellValue, FetchRowsResponse, FilterSpec, RowChange, SortSpec, TableStructure } from "@/lib/types";
import { DataGrid, selectionRange, selectionToText, type GridColumn, type Selection } from "./DataGrid";
import { FilterBar } from "./FilterBar";
import { StructureView } from "./StructureView";
import { IndexView } from "./IndexView";
import { DefinitionView } from "./DefinitionView";
import { ContextMenu, useContextMenu, type MenuItem } from "@/components/common/ContextMenu";
import { Modal } from "@/components/common/Modal";
import { formatCount, formatDuration, mod } from "@/lib/format";
import { quoteIdent } from "@/lib/sql";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";

interface InsertedRow { id: number; values: Record<string, CellValue>; display: (string | null)[] }
let insertSeq = 1;

export function TableTab({ tab }: { tab: Tab; active: boolean }) {
  const session = useApp((s) => s.session)!;
  const toast = useApp((s) => s.toast);
  const setDirty = useApp((s) => s.setDirty);
  const register = useApp((s) => s.registerTabActions);
  const updateTab = useApp((s) => s.updateTab);
  const openItem = useApp((s) => s.openItem);
  const openQueryTab = useApp((s) => s.openQueryTab);
  const { schema, name, kind } = tab as Required<Pick<Tab, "schema" | "name" | "kind">> & Tab;
  const view: TableView = tab.view ?? "data";
  const setView = useCallback((v: TableView) => updateTab(tab.id, { view: v }), [tab.id, updateTab]);

  const [structure, setStructure] = useState<TableStructure | null>(null);
  const [data, setData] = useState<FetchRowsResponse | null>(null);
  // Starts true for relations: the first fetch is fired from an effect, and one frame of "Table is empty" would flash.
  const [loading, setLoading] = useState(kind === "table" || kind === "partitioned" || kind === "view" || kind === "matview" || kind === "foreign");
  const setTabBusy = useApp((s) => s.setTabBusy);
  const [error, setError] = useState<string | null>(null);
  const [limit, setLimit] = useState(session.limitRowsReturned || 300);
  const [offset, setOffset] = useState(0);
  const [sort, setSort] = useState<SortSpec[]>([]);
  const [filters, setFilters] = useState<FilterSpec[]>(tab.initialFilter ? [{ column: tab.initialFilter.column, op: tab.initialFilter.value == null ? "is_null" : "eq", value: tab.initialFilter.value, enabled: true }] : []);
  const [appliedFilters, setAppliedFilters] = useState<FilterSpec[]>(filters);
  const [showFilters, setShowFilters] = useState(!!tab.initialFilter);
  const [quick, setQuick] = useState("");
  const [hidden, setHidden] = useState<Set<string>>(new Set());
  const [showColumns, setShowColumns] = useState(false);
  const [edits, setEdits] = useState<Map<number, Record<string, CellValue>>>(new Map());
  const [inserted, setInserted] = useState<InsertedRow[]>([]);
  const [deleted, setDeleted] = useState<Set<number>>(new Set());
  const [preview, setPreview] = useState<string | null>(null);
  const [sel, setSel] = useState<Selection | null>(null);
  const [editingCell, setEditingCell] = useState(false);
  const [selectReq, setSelectReq] = useState<{ r: number; c: number; seq: number } | null>(null);
  const { menu, open: openMenu, close: closeMenu } = useContextMenu();
  const reloadToken = useRef(0);

  const isRelation = kind === "table" || kind === "partitioned" || kind === "view" || kind === "matview" || kind === "foreign";
  const editable = kind === "table" || kind === "partitioned";

  const loadStructure = useCallback(async () => {
    if (!isRelation) return;
    try { setStructure(await ipc.tableStructure(session.sessionId, schema, name, kind)); } catch (e) { toast("err", errorMessage(e)); }
  }, [session.sessionId, schema, name, kind, isRelation, toast]);

  const load = useCallback(async (opts?: { offset?: number; limit?: number; sort?: SortSpec[]; filters?: FilterSpec[]; quick?: string }) => {
    if (!isRelation) return;
    const token = ++reloadToken.current;
    setLoading(true); setError(null);
    try {
      const res = await ipc.fetchRows({
        sessionId: session.sessionId, schema, table: name, kind,
        limit: opts?.limit ?? limit, offset: opts?.offset ?? offset,
        sort: opts?.sort ?? sort, filters: opts?.filters ?? appliedFilters, quickFilter: opts?.quick ?? quick, countMode: "auto",
      });
      if (token !== reloadToken.current) return;
      setData(res);
      setEdits(new Map()); setInserted([]); setDeleted(new Set());
    } catch (e) {
      if (token !== reloadToken.current) return;
      setError(errorMessage(e));
    } finally { if (token === reloadToken.current) setLoading(false); }
  }, [session.sessionId, schema, name, kind, limit, offset, sort, appliedFilters, quick, isRelation]);

  useEffect(() => { loadStructure(); load(); }, [schema, name]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => { setTabBusy(tab.id, loading); }, [loading, tab.id, setTabBusy]);
  useEffect(() => () => setTabBusy(tab.id, false), [tab.id, setTabBusy]);

  const pendingCount = edits.size + inserted.length + deleted.size;
  useEffect(() => { setDirty(tab.id, pendingCount > 0); }, [pendingCount, tab.id, setDirty]);

  const pk = structure?.primaryKey ?? [];
  const columns: GridColumn[] = useMemo(() => (data?.columns ?? []).filter((c) => !hidden.has(c.name)).map((c) => {
    const ci = structure?.columns.find((x) => x.name === c.name);
    return { name: c.name, type: ci?.formatType ?? c.typeName, pk: !!ci?.isPrimaryKey, fk: !!ci?.foreignKey, nullable: ci?.nullable, enumValues: ci?.enumValues };
  }), [data, structure, hidden]);
  const colIndex = useMemo(() => (data?.columns ?? []).map((c) => c.name), [data]);
  const visibleIdx = useMemo(() => columns.map((c) => colIndex.indexOf(c.name)), [columns, colIndex]);

  const baseRows = data?.rows ?? [];
  const displayRows = useMemo(() => {
    const out: (string | null)[][] = baseRows.map((row, r) => {
      const e = edits.get(r);
      return visibleIdx.map((ci) => {
        const col = colIndex[ci];
        const ev = e?.[col];
        if (ev) return ev.kind === "text" ? ev.value : ev.kind === "null" ? null : ev.kind === "expr" ? ev.value : null;
        return row[ci];
      });
    });
    for (const ins of inserted) out.push(visibleIdx.map((ci) => ins.display[ci]));
    return out;
  }, [baseRows, edits, inserted, visibleIdx, colIndex]);

  const rowState = useCallback((r: number) => {
    if (r >= baseRows.length) return "inserted" as const;
    if (deleted.has(r)) return "deleted" as const;
    if (edits.has(r)) return "modified" as const;
    return "normal" as const;
  }, [baseRows.length, deleted, edits]);
  const isDirtyCell = useCallback((r: number, c: number) => r < baseRows.length && !!edits.get(r)?.[columns[c]?.name], [edits, columns, baseRows.length]);

  const onEdit = useCallback((r: number, c: number, value: CellValue) => {
    const col = columns[c]?.name; if (!col) return;
    if (r >= baseRows.length) {
      const i = r - baseRows.length;
      setInserted((list) => list.map((ins, j) => {
        if (j !== i) return ins;
        const display = [...ins.display];
        display[colIndex.indexOf(col)] = value.kind === "text" || value.kind === "expr" ? value.value : null;
        const values = { ...ins.values };
        if (value.kind === "default") delete values[col]; else values[col] = value;
        return { ...ins, values, display };
      }));
      return;
    }
    setEdits((m) => {
      const next = new Map(m);
      const cur = { ...(next.get(r) ?? {}) };
      const original = baseRows[r][colIndex.indexOf(col)];
      const same = (value.kind === "text" && value.value === original) || (value.kind === "null" && original == null);
      if (same) delete cur[col]; else cur[col] = value;
      if (Object.keys(cur).length) next.set(r, cur); else next.delete(r);
      return next;
    });
  }, [columns, baseRows, colIndex]);

  const addRow = useCallback(() => {
    if (!editable || !data) return;
    const display = colIndex.map((c) => { const ci = structure?.columns.find((x) => x.name === c); return ci?.default ? ci.default : null; });
    setInserted((l) => { setSelectReq({ r: baseRows.length + l.length, c: 0, seq: insertSeq }); return [...l, { id: insertSeq++, values: {}, display }]; });
  }, [editable, data, colIndex, structure, baseRows.length]);

  const duplicateRows = useCallback(() => {
    if (!editable || !sel) return;
    const { r1, r2 } = selectionRange(sel);
    const rows = displayRows.slice(r1, r2 + 1);
    const skip = new Set((structure?.columns ?? []).filter((c) => c.isIdentity || c.isGenerated).map((c) => c.name));
    setInserted((l) => [...l, ...rows.map((row) => {
      const values: Record<string, CellValue> = {};
      const display: (string | null)[] = colIndex.map(() => null);
      columns.forEach((col, i) => {
        if (skip.has(col.name)) return;
        const v = row[i];
        values[col.name] = v == null ? { kind: "null" } : { kind: "text", value: v };
        display[colIndex.indexOf(col.name)] = v;
      });
      return { id: insertSeq++, values, display };
    })]);
  }, [editable, sel, displayRows, structure, columns, colIndex]);

  const deleteRows = useCallback((rows?: number[]) => {
    if (!editable) return;
    const list = rows ?? (sel ? (() => { const { r1, r2 } = selectionRange(sel); return Array.from({ length: r2 - r1 + 1 }, (_, i) => r1 + i); })() : []);
    if (!list.length) return;
    const insIdx = list.filter((r) => r >= baseRows.length).map((r) => r - baseRows.length);
    if (insIdx.length) setInserted((l) => l.filter((_, i) => !insIdx.includes(i)));
    setDeleted((d) => { const n = new Set(d); for (const r of list) if (r < baseRows.length) { if (n.has(r)) n.delete(r); else n.add(r); } return n; });
  }, [editable, sel, baseRows.length]);

  const buildChanges = useCallback((): RowChange[] => {
    const changes: RowChange[] = [];
    const identity = (r: number) => {
      const row = baseRows[r];
      const keyValues: Record<string, string | null> = {};
      if (pk.length) for (const k of pk) keyValues[k] = row[colIndex.indexOf(k)] ?? null;
      else colIndex.forEach((c, i) => { keyValues[c] = row[i]; });
      return { keyValues, ctid: data?.ctids?.[r] ?? null };
    };
    for (const r of deleted) changes.push({ kind: "delete", identity: identity(r) });
    for (const [r, values] of edits) if (!deleted.has(r)) changes.push({ kind: "update", identity: identity(r), values });
    for (const ins of inserted) changes.push({ kind: "insert", values: ins.values });
    return changes;
  }, [baseRows, pk, colIndex, data, deleted, edits, inserted]);

  const commit = useCallback(async () => {
    if (!pendingCount) return;
    try {
      const stmts = await ipc.applyChanges({ sessionId: session.sessionId, schema, table: name, changes: buildChanges(), primaryKey: pk });
      toast("ok", `${stmts.length} statement${stmts.length === 1 ? "" : "s"} committed`);
      load();
    } catch (e) { toast("err", errorMessage(e)); }
  }, [pendingCount, session.sessionId, schema, name, buildChanges, pk, toast, load]);

  const discard = useCallback(() => { setEdits(new Map()); setInserted([]); setDeleted(new Set()); }, []);
  const previewSql = useCallback(async () => {
    if (!pendingCount) return;
    try { setPreview((await ipc.previewChanges({ sessionId: session.sessionId, schema, table: name, changes: buildChanges(), primaryKey: pk })).join(";\n") + ";"); } catch (e) { toast("err", errorMessage(e)); }
  }, [pendingCount, session.sessionId, schema, name, buildChanges, pk, toast]);

  const exportData = useCallback(async () => {
    if (!data) return;
    const path = await saveDialog({ defaultPath: `${name}.csv`, title: "Export rows", filters: [{ name: "CSV", extensions: ["csv"] }, { name: "JSON", extensions: ["json"] }, { name: "SQL", extensions: ["sql"] }, { name: "Markdown", extensions: ["md"] }] });
    if (!path) return;
    const ext = path.split(".").pop()?.toLowerCase();
    const format = ext === "json" ? "json" : ext === "sql" ? "sql" : ext === "md" ? "markdown" : "csv";
    try {
      const n = await ipc.exportRows({ path, format, columns: columns.map((c) => c.name), rows: displayRows, table: `${quoteIdent(schema)}.${quoteIdent(name)}`, header: true });
      toast("ok", `Exported ${n} rows`);
    } catch (e) { toast("err", errorMessage(e)); }
  }, [data, name, schema, columns, displayRows, toast]);

  useEffect(() => {
    register(tab.id, {
      commit, discard, preview: previewSql, addRow, duplicateRows, deleteRows: () => deleteRows(), reload: () => { load(); loadStructure(); },
      exportData, setView, find: () => setShowFilters(true),
    });
  }, [tab.id, register, commit, discard, previewSql, addRow, duplicateRows, deleteRows, load, loadStructure, exportData, setView]);

  const onSort = (column: string, additive: boolean) => {
    const cur = sort.find((s) => s.column === column);
    let next: SortSpec[];
    if (!cur) next = additive ? [...sort, { column, direction: "asc" }] : [{ column, direction: "asc" }];
    else if (cur.direction === "asc") next = sort.map((s) => (s.column === column ? { ...s, direction: "desc" as const } : s));
    else next = sort.filter((s) => s.column !== column);
    if (!additive && cur?.direction !== "asc" && cur) next = next.filter((s) => s.column === column);
    setSort(next); setOffset(0); load({ sort: next, offset: 0 });
  };
  const applyFilters = () => { setAppliedFilters(filters); setOffset(0); load({ filters, offset: 0 }); };
  const page = (dir: number) => { const o = Math.max(0, offset + dir * limit); setOffset(o); load({ offset: o }); };

  const followFk = (r: number, c: number) => {
    const col = columns[c]; const fk = structure?.foreignKeys.find((f) => f.childColumns.includes(col.name));
    if (!fk) return;
    const parentCol = fk.parentColumns[fk.childColumns.indexOf(col.name)];
    openItem(fk.parentSchema, fk.parentTable, "table", { pinned: true, filter: { column: parentCol, value: displayRows[r][c] } });
  };

  const cellMenu = (e: React.MouseEvent, s: Selection | null) => {
    const items: MenuItem[] = [];
    if (s) {
      const { r1, r2 } = selectionRange(s);
      const rowsN = r2 - r1 + 1;
      const col = columns[s.focus.c];
      const value = displayRows[s.focus.r]?.[s.focus.c];
      items.push(
        { label: "Copy", shortcut: `${mod}C`, onClick: () => navigator.clipboard.writeText(selectionToText(s, displayRows, columns, "tsv")) },
        { label: "Copy as CSV", onClick: () => navigator.clipboard.writeText(selectionToText(s, displayRows, columns, "csv")) },
        { label: "Copy as JSON", onClick: () => navigator.clipboard.writeText(selectionToText(s, displayRows, columns, "json")) },
        { label: "Copy as INSERT", onClick: () => navigator.clipboard.writeText(selectionToText(s, displayRows, columns, "insert", `${quoteIdent(schema)}.${quoteIdent(name)}`)) },
        { label: "Copy as Markdown", onClick: () => navigator.clipboard.writeText(selectionToText(s, displayRows, columns, "markdown")) },
        { separator: true },
        { label: `Filter: ${col?.name} = ${value == null ? "NULL" : JSON.stringify(value).slice(0, 30)}`, onClick: () => { const f: FilterSpec[] = [...appliedFilters, { column: col.name, op: value == null ? "is_null" : "eq", value, enabled: true }]; setFilters(f); setAppliedFilters(f); setShowFilters(true); setOffset(0); load({ filters: f, offset: 0 }); } },
      );
      if (col?.fk && value != null) items.push({ label: "Open referenced row", onClick: () => followFk(s.focus.r, s.focus.c) });
      if (editable) {
        items.push({ separator: true },
          { label: "Set NULL", shortcut: "⌫", onClick: () => { const { r1, r2, c1, c2 } = selectionRange(s); for (let r = r1; r <= r2; r++) for (let c = c1; c <= c2; c++) onEdit(r, c, { kind: "null" }); } },
          { label: "Set empty string", onClick: () => onEdit(s.focus.r, s.focus.c, { kind: "text", value: "" }) },
          ...(s.focus.r >= baseRows.length ? [{ label: "Set DEFAULT", onClick: () => onEdit(s.focus.r, s.focus.c, { kind: "default" }) }] : []),
          { label: "Set expression (now(), gen_random_uuid()...)", onClick: () => { const v = prompt("SQL expression", "now()"); if (v) onEdit(s.focus.r, s.focus.c, { kind: "expr", value: v }); } },
        );
        if (col?.enumValues?.length) items.push({ separator: true }, ...col.enumValues.slice(0, 20).map((ev) => ({ label: `→ ${ev}`, onClick: () => onEdit(s.focus.r, s.focus.c, { kind: "text", value: ev }) })));
        if (col && (col.type === "boolean" || col.type === "bool")) items.push({ separator: true }, { label: "→ true", onClick: () => onEdit(s.focus.r, s.focus.c, { kind: "text", value: "true" }) }, { label: "→ false", onClick: () => onEdit(s.focus.r, s.focus.c, { kind: "text", value: "false" }) });
        items.push({ separator: true },
          { label: `Duplicate ${rowsN > 1 ? `${rowsN} rows` : "row"}`, shortcut: `${mod}D`, onClick: duplicateRows },
          { label: `Delete ${rowsN > 1 ? `${rowsN} rows` : "row"}`, shortcut: `${mod}⌫`, danger: true, onClick: () => deleteRows() },
        );
      }
      items.push({ separator: true });
    }
    items.push({ label: "Add row", shortcut: `${mod}I`, disabled: !editable, onClick: addRow }, { label: "Reload", shortcut: `${mod}R`, onClick: () => load() }, { label: "Export...", shortcut: `${mod}⇧E`, onClick: exportData });
    openMenu(e, items);
  };

  const headerMenu = (e: React.MouseEvent, c: number) => {
    const col = columns[c];
    openMenu(e, [
      { label: "Sort ascending", onClick: () => { const n: SortSpec[] = [{ column: col.name, direction: "asc" }]; setSort(n); load({ sort: n }); } },
      { label: "Sort descending", onClick: () => { const n: SortSpec[] = [{ column: col.name, direction: "desc" }]; setSort(n); load({ sort: n }); } },
      { label: "Remove sort", disabled: !sort.length, onClick: () => { setSort([]); load({ sort: [] }); } },
      { separator: true },
      { label: "Filter with column", onClick: () => { setFilters([...filters, { column: col.name, op: "eq", value: "", enabled: true }]); setShowFilters(true); } },
      { label: "Copy column name", onClick: () => navigator.clipboard.writeText(col.name) },
      { label: "Copy all column values", onClick: () => navigator.clipboard.writeText(displayRows.map((r) => r[c] ?? "").join("\n")) },
      { separator: true },
      { label: "Hide this column", onClick: () => setHidden(new Set([...hidden, col.name])) },
      { label: "Show all columns", disabled: !hidden.size, onClick: () => setHidden(new Set()) },
      { separator: true },
      { label: "Open structure", onClick: () => setView("structure") },
    ]);
  };

  const total = data?.total ?? null;
  const from = baseRows.length ? offset + 1 : 0;
  const to = offset + baseRows.length;
  const rangeText = total != null ? `${formatCount(from)}-${formatCount(to)} of ${data?.totalIsEstimate ? "~" : ""}${formatCount(total)} rows` : `${formatCount(from)}-${formatCount(to)} rows`;

  if (!isRelation) return <DefinitionView tab={tab} />;

  return (
    <div className="flex-1 flex flex-col min-h-0">
      {view === "data" && (
        <>
          {showFilters && <FilterBar columns={colIndex} filters={filters} onChange={setFilters} onApply={applyFilters} />}
          {error && <div className="px-3 py-2 bg-err text-white text-xs selectable">{error}</div>}
          <DataGrid
            columns={columns} rows={displayRows} rowOffset={offset} editable={editable}
            rowState={rowState} isDirty={isDirtyCell} sort={sort} onSort={onSort} onEdit={onEdit}
            onContextMenu={cellMenu} onHeaderContextMenu={headerMenu} onFollowFk={followFk} onSelectionChange={setSel}
            onDeleteRows={(rows) => deleteRows(rows)} onEditingChange={setEditingCell} selectRequest={selectReq}
            emptyText={loading ? "Loading..." : appliedFilters.length || quick ? "No rows match the filter" : "Table is empty"}
          />
          {pendingCount > 0 && (
            <div className="flex items-center gap-2 px-3 h-8 bg-warn text-white text-xs shrink-0">
              <span className="flex-1">{pendingCount} pending change{pendingCount === 1 ? "" : "s"} · {editingCell ? "editing" : "not saved"}</span>
              <button className="btn !py-0 !bg-[var(--app-bg)]" onClick={previewSql}>Preview ({mod}⇧P)</button>
              <button className="btn !py-0 !bg-[var(--app-bg)]" onClick={discard}>Discard</button>
              <button className="btn btn-primary !py-0" onClick={commit}>Commit ({mod}S)</button>
            </div>
          )}
        </>
      )}
      {view === "structure" && structure && <StructureView tab={tab} structure={structure} onChanged={() => { loadStructure(); load(); }} />}
      {view === "index" && structure && <IndexView tab={tab} structure={structure} onChanged={loadStructure} />}
      {view === "definition" && <DefinitionView tab={tab} />}
      {(view === "structure" || view === "index") && !structure && <div className="flex-1 flex items-center justify-center text-dim"><Loader2 className="animate-spin mr-2" size={14} /> Loading structure…</div>}

      <div className="h-9 flex items-center gap-2 px-2 border-t border-border bg-app shrink-0">
        <div className="segmented">
          {(["data", "structure", "index", "definition"] as TableView[]).map((v) => (
            <button key={v} className={view === v ? "active" : ""} onClick={() => setView(v)}>{v === "data" ? "Data" : v === "structure" ? "Structure" : v === "index" ? "Index" : "DDL"}</button>
          ))}
        </div>
        {view === "data" && (
          <>
            <button className="btn !py-[2px]" disabled={!editable} onClick={addRow} title={`Add row (${mod}I)`}><Plus size={13} /> Row</button>
            <input type="text" className="w-44 !py-[2px] text-xs" placeholder="Quick filter (all columns)" value={quick} onChange={(e) => setQuick(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") { setOffset(0); load({ quick, offset: 0 }); } }} />
            <div className="flex-1 text-center text-xs text-dim flex items-center justify-center gap-2">
              {loading && <Loader2 size={12} className="animate-spin" />}
              <span>{rangeText}</span>
              {data && <span className="opacity-60">· {formatDuration(data.durationMs)}</span>}
              {!editable && <span className="opacity-60">· read-only</span>}
              {editable && !pk.length && <span className="text-warn" title="Edits use the row's physical location (ctid)">· no primary key</span>}
            </div>
            <button className={`btn !py-[2px] ${showFilters ? "btn-primary" : ""}`} onClick={() => setShowFilters((v) => !v)}><Filter size={13} /> Filters{appliedFilters.length ? ` (${appliedFilters.filter((f) => f.enabled).length})` : ""}</button>
            <div className="relative">
              <button className={`btn !py-[2px] ${hidden.size ? "btn-primary" : ""}`} onClick={() => setShowColumns((v) => !v)}><Columns3 size={13} /> Columns</button>
              {showColumns && (
                <div className="absolute bottom-8 right-0 z-50 menu max-h-80 overflow-auto" onMouseLeave={() => setShowColumns(false)}>
                  {colIndex.map((c) => (
                    <label key={c} className="menu-item !justify-start gap-2"><input type="checkbox" checked={!hidden.has(c)} onChange={(e) => { const n = new Set(hidden); if (e.target.checked) n.delete(c); else n.add(c); setHidden(n); }} /><span className="mono">{c}</span></label>
                  ))}
                </div>
              )}
            </div>
            <select className="!py-[2px] text-xs w-20" value={limit} onChange={(e) => { const l = Number(e.target.value); setLimit(l); setOffset(0); load({ limit: l, offset: 0 }); }}>
              {[50, 100, 300, 500, 1000, 5000].map((n) => <option key={n} value={n}>{n}</option>)}
            </select>
            <button className="icon-btn" disabled={offset === 0 || loading} onClick={() => page(-1)}><ChevronLeft size={14} /></button>
            <button className="icon-btn" disabled={loading || (total != null && !data?.totalIsEstimate && to >= total) || baseRows.length < limit} onClick={() => page(1)}><ChevronRight size={14} /></button>
            <button className="icon-btn" title={`Reload (${mod}R)`} onClick={() => load()}><RotateCw size={13} /></button>
          </>
        )}
        {view !== "data" && <div className="flex-1" />}
        {view !== "data" && <button className="btn !py-[2px]" onClick={() => openQueryTab(`SELECT * FROM ${quoteIdent(schema)}.${quoteIdent(name)}\nLIMIT 100;`, name)}>Query</button>}
      </div>
      <ContextMenu menu={menu} onClose={closeMenu} />
      {preview && (
        <Modal title="Preview SQL" onClose={() => setPreview(null)} width={720} footer={<>
          <button className="btn" onClick={() => { navigator.clipboard.writeText(preview); }}>Copy</button>
          <button className="btn" onClick={() => setPreview(null)}>Close</button>
          <button className="btn btn-primary" onClick={() => { setPreview(null); commit(); }}>Commit</button>
        </>}>
          <pre className="mono selectable whitespace-pre-wrap text-xs max-h-[60vh] overflow-auto">{preview}</pre>
        </Modal>
      )}
    </div>
  );
}
