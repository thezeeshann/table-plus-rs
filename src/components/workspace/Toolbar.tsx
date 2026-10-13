import { PanelLeft, Database, RotateCw, Plus, Activity, Lock, Settings, History, Star, Terminal } from "lucide-react";
import { useApp, useActiveTab } from "@/store/app";
import { mod } from "@/lib/format";

export function Toolbar() {
  const session = useApp((s) => s.session);
  const connection = useApp((s) => s.connection);
  const status = useApp((s) => s.connectionStatus);
  const schema = useApp((s) => s.schema);
  const toggleSidebar = useApp((s) => s.toggleSidebar);
  const sidebarVisible = useApp((s) => s.sidebarVisible);
  const setOpenDb = useApp((s) => s.setOpenDatabaseDialog);
  const openQueryTab = useApp((s) => s.openQueryTab);
  const openProcesses = useApp((s) => s.openProcessesTab);
  const refreshListing = useApp((s) => s.refreshListing);
  const reconnect = useApp((s) => s.reconnect);
  const setSettings = useApp((s) => s.setSettingsDialog);
  const setSection = useApp((s) => s.setSidebarSection);
  const section = useApp((s) => s.sidebarSection);
  const tabActions = useApp((s) => s.tabActions);
  const tab = useActiveTab();
  if (!session) return null;

  const env = (connection?.environment || "LOCAL").toUpperCase();
  const where = tab?.type === "table" ? `${tab.schema}.${tab.name}` : tab?.type === "query" ? tab.title : schema;
  const pill = `${env} | PostgreSQL ${session.serverVersion}${session.tls ? ` : ${session.tls}` : ""} : ${session.connectionName} : ${session.database} : ${where}`;

  return (
    <div className="h-[52px] flex items-center pl-[84px] pr-3 gap-1 border-b border-border bg-app shrink-0" data-tauri-drag-region>
      <button className={`icon-btn ${sidebarVisible ? "" : ""}`} title={`Toggle sidebar (${mod}\\)`} onClick={toggleSidebar}><PanelLeft size={15} /></button>
      <span className="w-px h-5 bg-border mx-1" />
      <button className={`icon-btn ${section === "items" ? "active" : ""}`} title="Items" onClick={() => setSection("items")}><Database size={15} /></button>
      <button className={`icon-btn ${section === "queries" ? "active" : ""}`} title="Saved queries" onClick={() => setSection("queries")}><Star size={15} /></button>
      <button className={`icon-btn ${section === "history" ? "active" : ""}`} title="History" onClick={() => setSection("history")}><History size={15} /></button>
      <span className="w-px h-5 bg-border mx-1" />
      <button className="icon-btn" title={`Open database (${mod}K)`} onClick={() => setOpenDb(true)}><Lock size={15} /></button>
      <button className="icon-btn" title={`New SQL query (${mod}T)`} onClick={() => openQueryTab()}><Terminal size={15} /></button>
      <div
        className={`status-pill ${status === "disconnected" ? "disconnected" : status === "reconnecting" ? "reconnecting" : ""}`}
        title={status === "disconnected" ? `Disconnected – ${mod}⇧R to reconnect` : pill}
        onClick={() => { if (status === "disconnected") reconnect(); }}
        style={{ borderLeft: `4px solid ${connection ? `var(--conn-${connection.color})` : "transparent"}` }}
        data-tauri-drag-region
      >
        {status === "disconnected" ? `Database is disconnected! Press ${mod}⇧R to reconnect` : pill}
      </div>
      <button className="icon-btn" title={`Reload (${mod}R)`} onClick={() => { tab && tabActions[tab.id]?.reload?.(); refreshListing(); }}><RotateCw size={15} /></button>
      <button className="icon-btn" title={`Add row (${mod}I)`} disabled={!tab || tab.type !== "table"} onClick={() => tab && tabActions[tab.id]?.addRow?.()}><Plus size={15} /></button>
      <button className="icon-btn" title={`Process list (${mod}.)`} onClick={openProcesses}><Activity size={15} /></button>
      <button className="icon-btn" title={`Settings (${mod},)`} onClick={() => setSettings(true)}><Settings size={15} /></button>
    </div>
  );
}
