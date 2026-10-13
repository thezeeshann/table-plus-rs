import { useEffect, useState } from "react";
import { Modal } from "@/components/common/Modal";
import { useApp } from "@/store/app";
import { ipc, errorMessage } from "@/lib/ipc";
import { Database, Loader2 } from "lucide-react";

export function OpenDatabaseDialog() {
  const session = useApp((s) => s.session);
  const close = () => useApp.getState().setOpenDatabaseDialog(false);
  const connect = useApp((s) => s.connect);
  const toast = useApp((s) => s.toast);
  const [dbs, setDbs] = useState<string[] | null>(null);
  const [q, setQ] = useState("");
  const [sel, setSel] = useState(0);
  const [creating, setCreating] = useState(false);
  const [newName, setNewName] = useState("");

  useEffect(() => {
    if (!session) return;
    ipc.listDatabases(session.sessionId).then(setDbs).catch((e) => toast("err", errorMessage(e)));
  }, [session, toast]);

  const list = (dbs ?? []).filter((d) => d.toLowerCase().includes(q.toLowerCase()));
  const go = async (db: string) => {
    if (!session) return;
    close();
    if (db === session.database) return;
    const dirty = useApp.getState().tabs.some((t) => t.dirty);
    if (dirty && !confirm("Switching database closes all tabs. Discard unsaved changes?")) return;
    await connect(session.connectionId, db).catch(() => {});
  };

  return (
    <Modal title="Open a Database" onClose={close} width={460}>
      <input
        type="text" autoFocus className="w-full mb-2" placeholder="Filter databases..." value={q}
        onChange={(e) => { setQ(e.target.value); setSel(0); }}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown") { e.preventDefault(); setSel((s) => Math.min(list.length - 1, s + 1)); }
          if (e.key === "ArrowUp") { e.preventDefault(); setSel((s) => Math.max(0, s - 1)); }
          if (e.key === "Enter" && list[sel]) go(list[sel]);
        }}
      />
      <div className="max-h-80 overflow-auto rounded border border-border">
        {!dbs && <div className="p-3 text-dim flex items-center gap-2"><Loader2 size={14} className="animate-spin" /> Loading...</div>}
        {list.map((d, i) => (
          <div key={d} className={`tree-row px-2 ${i === sel ? "selected" : ""}`} onMouseEnter={() => setSel(i)} onClick={() => go(d)}>
            <Database size={13} className="opacity-70" />
            <span className="name">{d}</span>
            {d === session?.database && <span className="text-xs opacity-70">current</span>}
          </div>
        ))}
      </div>
      <div className="mt-3 flex items-center gap-2">
        {creating ? (
          <>
            <input type="text" className="flex-1 mono" placeholder="new_database" value={newName} onChange={(e) => setNewName(e.target.value)} />
            <button className="btn btn-primary" disabled={!newName.trim()} onClick={async () => {
              if (!session) return;
              try {
                await ipc.execute(session.sessionId, `CREATE DATABASE "${newName.replace(/"/g, '""')}"`);
                toast("ok", `Database ${newName} created`);
                setDbs(await ipc.listDatabases(session.sessionId));
                setCreating(false);
              } catch (e) { toast("err", errorMessage(e)); }
            }}>Create</button>
            <button className="btn" onClick={() => setCreating(false)}>Cancel</button>
          </>
        ) : (
          <button className="btn btn-ghost text-xs" onClick={() => setCreating(true)}>+ New database</button>
        )}
      </div>
    </Modal>
  );
}
