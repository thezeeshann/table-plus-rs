import { useMemo, useState } from "react";
import { Plus, Search, Settings, Database, Loader2, FolderPlus } from "lucide-react";
import { useApp } from "@/store/app";
import { ContextMenu, useContextMenu } from "@/components/common/ContextMenu";
import { Confirm } from "@/components/common/Confirm";
import { ipc, errorMessage } from "@/lib/ipc";
import type { ConnectionConfig } from "@/lib/types";
import { SettingsDialog } from "@/components/workspace/dialogs/SettingsDialog";
import { Modal, Field } from "@/components/common/Modal";

export const CONNECTION_COLORS: Record<string, string> = {
  "1": "var(--conn-1)", "2": "var(--conn-2)", "3": "var(--conn-3)", "4": "var(--conn-4)", "5": "var(--conn-5)", "0": "#525252",
};

export function envBadge(env: string) {
  const map: Record<string, string> = { production: "var(--status-err)", staging: "var(--status-warn)", testing: "var(--conn-3)", development: "var(--conn-2)", local: "var(--conn-1)" };
  const color = map[env.toLowerCase()];
  if (!color) return null;
  return <span className="env-badge" style={{ background: color, color: "var(--on-accent)" }}>{env}</span>;
}

export function WelcomeScreen() {
  const connections = useApp((s) => s.connections);
  const groups = useApp((s) => s.groups);
  const connecting = useApp((s) => s.connecting);
  const connect = useApp((s) => s.connect);
  const showForm = useApp((s) => s.showConnectionForm);
  const loadConnections = useApp((s) => s.loadConnections);
  const toast = useApp((s) => s.toast);
  const settingsOpen = useApp((s) => s.settingsDialog);
  const setSettings = useApp((s) => s.setSettingsDialog);
  const [search, setSearch] = useState("");
  const [selected, setSelected] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<ConnectionConfig | null>(null);
  const [groupDialog, setGroupDialog] = useState<{ id: string; name: string } | null>(null);
  const { menu, open, close } = useContextMenu();

  const filtered = useMemo(() => {
    const q = search.trim().toLowerCase();
    return connections.filter((c) => !q || c.name.toLowerCase().includes(q) || c.host.toLowerCase().includes(q) || c.database.toLowerCase().includes(q));
  }, [connections, search]);

  const grouped = useMemo(() => {
    const byGroup = new Map<string | null, ConnectionConfig[]>();
    for (const c of filtered) {
      const g = c.groupId && groups.some((x) => x.id === c.groupId) ? c.groupId : null;
      byGroup.set(g, [...(byGroup.get(g) ?? []), c]);
    }
    const out: { id: string | null; name: string; items: ConnectionConfig[] }[] = [];
    for (const g of groups) if (byGroup.has(g.id) || !search) out.push({ id: g.id, name: g.name, items: byGroup.get(g.id) ?? [] });
    out.push({ id: null, name: "", items: byGroup.get(null) ?? [] });
    return out;
  }, [filtered, groups, search]);

  const doConnect = async (c: ConnectionConfig) => {
    try { await connect(c.id); } catch { /* toast already shown */ }
  };

  const copyUrl = (c: ConnectionConfig) => {
    const url = `postgresql://${encodeURIComponent(c.user)}@${c.host}:${c.port}/${c.database}${c.sslMode !== "prefer" ? `?sslmode=${c.sslMode}` : ""}`;
    navigator.clipboard.writeText(url);
    toast("ok", "Connection URL copied");
  };

  const duplicate = async (c: ConnectionConfig) => {
    try {
      const full = await ipc.getConnection(c.id);
      await ipc.saveConnection({ ...full, id: "", name: `${c.name} copy`, createdAt: null });
      loadConnections();
    } catch (e) { toast("err", errorMessage(e)); }
  };

  const itemMenu = (c: ConnectionConfig) => [
    { label: "Connect", onClick: () => doConnect(c) },
    { label: "Edit...", shortcut: "⌘E", onClick: () => showForm(c) },
    { label: "Duplicate", onClick: () => duplicate(c) },
    { label: "Copy connection URL", onClick: () => copyUrl(c) },
    { separator: true },
    ...(groups.length ? [{ label: c.groupId ? "Remove from group" : "Move to group...", onClick: async () => {
      if (c.groupId) { await ipc.saveConnection({ ...(await ipc.getConnection(c.id)), groupId: null }); loadConnections(); }
      else { const g = groups[0]; await ipc.saveConnection({ ...(await ipc.getConnection(c.id)), groupId: g.id }); loadConnections(); }
    } }, { separator: true }] : []),
    { label: "Delete", danger: true, onClick: () => setConfirmDelete(c) },
  ];

  return (
    <div className="h-full flex flex-col" data-tauri-drag-region>
      <div className="h-[52px] flex items-center pl-[84px] pr-3 gap-2 border-b border-border bg-app" data-tauri-drag-region>
        <div className="relative flex-1 max-w-md">
          <Search size={13} className="absolute left-2 top-[7px] text-dim" />
          <input type="text" placeholder="Search for connection..." className="w-full !pl-7" value={search} onChange={(e) => setSearch(e.target.value)} />
        </div>
        <div className="flex-1" data-tauri-drag-region />
        <button className="icon-btn" title="New group" onClick={() => setGroupDialog({ id: "", name: "" })}><FolderPlus size={15} /></button>
        <button className="icon-btn" title="Settings (⌘,)" onClick={() => setSettings(true)}><Settings size={15} /></button>
        <button className="btn btn-primary" onClick={() => showForm(null)}><Plus size={14} /> New Connection</button>
      </div>
      <div className="flex-1 overflow-auto p-3" onClick={() => setSelected(null)}>
        {connections.length === 0 && (
          <div className="h-full flex flex-col items-center justify-center text-dim gap-3">
            <Database size={40} />
            <div>No connections yet.</div>
            <button className="btn btn-primary" onClick={(e) => { e.stopPropagation(); showForm(null); }}>Create a connection (⌘N)</button>
          </div>
        )}
        {grouped.map((g) => (
          <div key={g.id ?? "_"} className="mb-2">
            {g.id && (
              <div
                className="flex items-center gap-2 px-2 py-1 text-xs font-semibold text-dim uppercase tracking-wide"
                onContextMenu={(e) => open(e, [
                  { label: "Rename group", onClick: () => setGroupDialog({ id: g.id!, name: g.name }) },
                  { label: "Delete group", danger: true, onClick: async () => { await ipc.deleteGroup(g.id!); loadConnections(); } },
                ])}
              >
                {g.name}
              </div>
            )}
            <div className="grid gap-2" style={{ gridTemplateColumns: "repeat(auto-fill, minmax(260px, 1fr))" }}>
              {g.items.map((c) => (
                <div
                  key={c.id}
                  className={`flex items-center gap-3 px-3 py-2 rounded-lg border cursor-default ${selected === c.id ? "border-accent bg-selection" : "border-border bg-control hover:bg-tab-hover"}`}
                  onClick={(e) => { e.stopPropagation(); setSelected(c.id); }}
                  onDoubleClick={() => doConnect(c)}
                  onContextMenu={(e) => { setSelected(c.id); open(e, itemMenu(c)); }}
                  onKeyDown={(e) => { if (e.key === "Enter") doConnect(c); }}
                  tabIndex={0}
                >
                  <span className="w-9 h-9 rounded-md flex items-center justify-center" style={{ background: CONNECTION_COLORS[c.color] ?? CONNECTION_COLORS["0"] }}>
                    {connecting && selected === c.id ? <Loader2 size={18} className="animate-spin" /> : <Database size={18} />}
                  </span>
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2"><span className="font-semibold truncate-1">{c.name}</span>{envBadge(c.environment)}</div>
                    <div className="text-dim text-xs truncate-1">{c.user}@{c.host}:{c.port}/{c.database}</div>
                  </div>
                </div>
              ))}
            </div>
          </div>
        ))}
      </div>
      <div className="h-7 flex items-center px-3 text-xs text-dim border-t border-border">
        {connections.length} connection{connections.length === 1 ? "" : "s"} · double-click to open · PostgreSQL only
      </div>
      <ContextMenu menu={menu} onClose={close} />
      {confirmDelete && (
        <Confirm
          title={`Delete "${confirmDelete.name}"?`}
          message="The connection and its query history will be removed."
          confirmLabel="Delete"
          danger
          onCancel={() => setConfirmDelete(null)}
          onConfirm={async () => {
            try { await ipc.deleteConnection(confirmDelete.id); loadConnections(); } catch (e) { toast("err", errorMessage(e)); }
            setConfirmDelete(null);
          }}
        />
      )}
      {groupDialog && (
        <Modal
          title={groupDialog.id ? "Rename group" : "New group"}
          onClose={() => setGroupDialog(null)}
          footer={<>
            <button className="btn" onClick={() => setGroupDialog(null)}>Cancel</button>
            <button className="btn btn-primary" disabled={!groupDialog.name.trim()} onClick={async () => {
              await ipc.saveGroup({ id: groupDialog.id, name: groupDialog.name.trim() });
              setGroupDialog(null); loadConnections();
            }}>Save</button>
          </>}
        >
          <Field label="Name"><input type="text" autoFocus className="flex-1" value={groupDialog.name} onChange={(e) => setGroupDialog({ ...groupDialog, name: e.target.value })} /></Field>
        </Modal>
      )}
      {settingsOpen && <SettingsDialog />}
    </div>
  );
}
