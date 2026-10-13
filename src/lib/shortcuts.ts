import { useEffect } from "react";
import { useApp } from "@/store/app";

/** Keyboard shortcuts mirroring TablePlus' menu (observed via its menu bar). */
export function useGlobalShortcuts() {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const s = useApp.getState();
      const meta = e.metaKey || e.ctrlKey;
      if (!meta) return;
      const key = e.key.toLowerCase();
      const active = s.activeTabId ? s.tabActions[s.activeTabId] : undefined;
      const inEditor = (e.target as HTMLElement)?.closest?.(".cm-editor, input, textarea");
      const run = (fn?: () => void) => { if (fn) { e.preventDefault(); fn(); } };

      if (s.screen !== "workspace") {
        if (key === "n" && !e.shiftKey) { e.preventDefault(); s.showConnectionForm(null); }
        return;
      }
      if (key === "enter") {
        if (e.shiftKey) return run(active?.runAll);
        return run(active?.runCurrent);
      }
      if (key === "s" && !e.shiftKey) return run(active?.commit);
      if (key === "p" && e.shiftKey) return run(active?.preview);
      if (key === "i" && !e.shiftKey && !inEditor) return run(active?.addRow);
      if (key === "d" && !e.shiftKey && !inEditor) return run(active?.duplicateRows);
      if (key === "backspace" && !inEditor) return run(active?.deleteRows);
      if (key === "r" && !e.shiftKey && !e.altKey) { e.preventDefault(); active?.reload?.(); s.refreshListing(); return; }
      if (key === "r" && e.shiftKey) { e.preventDefault(); s.reconnect(); return; }
      if (key === "t" && !e.shiftKey) { e.preventDefault(); s.openQueryTab(); return; }
      if (key === "w" && !e.shiftKey) { e.preventDefault(); if (s.activeTabId) s.closeTab(s.activeTabId); return; }
      if (key === "k" && !e.shiftKey) { e.preventDefault(); s.setOpenDatabaseDialog(true); return; }
      if (key === "e" && e.shiftKey) return run(active?.exportData);
      if (key === "." && !e.shiftKey) { e.preventDefault(); s.openProcessesTab(); return; }
      if (key === "," ) { e.preventDefault(); s.setSettingsDialog(true); return; }
      if (key === "f" && !inEditor) return run(active?.find);
      if (key === "escape") return run(active?.discard);
      if (key === "\\" ) { e.preventDefault(); s.toggleSidebar(); return; }
      if (key === "1" && e.shiftKey) return run(() => active?.setView?.("data"));
      if (key === "2" && e.shiftKey) return run(() => active?.setView?.("structure"));
      if (key === "3" && e.shiftKey) return run(() => active?.setView?.("index"));
      if ((key === "}" || key === "]") && e.shiftKey) { e.preventDefault(); cycle(1); return; }
      if ((key === "{" || key === "[") && e.shiftKey) { e.preventDefault(); cycle(-1); return; }
      if (/^[1-9]$/.test(key) && !e.shiftKey && !e.altKey) {
        const idx = Number(key) - 1;
        const t = s.tabs[idx];
        if (t) { e.preventDefault(); s.activateTab(t.id); }
      }
    };
    const cycle = (dir: number) => {
      const s = useApp.getState();
      if (!s.tabs.length) return;
      const i = s.tabs.findIndex((t) => t.id === s.activeTabId);
      const n = (i + dir + s.tabs.length) % s.tabs.length;
      s.activateTab(s.tabs[n].id);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}
