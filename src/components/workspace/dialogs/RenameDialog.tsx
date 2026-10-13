import { useState } from "react";
import { Modal, Field } from "@/components/common/Modal";
import { useApp } from "@/store/app";
import { ipc, errorMessage } from "@/lib/ipc";
import { quoteIdent } from "@/lib/sql";

export function RenameDialog() {
  const dlg = useApp((s) => s.renameDialog)!;
  const session = useApp((s) => s.session);
  const toast = useApp((s) => s.toast);
  const refresh = useApp((s) => s.refreshListing);
  const close = () => useApp.getState().setRenameDialog(null);
  const [name, setName] = useState(dlg.name);
  const kw = dlg.kind === "view" ? "VIEW" : dlg.kind === "matview" ? "MATERIALIZED VIEW" : dlg.kind === "sequence" ? "SEQUENCE" : "TABLE";
  const go = async () => {
    if (!session || !name.trim() || name === dlg.name) return close();
    try {
      await ipc.execute(session.sessionId, `ALTER ${kw} ${quoteIdent(dlg.schema)}.${quoteIdent(dlg.name)} RENAME TO ${quoteIdent(name.trim())}`);
      toast("ok", `Renamed to ${name}`);
      const s = useApp.getState();
      s.tabs.filter((t) => t.schema === dlg.schema && t.name === dlg.name).forEach((t) => s.updateTab(t.id, { name: name.trim(), title: name.trim() }));
      close(); refresh();
    } catch (e) { toast("err", errorMessage(e)); }
  };
  return (
    <Modal title={`Rename ${dlg.name}`} onClose={close} footer={<>
      <button className="btn" onClick={close}>Cancel</button>
      <button className="btn btn-primary" onClick={go}>Rename</button>
    </>}>
      <Field label="New name"><input type="text" autoFocus className="flex-1 mono" value={name} onChange={(e) => setName(e.target.value)} onKeyDown={(e) => e.key === "Enter" && go()} /></Field>
    </Modal>
  );
}
