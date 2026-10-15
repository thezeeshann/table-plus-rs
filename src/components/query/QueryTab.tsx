import { useCallback, useEffect, useRef, useState } from "react";
import { Play, PlayCircle, Square, Star, Save, WrapText, FolderOpen } from "lucide-react";
import type { EditorView } from "@codemirror/view";
import { useApp, type Tab } from "@/store/app";
import { ipc, errorMessage } from "@/lib/ipc";
import type { QueryResult } from "@/lib/types";
import { SqlEditor, currentStatement } from "./SqlEditor";
import { ResultsPane } from "./ResultsPane";
import { Splitter } from "@/components/common/Splitter";
import { Modal, Field } from "@/components/common/Modal";
import { isMutation, splitStatements } from "@/lib/sql";
import { mod } from "@/lib/format";
import { save as saveDialog, open as openDialog } from "@tauri-apps/plugin-dialog";

export function QueryTab({ tab, active }: { tab: Tab; active: boolean }) {
  const session = useApp((s) => s.session)!;
  const connection = useApp((s) => s.connection);
  const schema = useApp((s) => s.schema);
  const listing = useApp((s) => s.listing);
  const settings = useApp((s) => s.settings);
  const toast = useApp((s) => s.toast);
  const register = useApp((s) => s.registerTabActions);
  const updateTab = useApp((s) => s.updateTab);
  const loadHistory = useApp((s) => s.loadHistory);
  const loadFavorites = useApp((s) => s.loadFavorites);
  const [sql, setSql] = useState(tab.sql ?? "");
  const [results, setResults] = useState<QueryResult[]>([]);
  const [running, setRunning] = useState(false);
  const setTabBusy = useApp((s) => s.setTabBusy);
  useEffect(() => { setTabBusy(tab.id, running); }, [running, tab.id, setTabBusy]);
  useEffect(() => () => setTabBusy(tab.id, false), [tab.id, setTabBusy]);
  const [editorHeight, setEditorHeight] = useState(0.5);
  const [schemaMap, setSchemaMap] = useState<Record<string, string[]>>({});
  const [favDialog, setFavDialog] = useState(false);
  const [favName, setFavName] = useState(tab.title);
  const [favThisConn, setFavThisConn] = useState(true);
  const viewRef = useRef<EditorView | null>(null);
  const container = useRef<HTMLDivElement>(null);

  useEffect(() => { updateTab(tab.id, { sql }); }, [sql]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    ipc.schemaColumns(session.sessionId, schema).then(setSchemaMap).catch(() => {});
  }, [session.sessionId, schema, listing]);

  const confirmIfDangerous = (statements: string[]) => {
    const safe = (connection?.safeMode ?? 0) > 0;
    if (!safe) return true;
    const danger = statements.filter((s) => isMutation(s));
    if (!danger.length) return true;
    return confirm(`Safe mode: ${danger.length} statement${danger.length === 1 ? "" : "s"} will modify the database. Continue?`);
  };

  const run = useCallback(async (text: string, split: boolean) => {
    if (!text.trim() || running) return;
    const stmts = split ? splitStatements(text).map((s) => s.text) : [text];
    if (!confirmIfDangerous(stmts)) return;
    setRunning(true);
    try {
      const res = await ipc.runQuery({ sessionId: session.sessionId, sql: text, split, rowLimit: 100000 });
      setResults(res);
      const errs = res.filter((r) => r.error).length;
      if (errs) toast("err", res.find((r) => r.error)!.error!.message);
      const kinds = res.map((r) => r.command);
      if (res.some((r) => !r.error && kinds.some((k) => k && ["CREATE", "DROP", "ALTER", "TRUNCATE", "COMMENT"].includes(k)))) useApp.getState().refreshListing();
      loadHistory();
    } catch (e) {
      toast("err", errorMessage(e));
    } finally { setRunning(false); }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [session.sessionId, running, toast, loadHistory, connection?.safeMode]);

  const runCurrent = useCallback(() => {
    const cur = currentStatement(viewRef.current);
    if (cur) run(cur.sql, false);
  }, [run]);
  const runAll = useCallback(() => run(sql, true), [run, sql]);
  const stop = useCallback(async () => {
    try { await ipc.cancelQuery(session.sessionId); toast("warn", "Cancel requested"); } catch (e) { toast("err", errorMessage(e)); }
  }, [session.sessionId, toast]);

  const saveFile = useCallback(async () => {
    const path = await saveDialog({ defaultPath: `${tab.title.replace(/[^\w.-]+/g, "_")}.sql`, filters: [{ name: "SQL", extensions: ["sql"] }] });
    if (!path) return;
    try { await ipc.writeTextFile(path, sql); toast("ok", `Saved ${path}`); } catch (e) { toast("err", errorMessage(e)); }
  }, [tab.title, sql, toast]);

  useEffect(() => {
    register(tab.id, { runCurrent, runAll, stop, reload: () => {}, find: () => viewRef.current?.focus(), exportData: () => {} });
  }, [tab.id, register, runCurrent, runAll, stop]);

  useEffect(() => { if (active) setTimeout(() => viewRef.current?.focus(), 0); }, [active]);

  const cmSchema = Object.fromEntries(Object.entries(schemaMap).map(([t, cols]) => [t, cols]));

  return (
    <div className="flex-1 flex flex-col min-h-0" ref={container}>
      <div className="h-8 flex items-center gap-1 px-2 border-b border-border bg-app shrink-0">
        <button className="btn !py-[2px]" onClick={runCurrent} disabled={running} title={`Run current query (${mod}⏎)`}><Play size={12} /> Run Current</button>
        <button className="btn !py-[2px]" onClick={runAll} disabled={running} title={`Run all queries (${mod}⇧⏎)`}><PlayCircle size={12} /> Run All</button>
        <button className="btn !py-[2px]" onClick={stop} disabled={!running} title="Cancel running query"><Square size={12} /> Stop</button>
        <span className="w-px h-5 bg-border mx-1" />
        <button className="btn !py-[2px]" onClick={() => { setFavName(tab.title); setFavDialog(true); }} title="Save to Queries"><Star size={12} /> Save</button>
        <button className="btn !py-[2px]" onClick={saveFile} title="Save as .sql file"><Save size={12} /> Save file</button>
        <button className="btn !py-[2px]" onClick={async () => { const p = await openDialog({ multiple: false, filters: [{ name: "SQL", extensions: ["sql", "psql", "txt"] }] }); if (typeof p === "string") { try { setSql(await ipc.readTextFile(p)); updateTab(tab.id, { title: p.split("/").pop() ?? tab.title }); } catch (e) { toast("err", errorMessage(e)); } } }} title="Open .sql file"><FolderOpen size={12} /> Open file</button>
        <button className="btn !py-[2px]" onClick={() => { const v = viewRef.current; if (!v) return; const text = v.state.doc.toString(); v.dispatch({ changes: { from: 0, to: text.length, insert: formatSql(text) } }); }} title="Format SQL (basic)"><WrapText size={12} /> Format</button>
        <div className="flex-1" />
        <span className="text-xs text-dim">{schema} · {running ? "running…" : `${splitStatements(sql).length} statement${splitStatements(sql).length === 1 ? "" : "s"}`}</span>
      </div>
      <div style={{ flex: `${editorHeight} 1 0` }} className="min-h-[80px]">
        <SqlEditor value={sql} onChange={setSql} schema={cmSchema} defaultSchema={schema} onRunCurrent={runCurrent} onRunAll={runAll} fontSize={settings.editorFontSize} autoCloseBrackets={settings.autoCloseBrackets} viewRef={viewRef} />
      </div>
      <Splitter direction="v" onDrag={(d) => { const h = container.current?.clientHeight ?? 600; setEditorHeight((v) => Math.max(0.1, Math.min(0.9, v + d / h))); }} />
      <div style={{ flex: `${1 - editorHeight} 1 0` }} className="flex flex-col min-h-[80px] border-t border-border">
        <ResultsPane results={results} running={running} />
      </div>
      {favDialog && (
        <Modal title="Save query" onClose={() => setFavDialog(false)} footer={<>
          <button className="btn" onClick={() => setFavDialog(false)}>Cancel</button>
          <button className="btn btn-primary" disabled={!favName.trim()} onClick={async () => {
            try {
              await ipc.saveFavorite({ id: "", name: favName.trim(), sql, connectionId: favThisConn ? session.connectionId : null, createdAt: "" });
              updateTab(tab.id, { title: favName.trim() }); loadFavorites(); setFavDialog(false); toast("ok", "Saved to Queries");
            } catch (e) { toast("err", errorMessage(e)); }
          }}>Save</button>
        </>}>
          <Field label="Name"><input type="text" autoFocus className="flex-1" value={favName} onChange={(e) => setFavName(e.target.value)} /></Field>
          <Field label="Scope"><label className="flex items-center gap-1.5"><input type="checkbox" checked={favThisConn} onChange={(e) => setFavThisConn(e.target.checked)} />Only for this connection</label></Field>
        </Modal>
      )}
    </div>
  );
}

/** Lightweight formatter: uppercase keywords and break before major clauses. */
function formatSql(text: string): string {
  const kw = ["SELECT", "FROM", "WHERE", "GROUP BY", "ORDER BY", "HAVING", "LIMIT", "OFFSET", "LEFT JOIN", "RIGHT JOIN", "INNER JOIN", "FULL JOIN", "CROSS JOIN", "JOIN", "ON", "UNION ALL", "UNION", "INSERT INTO", "VALUES", "UPDATE", "SET", "DELETE FROM", "RETURNING", "WITH", "AND", "OR"];
  return splitStatements(text).map((s) => {
    let out = s.text.replace(/\s+/g, " ");
    for (const k of kw) {
      const re = new RegExp(`\\b${k.replace(" ", "\\s+")}\\b`, "gi");
      out = out.replace(re, (m) => (["AND", "OR", "ON", "SET", "VALUES", "RETURNING"].includes(k) ? `\n  ${k}` : `\n${k}`).replace(/^\n/, m === out.slice(0, m.length) ? "" : "\n"));
    }
    return out.replace(/^\n/, "").replace(/\n{2,}/g, "\n").trim() + ";";
  }).join("\n\n");
}
