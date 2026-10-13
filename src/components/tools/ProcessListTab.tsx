import { useCallback, useEffect, useState } from "react";
import { RotateCw, XCircle, Skull } from "lucide-react";
import { useApp, type Tab } from "@/store/app";
import { ipc, errorMessage } from "@/lib/ipc";
import type { ProcessInfo } from "@/lib/types";
import { DataGrid, type Selection } from "@/components/table/DataGrid";

export function ProcessListTab({ tab, active }: { tab: Tab; active: boolean }) {
  const session = useApp((s) => s.session)!;
  const toast = useApp((s) => s.toast);
  const register = useApp((s) => s.registerTabActions);
  const [procs, setProcs] = useState<ProcessInfo[]>([]);
  const [sel, setSel] = useState<Selection | null>(null);
  const [auto, setAuto] = useState(false);
  const load = useCallback(async () => { try { setProcs(await ipc.listProcesses(session.sessionId)); } catch (e) { toast("err", errorMessage(e)); } }, [session.sessionId, toast]);
  useEffect(() => { load(); }, [load]);
  useEffect(() => { if (!auto || !active) return; const t = setInterval(load, 2000); return () => clearInterval(t); }, [auto, active, load]);
  useEffect(() => { register(tab.id, { reload: load }); }, [tab.id, register, load]);

  const columns = [
    { name: "pid", type: "int4" }, { name: "user", type: "name" }, { name: "database", type: "name" }, { name: "application", type: "text" },
    { name: "client", type: "inet" }, { name: "state", type: "text" }, { name: "wait_event", type: "text" }, { name: "query_start", type: "timestamptz" }, { name: "query", type: "text" },
  ];
  const rows = procs.map((p) => [String(p.pid), p.user, p.database, p.application, p.client, p.state, p.waitEvent, p.queryStart, p.query?.replace(/\s+/g, " ") ?? null]);
  const selectedPid = sel ? procs[sel.focus.r]?.pid : null;
  const act = async (terminate: boolean) => {
    if (selectedPid == null) return;
    if (!confirm(`${terminate ? "Terminate" : "Cancel"} backend ${selectedPid}?`)) return;
    try { await ipc.killProcess(session.sessionId, selectedPid, terminate); toast("ok", `${terminate ? "Terminated" : "Cancelled"} ${selectedPid}`); load(); } catch (e) { toast("err", errorMessage(e)); }
  };
  return (
    <div className="flex-1 flex flex-col min-h-0">
      <div className="h-8 flex items-center gap-1 px-2 border-b border-border bg-app shrink-0">
        <button className="btn !py-[2px]" onClick={load}><RotateCw size={12} /> Refresh</button>
        <label className="flex items-center gap-1 text-xs ml-2"><input type="checkbox" checked={auto} onChange={(e) => setAuto(e.target.checked)} />Auto refresh (2s)</label>
        <div className="flex-1" />
        <button className="btn !py-[2px]" disabled={selectedPid == null} onClick={() => act(false)}><XCircle size={12} /> Cancel query</button>
        <button className="btn !py-[2px] btn-danger" disabled={selectedPid == null} onClick={() => act(true)}><Skull size={12} /> Terminate</button>
      </div>
      <DataGrid columns={columns} rows={rows} onSelectionChange={setSel} emptyText="No other backends" />
    </div>
  );
}
