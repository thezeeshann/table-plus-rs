import { useState } from "react";
import { X, Plus, Table2, Terminal, Activity, Eye, Layers, FunctionSquare, Hash } from "lucide-react";
import { useApp, type Tab } from "@/store/app";
import { ContextMenu, useContextMenu } from "@/components/common/ContextMenu";
import { Modal, Field } from "@/components/common/Modal";

export function tabIcon(t: Tab, size = 13) {
  if (t.type === "query") return <Terminal size={size} />;
  if (t.type === "processes") return <Activity size={size} />;
  switch (t.kind) {
    case "view": return <Eye size={size} />;
    case "matview": return <Layers size={size} />;
    case "function": return <FunctionSquare size={size} />;
    case "sequence": return <Hash size={size} />;
    default: return <Table2 size={size} />;
  }
}

export function TabBar() {
  const tabs = useApp((s) => s.tabs);
  const activeTabId = useApp((s) => s.activeTabId);
  const activate = useApp((s) => s.activateTab);
  const close = useApp((s) => s.closeTab);
  const closeOthers = useApp((s) => s.closeOtherTabs);
  const pin = useApp((s) => s.pinTab);
  const rename = useApp((s) => s.renameTab);
  const openQuery = useApp((s) => s.openQueryTab);
  const { menu, open, close: closeMenu } = useContextMenu();
  const [renaming, setRenaming] = useState<{ id: string; title: string } | null>(null);

  const tryClose = (t: Tab) => {
    if (t.dirty && !confirm(`"${t.title}" has unsaved changes. Discard them?`)) return;
    close(t.id);
  };

  return (
    <div className="h-[30px] flex items-stretch bg-app border-b border-border shrink-0 overflow-x-auto">
      {tabs.map((t) => (
        <div
          key={t.id}
          className={`tab ${t.id === activeTabId ? "active" : ""} ${t.pinned ? "" : "preview"}`}
          onMouseDown={(e) => { if (e.button === 1) { e.preventDefault(); tryClose(t); } else activate(t.id); }}
          onDoubleClick={() => pin(t.id)}
          onContextMenu={(e) => open(e, [
            { label: "Close Tab", shortcut: "⌘W", onClick: () => tryClose(t) },
            { label: "Close other Tabs", onClick: () => closeOthers(t.id) },
            { separator: true },
            { label: "Rename Tab", onClick: () => setRenaming({ id: t.id, title: t.title }) },
            { label: t.pinned ? "Pinned" : "Pin Tab", disabled: t.pinned, onClick: () => pin(t.id) },
          ])}
          title={t.type === "table" ? `${t.schema}.${t.name}` : t.title}
        >
          <span className="opacity-70">{tabIcon(t)}</span>
          <span className="label truncate-1 flex-1">{t.dirty ? "● " : ""}{t.title}</span>
          <span className="close" onMouseDown={(e) => e.stopPropagation()} onClick={(e) => { e.stopPropagation(); tryClose(t); }}><X size={12} /></span>
        </div>
      ))}
      <button className="w-8 flex items-center justify-center text-dim hover:text-fg hover:bg-tab-hover" title="New SQL query (⌘T)" onClick={() => openQuery()}><Plus size={14} /></button>
      <ContextMenu menu={menu} onClose={closeMenu} />
      {renaming && (
        <Modal title="Rename Tab" onClose={() => setRenaming(null)} footer={<>
          <button className="btn" onClick={() => setRenaming(null)}>Cancel</button>
          <button className="btn btn-primary" onClick={() => { rename(renaming.id, renaming.title.trim() || "Untitled"); setRenaming(null); }}>Rename</button>
        </>}>
          <Field label="Tab name"><input type="text" autoFocus className="flex-1" value={renaming.title} onChange={(e) => setRenaming({ ...renaming, title: e.target.value })} onKeyDown={(e) => { if (e.key === "Enter") { rename(renaming.id, renaming.title.trim() || "Untitled"); setRenaming(null); } }} /></Field>
        </Modal>
      )}
    </div>
  );
}
