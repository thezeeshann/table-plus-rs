import { useEffect, useMemo, useState } from "react";
import { Key, Link2, Plus, Trash2 } from "lucide-react";
import { useApp, type Tab } from "@/store/app";
import { ipc, errorMessage } from "@/lib/ipc";
import type { ColumnInfo, DdlOp, TableStructure } from "@/lib/types";
import { ContextMenu, useContextMenu } from "@/components/common/ContextMenu";
import { Modal, Field } from "@/components/common/Modal";
import { PG_TYPES, FK_ACTIONS } from "./pgTypes";
import { mod } from "@/lib/format";

interface Draft { name: string; type: string; nullable: boolean; default: string; comment: string }
interface NewCol extends Draft { id: number }
let newSeq = 1;

export function StructureView({ tab, structure, onChanged }: { tab: Tab; structure: TableStructure; onChanged: () => void }) {
  const session = useApp((s) => s.session)!;
  const toast = useApp((s) => s.toast);
  const setDirty = useApp((s) => s.setDirty);
  const register = useApp((s) => s.registerTabActions);
  const [drafts, setDrafts] = useState<Record<string, Draft>>({});
  const [newCols, setNewCols] = useState<NewCol[]>([]);
  const [dropped, setDropped] = useState<Set<string>>(new Set());
  const [extraOps, setExtraOps] = useState<{ label: string; op: DdlOp }[]>([]);
  const [tableComment, setTableComment] = useState<string | null>(null);
  const [preview, setPreview] = useState<string | null>(null);
  const [fkDialog, setFkDialog] = useState(false);
  const { menu, open, close } = useContextMenu();
  const listing = useApp((s) => s.listing);
  const editable = structure.kind === "table" || structure.kind === "partitioned";

  useEffect(() => { setDrafts({}); setNewCols([]); setDropped(new Set()); setExtraOps([]); setTableComment(null); }, [structure]);

  const draftFor = (c: ColumnInfo): Draft => drafts[c.name] ?? { name: c.name, type: c.formatType, nullable: c.nullable, default: c.default ?? "", comment: c.comment ?? "" };
  const setDraft = (c: ColumnInfo, p: Partial<Draft>) => {
    const d = { ...draftFor(c), ...p };
    const same = d.name === c.name && d.type === c.formatType && d.nullable === c.nullable && d.default === (c.default ?? "") && d.comment === (c.comment ?? "");
    setDrafts((all) => { const n = { ...all }; if (same) delete n[c.name]; else n[c.name] = d; return n; });
  };

  const ops = useMemo((): DdlOp[] => {
    const out: DdlOp[] = [];
    for (const name of dropped) out.push({ op: "drop_column", name });
    for (const c of structure.columns) {
      const d = drafts[c.name]; if (!d || dropped.has(c.name)) continue;
      if (d.type !== c.formatType) out.push({ op: "change_type", name: c.name, data_type: d.type });
      if (d.nullable !== c.nullable) out.push({ op: "set_nullable", name: c.name, nullable: d.nullable });
      if (d.default !== (c.default ?? "")) out.push({ op: "set_default", name: c.name, default: d.default || null });
      if (d.comment !== (c.comment ?? "")) out.push({ op: "set_comment", name: c.name, comment: d.comment || null });
      if (d.name !== c.name && d.name.trim()) out.push({ op: "rename_column", name: c.name, new_name: d.name.trim() });
    }
    for (const n of newCols) if (n.name.trim()) out.push({ op: "add_column", name: n.name.trim(), data_type: n.type || "text", nullable: n.nullable, default: n.default || null, comment: n.comment || null });
    for (const e of extraOps) out.push(e.op);
    if (tableComment != null && tableComment !== (structure.comment ?? "")) out.push({ op: "set_table_comment", comment: tableComment || null });
    return out;
  }, [drafts, newCols, dropped, extraOps, tableComment, structure]);

  useEffect(() => { setDirty(tab.id, ops.length > 0); }, [ops.length, tab.id, setDirty]);

  const discard = () => { setDrafts({}); setNewCols([]); setDropped(new Set()); setExtraOps([]); setTableComment(null); };
  const commit = async () => {
    if (!ops.length) return;
    try {
      const stmts = await ipc.applyDdl(session.sessionId, structure.schema, structure.table, ops);
      toast("ok", `${stmts.length} DDL statement${stmts.length === 1 ? "" : "s"} applied`);
      discard(); onChanged();
    } catch (e) { toast("err", errorMessage(e)); }
  };
  const previewSql = async () => { if (ops.length) setPreview((await ipc.previewDdl(structure.schema, structure.table, ops)).join(";\n") + ";"); };

  useEffect(() => {
    register(tab.id, { commit, discard, preview: previewSql, addRow: () => setNewCols((l) => [...l, { id: newSeq++, name: "", type: "text", nullable: true, default: "", comment: "" }]), reload: onChanged, setView: (v) => useApp.getState().updateTab(tab.id, { view: v }) });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tab.id, ops, structure]);

  const colMenu = (e: React.MouseEvent, c: ColumnInfo) => open(e, [
    { label: "Copy name", onClick: () => navigator.clipboard.writeText(c.name) },
    { label: "Copy type", onClick: () => navigator.clipboard.writeText(c.formatType) },
    { separator: true },
    ...(editable ? [
      ...(!structure.primaryKey.length ? [{ label: "Set as primary key", onClick: () => setExtraOps((l) => [...l, { label: `PK ${c.name}`, op: { op: "add_primary_key" as const, columns: [c.name] } }]) }] : []),
      { label: "Add foreign key...", onClick: () => setFkDialog(true) },
      ...(c.foreignKey ? [{ label: `Drop foreign key ${c.foreignKey.constraintName}`, danger: true, onClick: () => setExtraOps((l) => [...l, { label: `drop fk ${c.foreignKey!.constraintName}`, op: { op: "drop_constraint" as const, name: c.foreignKey!.constraintName } }]) }] : []),
      { separator: true },
      { label: dropped.has(c.name) ? "Undo drop" : "Drop column", danger: !dropped.has(c.name), onClick: () => setDropped((d) => { const n = new Set(d); if (n.has(c.name)) n.delete(c.name); else n.add(c.name); return n; }) },
    ] : []),
  ]);

  const th = "text-left text-dim font-semibold px-2 py-1 border-b border-border sticky top-0 bg-content";
  const td = "px-2 py-0.5 border-b border-[#00000040]";
  const input = (value: string, onChange: (v: string) => void, extra?: string, list?: string) => (
    <input type="text" list={list} className={`w-full !py-[2px] !bg-transparent !border-transparent hover:!border-border focus:!border-accent mono ${extra ?? ""}`} value={value} onChange={(e) => onChange(e.target.value)} disabled={!editable} />
  );

  return (
    <div className="flex-1 flex flex-col min-h-0">
      <div className="flex-1 overflow-auto min-h-0">
        <table className="w-full text-xs border-collapse">
          <thead>
            <tr><th className={`${th} w-8`}>#</th><th className={th}>Column</th><th className={th}>Type</th><th className={`${th} w-16`}>Nullable</th><th className={th}>Default</th><th className={th}>Key</th><th className={th}>Comment</th></tr>
          </thead>
          <tbody>
            {structure.columns.map((c, i) => {
              const d = draftFor(c);
              const changed = !!drafts[c.name];
              const isDropped = dropped.has(c.name);
              return (
                <tr key={c.name} className={`${isDropped ? "bg-deleted/60 line-through" : changed ? "bg-modified/40" : ""} hover:bg-tab-hover/40`} onContextMenu={(e) => colMenu(e, c)}>
                  <td className={`${td} text-dim text-right`}>{i + 1}</td>
                  <td className={td}><div className="flex items-center gap-1">{c.isPrimaryKey && <Key size={11} className="text-[#f9b746]" />}{input(d.name, (v) => setDraft(c, { name: v }))}</div></td>
                  <td className={td}>{input(d.type, (v) => setDraft(c, { type: v }), undefined, "pg-types-list")}</td>
                  <td className={`${td} text-center`}><input type="checkbox" checked={d.nullable} disabled={!editable || c.isPrimaryKey} onChange={(e) => setDraft(c, { nullable: e.target.checked })} /></td>
                  <td className={td}>{c.isIdentity ? <span className="text-dim italic">identity</span> : c.isGenerated ? <span className="text-dim italic" title={c.generationExpression ?? ""}>generated</span> : input(d.default, (v) => setDraft(c, { default: v }))}</td>
                  <td className={`${td} text-dim`}>
                    {c.isPrimaryKey && <span className="mr-2">PRIMARY</span>}
                    {c.foreignKey && <span className="inline-flex items-center gap-1" title={`${c.foreignKey.constraintName} → ${c.foreignKey.parentSchema}.${c.foreignKey.parentTable}(${c.foreignKey.parentColumns.join(",")}) ON UPDATE ${c.foreignKey.onUpdate} ON DELETE ${c.foreignKey.onDelete}`}><Link2 size={11} />{c.foreignKey.parentTable}.{c.foreignKey.parentColumns[c.foreignKey.childColumns.indexOf(c.name)]}</span>}
                  </td>
                  <td className={td}>{input(d.comment, (v) => setDraft(c, { comment: v }))}</td>
                </tr>
              );
            })}
            {newCols.map((n) => (
              <tr key={n.id} className="bg-inserted/40">
                <td className={`${td} text-dim text-right`}>+</td>
                <td className={td}><input type="text" autoFocus className="w-full !py-[2px] mono" placeholder="column_name" value={n.name} onChange={(e) => setNewCols((l) => l.map((x) => (x.id === n.id ? { ...x, name: e.target.value } : x)))} /></td>
                <td className={td}><input type="text" list="pg-types-list" className="w-full !py-[2px] mono" value={n.type} onChange={(e) => setNewCols((l) => l.map((x) => (x.id === n.id ? { ...x, type: e.target.value } : x)))} /></td>
                <td className={`${td} text-center`}><input type="checkbox" checked={n.nullable} onChange={(e) => setNewCols((l) => l.map((x) => (x.id === n.id ? { ...x, nullable: e.target.checked } : x)))} /></td>
                <td className={td}><input type="text" className="w-full !py-[2px] mono" value={n.default} onChange={(e) => setNewCols((l) => l.map((x) => (x.id === n.id ? { ...x, default: e.target.value } : x)))} /></td>
                <td className={td}><button className="icon-btn" onClick={() => setNewCols((l) => l.filter((x) => x.id !== n.id))}><Trash2 size={12} /></button></td>
                <td className={td}><input type="text" className="w-full !py-[2px]" value={n.comment} onChange={(e) => setNewCols((l) => l.map((x) => (x.id === n.id ? { ...x, comment: e.target.value } : x)))} /></td>
              </tr>
            ))}
          </tbody>
        </table>
        <datalist id="pg-types-list">{PG_TYPES.map((t) => <option key={t} value={t} />)}</datalist>
        {editable && <button className="btn btn-ghost m-2" onClick={() => setNewCols((l) => [...l, { id: newSeq++, name: "", type: "text", nullable: true, default: "", comment: "" }])}><Plus size={13} /> Column</button>}

        <div className="px-2 pb-3 grid gap-3" style={{ gridTemplateColumns: "1fr 1fr" }}>
          <Section title={`Foreign keys (${structure.foreignKeys.length})`}>
            {structure.foreignKeys.map((fk) => (
              <div key={fk.constraintName} className="flex items-center gap-2 mono text-xs py-0.5" title={`ON UPDATE ${fk.onUpdate} · ON DELETE ${fk.onDelete}`}>
                <span className="text-dim">{fk.constraintName}</span><span>({fk.childColumns.join(", ")}) → {fk.parentSchema}.{fk.parentTable}({fk.parentColumns.join(", ")})</span>
                {editable && <button className="icon-btn ml-auto" title="Drop constraint" onClick={() => setExtraOps((l) => [...l, { label: `drop ${fk.constraintName}`, op: { op: "drop_constraint", name: fk.constraintName } }])}><Trash2 size={12} /></button>}
              </div>
            ))}
            {editable && <button className="btn btn-ghost text-xs mt-1" onClick={() => setFkDialog(true)}><Plus size={12} /> Foreign key</button>}
          </Section>
          <Section title={`Check constraints (${structure.checks.length})`}>
            {structure.checks.map((c) => (
              <div key={c.name} className="flex items-center gap-2 mono text-xs py-0.5"><span className="text-dim">{c.name}</span><span className="truncate-1">{c.condition}</span>
                {editable && <button className="icon-btn ml-auto" onClick={() => setExtraOps((l) => [...l, { label: `drop ${c.name}`, op: { op: "drop_constraint", name: c.name } }])}><Trash2 size={12} /></button>}
              </div>
            ))}
          </Section>
          <Section title={`Triggers (${structure.triggers.length})`}>
            {structure.triggers.map((t) => <div key={t.name} className="mono text-xs py-0.5" title={t.statement}><span className="text-dim">{t.timing} {t.event}</span> {t.name}</div>)}
          </Section>
          <Section title="Table comment">
            <textarea className="w-full mono text-xs" rows={2} disabled={!editable} value={tableComment ?? structure.comment ?? ""} onChange={(e) => setTableComment(e.target.value)} />
          </Section>
        </div>
        {extraOps.length > 0 && (
          <div className="px-2 pb-2 text-xs text-dim">Pending: {extraOps.map((e, i) => <span key={i} className="mr-2 px-1.5 py-0.5 rounded bg-control mono">{e.label} <button onClick={() => setExtraOps((l) => l.filter((_, j) => j !== i))}>×</button></span>)}</div>
        )}
      </div>
      {ops.length > 0 && (
        <div className="flex items-center gap-2 px-3 h-8 bg-warn text-white text-xs shrink-0">
          <span className="flex-1">{ops.length} pending DDL change{ops.length === 1 ? "" : "s"}</span>
          <button className="btn !py-0 !bg-[#0b2127]" onClick={previewSql}>Preview ({mod}⇧P)</button>
          <button className="btn !py-0 !bg-[#0b2127]" onClick={discard}>Discard</button>
          <button className="btn btn-primary !py-0" onClick={commit}>Commit ({mod}S)</button>
        </div>
      )}
      <ContextMenu menu={menu} onClose={close} />
      {preview && (
        <Modal title="Preview DDL" onClose={() => setPreview(null)} width={720} footer={<>
          <button className="btn" onClick={() => navigator.clipboard.writeText(preview)}>Copy</button>
          <button className="btn" onClick={() => setPreview(null)}>Close</button>
          <button className="btn btn-primary" onClick={() => { setPreview(null); commit(); }}>Commit</button>
        </>}>
          <pre className="mono selectable whitespace-pre-wrap text-xs">{preview}</pre>
        </Modal>
      )}
      {fkDialog && (
        <FkDialog structure={structure} tables={(listing?.items ?? []).map((i) => i.name)} onClose={() => setFkDialog(false)} onAdd={(op) => { setExtraOps((l) => [...l, { label: `fk ${op.columns.join(",")}→${op.ref_table}`, op }]); setFkDialog(false); }} />
      )}
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return <div className="rounded border border-border p-2"><div className="text-dim font-semibold text-xs mb-1">{title}</div>{children}</div>;
}

function FkDialog({ structure, tables, onClose, onAdd }: { structure: TableStructure; tables: string[]; onClose: () => void; onAdd: (op: Extract<DdlOp, { op: "add_foreign_key" }>) => void }) {
  const session = useApp((s) => s.session)!;
  const [col, setCol] = useState(structure.columns[0]?.name ?? "");
  const [refTable, setRefTable] = useState(tables[0] ?? "");
  const [refCols, setRefCols] = useState<string[]>([]);
  const [refCol, setRefCol] = useState("");
  const [onUpdate, setOnUpdate] = useState("NO ACTION");
  const [onDelete, setOnDelete] = useState("NO ACTION");
  useEffect(() => {
    if (!refTable) return;
    ipc.tableStructure(session.sessionId, structure.schema, refTable, "table").then((s) => { setRefCols(s.columns.map((c) => c.name)); setRefCol(s.primaryKey[0] ?? s.columns[0]?.name ?? ""); }).catch(() => {});
  }, [refTable, session.sessionId, structure.schema]);
  return (
    <Modal title="Add foreign key" onClose={onClose} footer={<>
      <button className="btn" onClick={onClose}>Cancel</button>
      <button className="btn btn-primary" disabled={!col || !refTable || !refCol} onClick={() => onAdd({ op: "add_foreign_key", columns: [col], ref_schema: structure.schema, ref_table: refTable, ref_columns: [refCol], on_update: onUpdate, on_delete: onDelete })}>Add</button>
    </>}>
      <Field label="Column"><select className="flex-1 mono" value={col} onChange={(e) => setCol(e.target.value)}>{structure.columns.map((c) => <option key={c.name}>{c.name}</option>)}</select></Field>
      <Field label="References"><select className="flex-1 mono" value={refTable} onChange={(e) => setRefTable(e.target.value)}>{tables.map((t) => <option key={t}>{t}</option>)}</select>
        <select className="flex-1 mono" value={refCol} onChange={(e) => setRefCol(e.target.value)}>{refCols.map((c) => <option key={c}>{c}</option>)}</select></Field>
      <Field label="On update"><select className="flex-1" value={onUpdate} onChange={(e) => setOnUpdate(e.target.value)}>{FK_ACTIONS.map((a) => <option key={a}>{a}</option>)}</select></Field>
      <Field label="On delete"><select className="flex-1" value={onDelete} onChange={(e) => setOnDelete(e.target.value)}>{FK_ACTIONS.map((a) => <option key={a}>{a}</option>)}</select></Field>
    </Modal>
  );
}
