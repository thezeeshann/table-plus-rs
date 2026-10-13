import { useMemo, useState } from "react";
import { AlertTriangle, CheckCircle2, Download, Copy } from "lucide-react";
import { DataGrid, selectionToText, type Selection } from "@/components/table/DataGrid";
import { ContextMenu, useContextMenu } from "@/components/common/ContextMenu";
import type { QueryResult } from "@/lib/types";
import { formatCount, formatDuration, mod } from "@/lib/format";
import { useApp } from "@/store/app";
import { ipc, errorMessage } from "@/lib/ipc";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";

export function ResultsPane({ results, running }: { results: QueryResult[]; running: boolean }) {
  const [active, setActive] = useState(0);
  const [sel, setSel] = useState<Selection | null>(null);
  const toast = useApp((s) => s.toast);
  const { menu, open, close } = useContextMenu();
  const idx = Math.min(active, Math.max(0, results.length - 1));
  const r = results[idx];
  const columns = useMemo(() => (r?.columns ?? []).map((c) => ({ name: c.name, type: c.typeName })), [r]);

  const exportResult = async (format: "csv" | "json" | "markdown" | "sql") => {
    if (!r) return;
    const path = await saveDialog({ defaultPath: `result.${format === "markdown" ? "md" : format}` });
    if (!path) return;
    try { const n = await ipc.exportRows({ path, format, columns: columns.map((c) => c.name), rows: r.rows, table: "result", header: true }); toast("ok", `Exported ${n} rows`); } catch (e) { toast("err", errorMessage(e)); }
  };

  if (!results.length) {
    return <div className="flex-1 flex items-center justify-center text-dim text-xs">{running ? "Running…" : `Press ${mod}⏎ to run the statement under the cursor, ${mod}⇧⏎ to run all.`}</div>;
  }

  return (
    <div className="flex-1 flex flex-col min-h-0">
      <div className="h-7 flex items-stretch bg-app border-b border-border shrink-0 overflow-x-auto">
        {results.map((res, i) => (
          <button key={i} className={`px-3 text-xs flex items-center gap-1.5 border-r border-separator ${i === idx ? "bg-tab-selected text-white" : "text-tab-text hover:bg-tab-hover"}`} onClick={() => setActive(i)} title={res.statement.slice(0, 300)}>
            {res.error ? <AlertTriangle size={11} className="text-[#ff7b7b]" /> : <CheckCircle2 size={11} className="text-[#8bd36a]" />}
            {res.columns.length ? `Result ${i + 1}` : res.command ?? "Statement"}{res.columns.length ? ` (${formatCount(res.rows.length)})` : ""}
          </button>
        ))}
        <div className="flex-1" />
        {r && r.columns.length > 0 && (
          <>
            <button className="icon-btn self-center" title="Copy all as TSV" onClick={() => navigator.clipboard.writeText(selectionToText({ anchor: { r: 0, c: 0 }, focus: { r: r.rows.length - 1, c: columns.length - 1 } }, r.rows, columns, "tsv"))}><Copy size={13} /></button>
            <button className="icon-btn self-center mr-1" title="Export result..." onClick={(e) => open(e, [
              { label: "Export as CSV...", onClick: () => exportResult("csv") }, { label: "Export as JSON...", onClick: () => exportResult("json") },
              { label: "Export as SQL inserts...", onClick: () => exportResult("sql") }, { label: "Export as Markdown...", onClick: () => exportResult("markdown") },
            ])}><Download size={13} /></button>
          </>
        )}
      </div>
      {r?.error ? (
        <div className="flex-1 overflow-auto p-3 text-xs selectable">
          <div className="text-[#ff7b7b] font-semibold mb-1">ERROR{r.error.code ? ` [${r.error.code}]` : ""}: {r.error.message}</div>
          {r.error.detail && <div className="text-dim">DETAIL: {r.error.detail}</div>}
          {r.error.hint && <div className="text-dim">HINT: {r.error.hint}</div>}
          {r.error.position != null && <div className="text-dim">POSITION: {r.error.position}</div>}
          <pre className="mono mt-2 text-dim whitespace-pre-wrap">{r.statement}</pre>
        </div>
      ) : r && r.columns.length ? (
        <DataGrid columns={columns} rows={r.rows} onSelectionChange={setSel} onContextMenu={(e, s) => open(e, [
          { label: "Copy", shortcut: `${mod}C`, disabled: !s, onClick: () => s && navigator.clipboard.writeText(selectionToText(s, r.rows, columns, "tsv")) },
          { label: "Copy as CSV", disabled: !s, onClick: () => s && navigator.clipboard.writeText(selectionToText(s, r.rows, columns, "csv")) },
          { label: "Copy as JSON", disabled: !s, onClick: () => s && navigator.clipboard.writeText(selectionToText(s, r.rows, columns, "json")) },
          { label: "Copy as INSERT", disabled: !s, onClick: () => s && navigator.clipboard.writeText(selectionToText(s, r.rows, columns, "insert")) },
          { label: "Copy as Markdown", disabled: !s, onClick: () => s && navigator.clipboard.writeText(selectionToText(s, r.rows, columns, "markdown")) },
          { separator: true },
          { label: "Export result as CSV...", onClick: () => exportResult("csv") },
          { label: "Export result as JSON...", onClick: () => exportResult("json") },
        ])} emptyText="Query returned no rows" />
      ) : (
        <div className="flex-1 p-3 text-xs selectable">
          <div className="text-[#8bd36a]">{r?.command ?? "OK"}{r?.rowsAffected != null ? ` · ${formatCount(r.rowsAffected)} row${r.rowsAffected === 1 ? "" : "s"} affected` : ""}</div>
          <pre className="mono mt-2 text-dim whitespace-pre-wrap">{r?.statement}</pre>
        </div>
      )}
      <div className="h-6 flex items-center px-2 gap-3 text-[11px] text-dim border-t border-border shrink-0">
        {r && !r.error && <span>{r.columns.length ? `${formatCount(r.rows.length)} row${r.rows.length === 1 ? "" : "s"}${r.truncated ? " (truncated)" : ""}` : `${r.rowsAffected ?? 0} affected`}</span>}
        {r && <span>{formatDuration(r.durationMs)}</span>}
        {sel && <span>{Math.abs(sel.focus.r - sel.anchor.r) + 1}×{Math.abs(sel.focus.c - sel.anchor.c) + 1} selected</span>}
      </div>
      <ContextMenu menu={menu} onClose={close} />
    </div>
  );
}
