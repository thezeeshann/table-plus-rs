import { useApp } from "@/store/app";
import { Toolbar } from "./Toolbar";
import { Sidebar } from "./Sidebar";
import { TabBar } from "./TabBar";
import { Splitter } from "@/components/common/Splitter";
import { TableTab } from "@/components/table/TableTab";
import { QueryTab } from "@/components/query/QueryTab";
import { ProcessListTab } from "@/components/tools/ProcessListTab";
import { OpenDatabaseDialog } from "./dialogs/OpenDatabaseDialog";
import { SettingsDialog } from "./dialogs/SettingsDialog";
import { CreateTableDialog } from "./dialogs/CreateTableDialog";
import { RenameDialog } from "./dialogs/RenameDialog";
import { Database } from "lucide-react";
import { mod } from "@/lib/format";

export function Workspace() {
  const tabs = useApp((s) => s.tabs);
  const activeTabId = useApp((s) => s.activeTabId);
  const sidebarVisible = useApp((s) => s.sidebarVisible);
  const sidebarWidth = useApp((s) => s.sidebarWidth);
  const setSidebarWidth = useApp((s) => s.setSidebarWidth);
  const openDb = useApp((s) => s.openDatabaseDialog);
  const settings = useApp((s) => s.settingsDialog);
  const createTable = useApp((s) => s.createTableDialog);
  const rename = useApp((s) => s.renameDialog);

  return (
    <div className="h-full flex flex-col">
      <Toolbar />
      <div className="flex-1 flex min-h-0">
        {sidebarVisible && (
          <>
            <div style={{ width: sidebarWidth }} className="shrink-0 bg-sidebar flex flex-col min-h-0 border-r border-border">
              <Sidebar />
            </div>
            <Splitter direction="h" onDrag={(d) => setSidebarWidth(useApp.getState().sidebarWidth + d)} />
          </>
        )}
        <div className="flex-1 flex flex-col min-w-0 min-h-0 bg-content">
          <TabBar />
          <div className="flex-1 min-h-0 relative">
            {tabs.length === 0 && (
              <div className="absolute inset-0 flex flex-col items-center justify-center text-dim gap-2">
                <Database size={36} />
                <div>Select a table in the sidebar, or press <span className="kbd">{mod}T</span> for a SQL query tab.</div>
                <div className="text-xs">Single-click previews · double-click pins · {mod}K switches database</div>
              </div>
            )}
            {tabs.map((t) => (
              <div key={t.id} className="absolute inset-0 flex flex-col" style={{ display: t.id === activeTabId ? "flex" : "none" }}>
                {t.type === "table" && <TableTab tab={t} active={t.id === activeTabId} />}
                {t.type === "query" && <QueryTab tab={t} active={t.id === activeTabId} />}
                {t.type === "processes" && <ProcessListTab tab={t} active={t.id === activeTabId} />}
              </div>
            ))}
          </div>
        </div>
      </div>
      {openDb && <OpenDatabaseDialog />}
      {settings && <SettingsDialog />}
      {createTable && <CreateTableDialog />}
      {rename && <RenameDialog />}
    </div>
  );
}
