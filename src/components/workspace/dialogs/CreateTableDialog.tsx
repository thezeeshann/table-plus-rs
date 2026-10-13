import { useState } from "react";
import { Modal, Field } from "@/components/common/Modal";
import { useApp } from "@/store/app";
import { ipc, errorMessage } from "@/lib/ipc";
import { Trash2, Plus } from "lucide-react";
import { PG_TYPES } from "@/components/table/pgTypes";

interface Col { name: string; dataType: string; nullable: boolean; default: string; primaryKey: boolean }

export function CreateTableDialog() {
  const session = useApp((s) => s.session);
  const schema = useApp((s) => s.schema);
  const toast = useApp((s) => s.toast);
  const refresh = useApp((s) => s.refreshListing);
  const openItem = useApp((s) => s.openItem);
  const close = () => useApp.getState().setCreateTableDialog(false);
  const [name, setName] = useState("");
  const [cols, setCols] = useState<Col[]>([{ name: "id", dataType: "bigint GENERATED ALWAYS AS IDENTITY", nullable: false, default: "", primaryKey: true }]);
  const [preview, setPreview] = useState<string | null>(null);
  const upd = (i: number, p: Partial<Col>) => setCols(cols.map((c, j) => (j === i ? { ...c, ...p } : c)));
  const ops = () => [{ op: "create_table" as const, name, columns: cols.filter((c) => c.name.trim()).map((c) => ({ name: c.name, dataType: c.dataType, nullable: c.nullable, default: c.default || null, primaryKey: c.primaryKey })) }];
  return (
    <Modal title={`Create new table in ${schema}`} onClose={close} width={760} footer={<>
      <button className="btn" onClick={async () => setPreview((await ipc.previewDdl(schema, name || "new_table", ops())).join(";\n") + ";")}>Preview SQL</button>
      <button className="btn" onClick={close}>Cancel</button>
      <button className="btn btn-primary" disabled={!name.trim() || !cols.some((c) => c.name.trim())} onClick={async () => {
        if (!session) return;
        try {
          await ipc.applyDdl(session.sessionId, schema, name, ops());
          toast("ok", `Table ${name} created`);
          close(); await refresh(); openItem(schema, name, "table", { pinned: true, view: "structure" });
        } catch (e) { toast("err", errorMessage(e)); }
      }}>Create</button>
    </>}>
      <Field label="Table name"><input type="text" autoFocus className="flex-1 mono" value={name} onChange={(e) => setName(e.target.value)} /></Field>
      <table className="w-full text-xs mt-2">
        <thead><tr className="text-dim text-left"><th className="pb-1">Name</th><th>Type</th><th className="w-14">PK</th><th className="w-16">Nullable</th><th>Default</th><th className="w-8" /></tr></thead>
        <tbody>
          {cols.map((c, i) => (
            <tr key={i}>
              <td className="pr-1 py-0.5"><input type="text" className="w-full mono" value={c.name} onChange={(e) => upd(i, { name: e.target.value })} /></td>
              <td className="pr-1"><input type="text" list="pg-types" className="w-full mono" value={c.dataType} onChange={(e) => upd(i, { dataType: e.target.value })} /></td>
              <td className="text-center"><input type="checkbox" checked={c.primaryKey} onChange={(e) => upd(i, { primaryKey: e.target.checked, nullable: e.target.checked ? false : c.nullable })} /></td>
              <td className="text-center"><input type="checkbox" checked={c.nullable} disabled={c.primaryKey} onChange={(e) => upd(i, { nullable: e.target.checked })} /></td>
              <td className="pr-1"><input type="text" className="w-full mono" value={c.default} onChange={(e) => upd(i, { default: e.target.value })} placeholder="now()" /></td>
              <td><button className="icon-btn" onClick={() => setCols(cols.filter((_, j) => j !== i))}><Trash2 size={13} /></button></td>
            </tr>
          ))}
        </tbody>
      </table>
      <datalist id="pg-types">{PG_TYPES.map((t) => <option key={t} value={t} />)}</datalist>
      <button className="btn btn-ghost mt-2" onClick={() => setCols([...cols, { name: "", dataType: "text", nullable: true, default: "", primaryKey: false }])}><Plus size={13} /> Column</button>
      {preview && <pre className="mono selectable mt-3 p-2 rounded bg-control border border-border whitespace-pre-wrap text-xs">{preview}</pre>}
    </Modal>
  );
}
