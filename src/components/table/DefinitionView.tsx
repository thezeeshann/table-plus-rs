import { useEffect, useState } from "react";
import { Copy, Loader2, Terminal } from "lucide-react";
import { useApp, type Tab } from "@/store/app";
import { ipc, errorMessage } from "@/lib/ipc";
import { SqlEditor } from "@/components/query/SqlEditor";

export function DefinitionView({ tab }: { tab: Tab }) {
  const session = useApp((s) => s.session)!;
  const listing = useApp((s) => s.listing);
  const openQueryTab = useApp((s) => s.openQueryTab);
  const toast = useApp((s) => s.toast);
  const [ddl, setDdl] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const kind = tab.kind ?? "table";
  const seq = kind === "sequence" ? listing?.sequences.find((s) => s.name === tab.name) : null;

  useEffect(() => {
    setDdl(null); setErr(null);
    if (kind === "sequence") return;
    ipc.objectDdl(session.sessionId, tab.schema!, tab.name!, kind).then(setDdl).catch((e) => setErr(errorMessage(e)));
  }, [session.sessionId, tab.schema, tab.name, kind]);

  if (kind === "sequence") {
    return (
      <div className="flex-1 p-4 overflow-auto">
        <div className="text-dim mb-2">Sequence {tab.schema}.{tab.name}</div>
        {seq ? (
          <table className="text-xs mono">
            <tbody>
              {Object.entries({ "Start value": seq.startValue, "Min value": seq.minValue, "Max value": seq.maxValue, "Increment by": seq.incrementBy, "Last value": seq.lastValue, "Cache size": seq.cacheSize, Comment: seq.comment }).map(([k, v]) => (
                <tr key={k}><td className="pr-4 py-0.5 text-dim">{k}</td><td className="selectable">{v ?? "—"}</td></tr>
              ))}
            </tbody>
          </table>
        ) : <div className="text-dim">Not found in current listing.</div>}
      </div>
    );
  }

  return (
    <div className="flex-1 flex flex-col min-h-0">
      <div className="flex items-center gap-2 px-2 h-8 border-b border-border shrink-0 text-xs text-dim">
        <span className="flex-1 truncate-1">Definition of {tab.schema}.{tab.name}</span>
        <button className="btn !py-[2px]" disabled={!ddl} onClick={() => { if (ddl) { navigator.clipboard.writeText(ddl); toast("ok", "Copied"); } }}><Copy size={12} /> Copy</button>
        <button className="btn !py-[2px]" disabled={!ddl} onClick={() => ddl && openQueryTab(ddl, `${tab.name} DDL`)}><Terminal size={12} /> Open in editor</button>
      </div>
      {err && <div className="p-3 text-[#ff7b7b] selectable">{err}</div>}
      {!ddl && !err && <div className="p-3 text-dim flex items-center gap-2"><Loader2 size={13} className="animate-spin" /> Loading…</div>}
      {ddl && <div className="flex-1 min-h-0"><SqlEditor value={ddl} readOnly /></div>}
    </div>
  );
}
