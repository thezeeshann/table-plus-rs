import { useState } from "react";
import { Modal, Field } from "@/components/common/Modal";
import { useApp } from "@/store/app";
import { ipc } from "@/lib/ipc";

export function SettingsDialog() {
  const settings = useApp((s) => s.settings);
  const save = useApp((s) => s.saveSettings);
  const session = useApp((s) => s.session);
  const toast = useApp((s) => s.toast);
  const close = () => useApp.getState().setSettingsDialog(false);
  const [s, setS] = useState(settings);
  return (
    <Modal title="Settings" onClose={close} width={520} footer={<>
      <button className="btn" onClick={close}>Cancel</button>
      <button className="btn btn-primary" onClick={async () => { await save(s); close(); }}>Save</button>
    </>}>
      <Field label="Rows per page"><input type="number" className="w-28 mono" value={s.defaultLimit} onChange={(e) => setS({ ...s, defaultLimit: Math.max(1, Number(e.target.value) || 300) })} /><span className="text-dim text-xs">default for new connections</span></Field>
      <Field label="Estimate count"><input type="number" className="w-32 mono" value={s.estimateCountThreshold} onChange={(e) => setS({ ...s, estimateCountThreshold: Number(e.target.value) || 100000 })} /><span className="text-dim text-xs">when table has more than N rows (unfiltered)</span></Field>
      <Field label="Editor font size"><input type="number" className="w-20 mono" value={s.editorFontSize} onChange={(e) => setS({ ...s, editorFontSize: Math.min(24, Math.max(9, Number(e.target.value) || 13)) })} /></Field>
      <Field label="Editor"><label className="flex items-center gap-1.5"><input type="checkbox" checked={s.autoCloseBrackets} onChange={(e) => setS({ ...s, autoCloseBrackets: e.target.checked })} />Automatically insert closing braces and quotes</label></Field>
      <Field label="History">
        <button className="btn" disabled={!session} onClick={async () => {
          if (!session || !confirm("Clear query history for this connection?")) return;
          await ipc.clearHistory(session.connectionId);
          useApp.getState().loadHistory();
          toast("ok", "History cleared");
        }}>Clear history for this connection</button>
      </Field>
      <div className="text-dim text-xs mt-3">Theme: Jade Dark (ported from TablePlus). Connections live in the app data folder; passwords in the macOS Keychain.</div>
    </Modal>
  );
}
