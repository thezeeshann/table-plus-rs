import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { ArrowRight, ArrowUp, ArrowDown, Key } from "lucide-react";
import type { CellValue, SortSpec } from "@/lib/types";

export interface GridColumn {
  name: string;
  type: string;
  pk?: boolean;
  fk?: boolean;
  nullable?: boolean;
  enumValues?: string[] | null;
}

export type RowStateKind = "normal" | "modified" | "inserted" | "deleted";

export interface CellPos { r: number; c: number }
export interface Selection { anchor: CellPos; focus: CellPos }

export interface DataGridProps {
  columns: GridColumn[];
  rows: (string | null)[][];
  rowOffset?: number;
  editable?: boolean;
  rowState?: (r: number) => RowStateKind;
  isDirty?: (r: number, c: number) => boolean;
  sort?: SortSpec[];
  onSort?: (column: string, additive: boolean) => void;
  onEdit?: (r: number, c: number, value: CellValue) => void;
  onContextMenu?: (e: React.MouseEvent, sel: Selection | null) => void;
  onFollowFk?: (r: number, c: number) => void;
  onSelectionChange?: (sel: Selection | null) => void;
  onDeleteRows?: (rows: number[]) => void;
  onHeaderContextMenu?: (e: React.MouseEvent, c: number) => void;
  emptyText?: string;
  /** Called with true while a cell editor is open (so global shortcuts can yield). */
  onEditingChange?: (editing: boolean) => void;
  /** Imperative "select and scroll to this cell" request; bump `seq` to re-trigger. */
  selectRequest?: { r: number; c: number; seq: number } | null;
}

const ROW_H = 24;
const HEADER_H = 26;
const NUM_W = 48;

export function selectionRange(sel: Selection) {
  return {
    r1: Math.min(sel.anchor.r, sel.focus.r), r2: Math.max(sel.anchor.r, sel.focus.r),
    c1: Math.min(sel.anchor.c, sel.focus.c), c2: Math.max(sel.anchor.c, sel.focus.c),
  };
}

