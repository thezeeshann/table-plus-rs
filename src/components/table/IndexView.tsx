import { useEffect, useState } from "react";
import { Plus, Trash2 } from "lucide-react";
import { useApp, type Tab } from "@/store/app";
import { ipc, errorMessage } from "@/lib/ipc";
import type { DdlOp, TableStructure } from "@/lib/types";
import { Modal } from "@/components/common/Modal";
import { formatBytes, mod } from "@/lib/format";
import { INDEX_METHODS } from "./pgTypes";

interface NewIndex { id: number; name: string; columns: string[]; unique: boolean; method: string; condition: string }
let seq = 1;

export function IndexView({ tab, structure, onChanged }: { tab: Tab; structure: TableStructure; onChanged: () => void }) {
  const session = useApp((s) => s.session)!;
  const toast = useApp((s) => s.toast);
  const setDirty = useApp((s) => s.setDirty);
  const register = useApp((s) => s.registerTabActions);
  const [dropped, setDropped] = useState<Set<string>>(new Set());
  const [added, setAdded] = useState<NewIndex[]>([]);
  const [preview, setPreview] = useState<string | null>(null);
  const editable = structure.kind === "table" || structure.kind === "partitioned" || structure.kind === "matview";

  useEffect(() => { setDropped(new Set()); setAdded([]); }, [structure]);

  const ops: DdlOp[] = [
    ...[...dropped].map((name) => ({ op: "drop_index" as const, name })),
    ...added.filter((a) => a.columns.length).map((a) => ({ op: "add_index" as const, name: a.name || `${structure.table}_${a.columns.join("_")}_idx`, columns: a.columns, unique: a.unique, method: a.method, condition: a.condition || null })),
  ];
  useEffect(() => { setDirty(tab.id, ops.length > 0); }, [ops.length, tab.id, setDirty]);

  const discard = () => { setDropped(new Set()); setAdded([]); };
  const commit = async () => {
    if (!ops.length) return;
    try { await ipc.applyDdl(session.sessionId, structure.schema, structure.table, ops); toast("ok", "Indexes updated"); discard(); onChanged(); } catch (e) { toast("err", errorMessage(e)); }
  };
  const previewSql = async () => { if (ops.length) setPreview((await ipc.previewDdl(structure.schema, structure.table, ops)).join(";\n") + ";"); };
  const addIndex = () => setAdded((l) => [...l, { id: seq++, name: "", columns: [], unique: false, method: "btree", condition: "" }]);
  useEffect(() => {
    register(tab.id, { commit, discard, preview: previewSql, addRow: addIndex, reload: onChanged, setView: (v) => useApp.getState().updateTab(tab.id, { view: v }) });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tab.id, ops.length, structure, dropped, added]);

  const th = "text-left text-dim font-semibold px-2 py-1 border-b border-border sticky top-0 bg-content";
  const td = "px-2 py-1 border-b border-[var(--grid-line)] mono";
  return (
    <div className="flex-1 flex flex-col min-h-0">
      <div className="flex-1 overflow-auto min-h-0">
        <table className="w-full text-xs border-collapse">
          <thead><tr><th className={th}>Index name</th><th className={th}>Algorithm</th><th className={`${th} w-16`}>Unique</th><th className={th}>Columns</th><th className={th}>Condition</th><th className={th}>Include</th><th className={th}>Size</th><th className={`${th} w-8`} /></tr></thead>
          <tbody>
            {structure.indexes.map((ix) => (
              <tr key={ix.name} className={dropped.has(ix.name) ? "bg-deleted/60 line-through" : "hover:bg-tab-hover/40"} title={ix.definition}>
                <td className={td}>{ix.isPrimary && <span className="text-[var(--key-color)] mr-1">PK</span>}{ix.name}</td>
                <td className={td}>{ix.algorithm}</td>
                <td className={`${td} text-center`}>{ix.isUnique ? "✓" : ""}</td>
                <td className={td}>{ix.columns.join(", ")}</td>
                <td className={`${td} text-dim`}>{ix.condition ?? ""}</td>
                <td className={`${td} text-dim`}>{ix.include ?? ""}</td>
                <td className={`${td} text-dim`}>{formatBytes(ix.size)}</td>
                <td className={td}>{editable && !ix.isPrimary && <button className="icon-btn" title={dropped.has(ix.name) ? "Undo" : "Drop index"} onClick={() => setDropped((d) => { const n = new Set(d); if (n.has(ix.name)) n.delete(ix.name); else n.add(ix.name); return n; })}><Trash2 size={12} /></button>}</td>
              </tr>
            ))}
            {added.map((a) => (
              <tr key={a.id} className="bg-inserted/40">
                <td className={td}><input type="text" className="w-full !py-[2px] mono" placeholder={`${structure.table}_${a.columns.join("_") || "col"}_idx`} value={a.name} onChange={(e) => setAdded((l) => l.map((x) => (x.id === a.id ? { ...x, name: e.target.value } : x)))} /></td>
                <td className={td}><select className="!py-[2px]" value={a.method} onChange={(e) => setAdded((l) => l.map((x) => (x.id === a.id ? { ...x, method: e.target.value } : x)))}>{INDEX_METHODS.map((m) => <option key={m}>{m}</option>)}</select></td>
                <td className={`${td} text-center`}><input type="checkbox" checked={a.unique} onChange={(e) => setAdded((l) => l.map((x) => (x.id === a.id ? { ...x, unique: e.target.checked } : x)))} /></td>
                <td className={td}>
                  <div className="flex flex-wrap gap-1">
                    {structure.columns.map((c) => (
                      <label key={c.name} className={`px-1.5 py-0.5 rounded border cursor-default ${a.columns.includes(c.name) ? "bg-tab-selected border-tab-selected text-white" : "border-border"}`} onClick={() => setAdded((l) => l.map((x) => (x.id === a.id ? { ...x, columns: x.columns.includes(c.name) ? x.columns.filter((y) => y !== c.name) : [...x.columns, c.name] } : x)))}>{c.name}</label>
                    ))}
                  </div>
                </td>
                <td className={td} colSpan={2}><input type="text" className="w-full !py-[2px] mono" placeholder="WHERE condition (partial index)" value={a.condition} onChange={(e) => setAdded((l) => l.map((x) => (x.id === a.id ? { ...x, condition: e.target.value } : x)))} /></td>
                <td className={td} />
                <td className={td}><button className="icon-btn" onClick={() => setAdded((l) => l.filter((x) => x.id !== a.id))}><Trash2 size={12} /></button></td>
              </tr>
            ))}
          </tbody>
        </table>
        {editable && <button className="btn btn-ghost m-2" onClick={addIndex}><Plus size={13} /> Index</button>}
      </div>
      {ops.length > 0 && (
        <div className="flex items-center gap-2 px-3 h-8 bg-warn text-white text-xs shrink-0">
          <span className="flex-1">{ops.length} pending index change{ops.length === 1 ? "" : "s"}</span>
          <button className="btn !py-0 !bg-[var(--app-bg)]" onClick={previewSql}>Preview</button>
          <button className="btn !py-0 !bg-[var(--app-bg)]" onClick={discard}>Discard</button>
          <button className="btn btn-primary !py-0" onClick={commit}>Commit ({mod}S)</button>
        </div>
      )}
      {preview && (
        <Modal title="Preview DDL" onClose={() => setPreview(null)} width={720} footer={<><button className="btn" onClick={() => setPreview(null)}>Close</button><button className="btn btn-primary" onClick={() => { setPreview(null); commit(); }}>Commit</button></>}>
          <pre className="mono selectable whitespace-pre-wrap text-xs">{preview}</pre>
        </Modal>
      )}
    </div>
  );
}