export function selectionToText(sel: Selection, rows: (string | null)[][], columns: GridColumn[], format: "tsv" | "csv" | "json" | "sql" | "markdown" | "insert", table?: string): string {
  const { r1, r2, c1, c2 } = selectionRange(sel);
  const cols = columns.slice(c1, c2 + 1);
  const data = rows.slice(r1, r2 + 1).map((r) => r.slice(c1, c2 + 1));
  const esc = (v: string | null) => (v == null ? "" : v);
  switch (format) {
    case "tsv": return data.map((r) => r.map(esc).join("\t")).join("\n");
    case "csv": return [cols.map((c) => c.name).join(","), ...data.map((r) => r.map((v) => (v == null ? "" : /[",\n]/.test(v) ? `"${v.replace(/"/g, '""')}"` : v)).join(","))].join("\n");
    case "json": return JSON.stringify(data.map((r) => Object.fromEntries(cols.map((c, i) => [c.name, r[i]]))), null, 2);
    case "markdown": return [`| ${cols.map((c) => c.name).join(" | ")} |`, `|${cols.map(() => " --- ").join("|")}|`, ...data.map((r) => `| ${r.map((v) => (v ?? "NULL").replace(/\|/g, "\\|")).join(" | ")} |`)].join("\n");
    case "insert":
    case "sql": return data.map((r) => `INSERT INTO ${table ?? "table"} (${cols.map((c) => `"${c.name}"`).join(", ")}) VALUES (${r.map((v) => (v == null ? "NULL" : `'${v.replace(/'/g, "''")}'`)).join(", ")});`).join("\n");
  }
}

function initialWidth(col: GridColumn, rows: (string | null)[][], idx: number): number {
  let max = col.name.length;
  const sample = Math.min(rows.length, 60);
  for (let i = 0; i < sample; i++) {
    const v = rows[i]?.[idx];
    if (v != null) max = Math.max(max, Math.min(v.length, 48));
  }
  const header = (col.name.length + Math.min(col.type.length, 14)) * 7 + 30 + (col.pk ? 14 : 0);
  return Math.max(80, Math.min(360, Math.max(max * 7.4 + 24 + (col.fk ? 18 : 0), header)));
}

export function DataGrid(props: DataGridProps) {
  const { columns, rows, rowOffset = 0, editable, rowState, isDirty, sort, onSort, onEdit, onContextMenu, onFollowFk, onSelectionChange, onDeleteRows, onHeaderContextMenu, emptyText, onEditingChange, selectRequest } = props;
  const scrollRef = useRef<HTMLDivElement>(null);
  const [widths, setWidths] = useState<number[]>([]);
  const [sel, setSel] = useState<Selection | null>(null);
  const [editing, setEditing] = useState<{ r: number; c: number; value: string } | null>(null);
  const editorRef = useRef<HTMLTextAreaElement>(null);
  const colKey = columns.map((c) => c.name).join("\u0001");

  useLayoutEffect(() => {
    setWidths(columns.map((c, i) => initialWidth(c, rows, i)));
    setSel(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [colKey]);

  useEffect(() => { onSelectionChange?.(sel); }, [sel, onSelectionChange]);
  useEffect(() => { onEditingChange?.(!!editing); }, [editing, onEditingChange]);

  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW_H,
    overscan: 12,
  });

  const totalWidth = useMemo(() => NUM_W + widths.reduce((a, b) => a + b, 0), [widths]);
  const lefts = useMemo(() => { const out: number[] = []; let x = NUM_W; for (const w of widths) { out.push(x); x += w; } return out; }, [widths]);

  const scrollTo = useCallback((r: number, c: number) => {
    virtualizer.scrollToIndex(r, { align: "auto" });
    const el = scrollRef.current;
    if (!el) return;
    const left = lefts[c] ?? 0;
    const right = left + (widths[c] ?? 0);
    if (left - NUM_W < el.scrollLeft) el.scrollLeft = left - NUM_W;
    else if (right > el.scrollLeft + el.clientWidth) el.scrollLeft = right - el.clientWidth;
  }, [virtualizer, lefts, widths]);

  useEffect(() => {
    if (!selectRequest || !rows.length) return;
    const r = Math.min(rows.length - 1, selectRequest.r);
    const c = Math.min(columns.length - 1, selectRequest.c);
    setSel({ anchor: { r, c }, focus: { r, c } });
    requestAnimationFrame(() => { scrollTo(r, c); scrollRef.current?.focus(); });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectRequest?.seq, rows.length]);

  const startEdit = useCallback((r: number, c: number, initial?: string) => {
    if (!editable || !onEdit) return;
    if (rowState?.(r) === "deleted") return;
    const col = columns[c];
    if (col.enumValues) { /* enum uses picker in context menu; still allow typing */ }
    setEditing({ r, c, value: initial ?? rows[r]?.[c] ?? "" });
  }, [editable, onEdit, rows, columns, rowState]);

  const commitEdit = useCallback((move?: "down" | "right") => {
    if (!editing) return;
    const { r, c, value } = editing;
    const original = rows[r]?.[c] ?? null;
    if (value !== (original ?? "")) onEdit?.(r, c, { kind: "text", value });
    setEditing(null);
    if (move === "down" && r + 1 < rows.length) { setSel({ anchor: { r: r + 1, c }, focus: { r: r + 1, c } }); scrollTo(r + 1, c); }
    if (move === "right" && c + 1 < columns.length) { setSel({ anchor: { r, c: c + 1 }, focus: { r, c: c + 1 } }); scrollTo(r, c + 1); }
    scrollRef.current?.focus();
  }, [editing, rows, onEdit, columns.length, scrollTo]);

  useEffect(() => {
    if (editing && editorRef.current) {
      editorRef.current.focus();
      const len = editorRef.current.value.length;
      editorRef.current.setSelectionRange(len, len);
    }
  }, [editing?.r, editing?.c]);  // eslint-disable-line react-hooks/exhaustive-deps

  const copySelection = useCallback(() => {
    if (!sel) return;
    navigator.clipboard.writeText(selectionToText(sel, rows, columns, "tsv"));
  }, [sel, rows, columns]);

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (editing) return;
    const meta = e.metaKey || e.ctrlKey;
    if (meta && e.key.toLowerCase() === "c") { e.preventDefault(); copySelection(); return; }
    if (meta && e.key.toLowerCase() === "a") { e.preventDefault(); if (rows.length) setSel({ anchor: { r: 0, c: 0 }, focus: { r: rows.length - 1, c: columns.length - 1 } }); return; }
    if (!sel) {
      if (e.key.startsWith("Arrow") && rows.length) { setSel({ anchor: { r: 0, c: 0 }, focus: { r: 0, c: 0 } }); e.preventDefault(); }
      return;
    }
    const f = sel.focus;
    const moveTo = (r: number, c: number) => {
      r = Math.max(0, Math.min(rows.length - 1, r));
      c = Math.max(0, Math.min(columns.length - 1, c));
      setSel(e.shiftKey ? { anchor: sel.anchor, focus: { r, c } } : { anchor: { r, c }, focus: { r, c } });
      scrollTo(r, c);
      e.preventDefault();
    };
    switch (e.key) {
      case "ArrowUp": return moveTo(meta ? 0 : f.r - 1, f.c);
      case "ArrowDown": return moveTo(meta ? rows.length - 1 : f.r + 1, f.c);
      case "ArrowLeft": return moveTo(f.r, meta ? 0 : f.c - 1);
      case "ArrowRight": return moveTo(f.r, meta ? columns.length - 1 : f.c + 1);
      case "PageDown": return moveTo(f.r + 30, f.c);
      case "PageUp": return moveTo(f.r - 30, f.c);
      case "Home": return moveTo(f.r, 0);
      case "End": return moveTo(f.r, columns.length - 1);
      case "Tab": return moveTo(f.r, f.c + (e.shiftKey ? -1 : 1));
      case "Enter": case "F2": e.preventDefault(); return startEdit(f.r, f.c);
      case "Backspace": case "Delete":
        if (meta) { e.preventDefault(); const { r1, r2 } = selectionRange(sel); onDeleteRows?.(Array.from({ length: r2 - r1 + 1 }, (_, i) => r1 + i)); }
        else if (editable) { e.preventDefault(); const { r1, r2, c1, c2 } = selectionRange(sel); for (let r = r1; r <= r2; r++) for (let c = c1; c <= c2; c++) onEdit?.(r, c, { kind: "null" }); }
        return;
      default:
        if (!meta && !e.altKey && e.key.length === 1 && editable) { e.preventDefault(); startEdit(f.r, f.c, e.key); }
    }
  };

  const onResizeStart = (e: React.MouseEvent, c: number) => {
    e.preventDefault(); e.stopPropagation();
    const startX = e.clientX; const startW = widths[c];
    const move = (ev: MouseEvent) => setWidths((w) => w.map((x, i) => (i === c ? Math.max(40, startW + ev.clientX - startX) : x)));
    const up = () => { window.removeEventListener("mousemove", move); window.removeEventListener("mouseup", up); };
    window.addEventListener("mousemove", move); window.addEventListener("mouseup", up);
  };

  const range = sel ? selectionRange(sel) : null;
  const items = virtualizer.getVirtualItems();

  return (
    <div
      ref={scrollRef}
      className="flex-1 overflow-auto outline-none relative min-h-0 bg-content"
      tabIndex={0}
      onKeyDown={onKeyDown}
      onContextMenu={(e) => { if (!(e.target as HTMLElement).closest(".grid-header")) onContextMenu?.(e, sel); }}
    >
      <div style={{ width: totalWidth, position: "relative", minWidth: "100%" }}>
        {/* header */}
        <div className="sticky top-0 z-20 flex" style={{ height: HEADER_H, width: totalWidth, minWidth: "100%" }}>
          <div className="grid-header sticky left-0 z-30 text-center text-dim" style={{ width: NUM_W, flex: "none", background: "var(--grid-header)" }} onClick={() => rows.length && setSel({ anchor: { r: 0, c: 0 }, focus: { r: rows.length - 1, c: columns.length - 1 } })}>#</div>
          {columns.map((col, c) => {
            const s = sort?.find((x) => x.column === col.name);
            return (
              <div
                key={col.name + c}
                className="grid-header flex items-center gap-1 cursor-default select-none"
                style={{ width: widths[c], flex: "none" }}
                title={`${col.name} · ${col.type}`}
                onClick={(e) => onSort?.(col.name, e.shiftKey)}
                onContextMenu={(e) => { e.preventDefault(); onHeaderContextMenu?.(e, c); }}
              >
                {col.pk && <Key size={11} className="text-[#f9b746] shrink-0" />}
                <span className="truncate-1 flex-1">{col.name}</span>
                {s && (s.direction === "asc" ? <ArrowUp size={11} className="shrink-0" /> : <ArrowDown size={11} className="shrink-0" />)}
                <span className="text-[10px] text-dim font-normal truncate-1 max-w-[80px]">{col.type}</span>
                <div className="resizer" onMouseDown={(e) => onResizeStart(e, c)} onClick={(e) => e.stopPropagation()} />
              </div>
            );
          })}
          <div className="grid-header flex-1" />
        </div>
        {rows.length === 0 && (
          <div className="absolute left-0 right-0 top-[60px] text-center text-dim">{emptyText ?? "No rows"}</div>
        )}
        {/* body */}
        <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
          {items.map((vi) => {
            const r = vi.index;
            const row = rows[r];
            const state = rowState?.(r) ?? "normal";
            return (
              <div
                key={vi.key}
                className={`grid-row flex absolute left-0 ${state}`}
                style={{ top: vi.start, height: ROW_H, width: totalWidth, minWidth: "100%" }}
              >
                <div className="grid-cell sticky left-0 z-10 text-right text-dim" style={{ width: NUM_W, flex: "none", background: "var(--grid-header)" }}
                  onMouseDown={(e) => { e.preventDefault(); setSel(e.shiftKey && sel ? { anchor: sel.anchor, focus: { r, c: columns.length - 1 } } : { anchor: { r, c: 0 }, focus: { r, c: columns.length - 1 } }); scrollRef.current?.focus(); }}>
                  {rowOffset + r + 1}
                </div>
                {columns.map((col, c) => {
                  const v = row[c];
                  const selected = range && r >= range.r1 && r <= range.r2 && c >= range.c1 && c <= range.c2;
                  const isFocus = sel && sel.focus.r === r && sel.focus.c === c;
                  const isEditing = editing && editing.r === r && editing.c === c;
                  const dirty = isDirty?.(r, c);
                  return (
                    <div
                      key={c}
                      className={`grid-cell relative ${v == null ? "null" : ""} ${isFocus ? "selected" : ""} ${selected && !isFocus ? "in-range" : ""} ${dirty ? "dirty" : ""}`}
                      style={{ width: widths[c], flex: "none" }}
                      onMouseDown={(e) => {
                        if (e.button === 2 && selected) return;
                        if (e.button !== 0 && e.button !== 2) return;
                        setSel(e.shiftKey && sel ? { anchor: sel.anchor, focus: { r, c } } : { anchor: { r, c }, focus: { r, c } });
                        if (!isEditing) { e.preventDefault(); scrollRef.current?.focus(); }
                      }}
                      onDoubleClick={() => startEdit(r, c)}
                      title={v == null ? "NULL" : v.length > 60 ? v.slice(0, 2000) : undefined}
                    >
                      {isEditing ? (
                        <textarea
                          ref={editorRef}
                          className="absolute inset-0 w-full h-full !rounded-none !border-accent mono resize-none !px-[7px] !py-0 leading-6 z-20 bg-[#0b2127]"
                          value={editing.value}
                          onChange={(e) => setEditing({ ...editing, value: e.target.value })}
                          onBlur={() => commitEdit()}
                          onKeyDown={(e) => {
                            e.stopPropagation();
                            if (e.key === "Enter" && !e.shiftKey && !e.altKey) { e.preventDefault(); commitEdit("down"); }
                            else if (e.key === "Tab") { e.preventDefault(); commitEdit("right"); }
                            else if (e.key === "Escape") { e.preventDefault(); setEditing(null); scrollRef.current?.focus(); }
                          }}
                        />
                      ) : (
                        <>
                          {v == null ? "NULL" : v === "" ? <span className="text-dim italic">empty</span> : v.length > 400 ? v.slice(0, 400) : v}
                          {col.fk && v != null && onFollowFk && (
                            <button className="absolute right-1 top-1 w-4 h-4 rounded flex items-center justify-center text-dim hover:text-white hover:bg-tab-selected" title="Open referenced row" onMouseDown={(e) => e.stopPropagation()} onClick={(e) => { e.stopPropagation(); onFollowFk(r, c); }}>
                              <ArrowRight size={11} />
                            </button>
                          )}
                        </>
                      )}
                    </div>
                  );
                })}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
