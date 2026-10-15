import { create } from "zustand";
import { ipc, errorMessage } from "@/lib/ipc";
import * as theme from "@/lib/theme";
import type {
  AppSettings, ConnectionConfig, ConnectionGroup, FavoriteQuery, HistoryEntry,
  ItemKind, SchemaListing, SessionInfo,
} from "@/lib/types";

export type TabType = "table" | "query" | "definition" | "processes";
export type TableView = "data" | "structure" | "index" | "definition";

export interface Tab {
  id: string;
  type: TabType;
  title: string;
  pinned: boolean;
  schema?: string;
  name?: string;
  kind?: ItemKind;
  sql?: string;
  view?: TableView;
  dirty?: boolean;
  /** Pre-applied filter when opening via a foreign key arrow. */
  initialFilter?: { column: string; value: string | null } | null;
}

export interface TabActions {
  commit?: () => void;
  discard?: () => void;
  preview?: () => void;
  addRow?: () => void;
  duplicateRows?: () => void;
  deleteRows?: () => void;
  reload?: () => void;
  runCurrent?: () => void;
  runAll?: () => void;
  stop?: () => void;
  find?: () => void;
  exportData?: () => void;
  setView?: (v: TableView) => void;
}

export interface Toast { id: number; kind: "ok" | "warn" | "err"; text: string }

export type SidebarSection = "items" | "queries" | "history";

interface AppStore {
  settings: AppSettings;
  connections: ConnectionConfig[];
  groups: ConnectionGroup[];
  favorites: FavoriteQuery[];
  screen: "welcome" | "workspace";
  session: SessionInfo | null;
  connection: ConnectionConfig | null;
  connecting: boolean;
  connectionStatus: "connected" | "disconnected" | "reconnecting";
  schemas: string[];
  schema: string;
  listing: SchemaListing | null;
  listingLoading: boolean;
  tabs: Tab[];
  activeTabId: string | null;
  sidebarSection: SidebarSection;
  sidebarVisible: boolean;
  sidebarWidth: number;
  history: HistoryEntry[];
  toasts: Toast[];
  tabActions: Record<string, TabActions>;
  /** Tab ids with a fetch in flight; drives the toolbar's loading fill. */
  busyTabs: Record<string, true>;
  connectionForm: { open: boolean; config: ConnectionConfig | null };
  openDatabaseDialog: boolean;
  settingsDialog: boolean;
  /** Where the live theme came from, and any --theme / $TABLE_PLUS_RS_THEME request that overrides the saved one. */
  themeSource: theme.Source;
  themeOverride: { flag: string | null; env: string | null };
  createTableDialog: boolean;
  renameDialog: { open: boolean; schema: string; name: string; kind: ItemKind } | null;

  init: () => Promise<void>;
  loadConnections: () => Promise<void>;
  loadFavorites: () => Promise<void>;
  saveSettings: (s: Partial<AppSettings>) => Promise<void>;
  /** Persist `id` as the saved theme and, unless a launch override is active, switch to it. */
  saveTheme: (id: string) => Promise<void>;
  connect: (connectionId: string, database?: string) => Promise<void>;
  disconnect: () => Promise<void>;
  reconnect: () => Promise<void>;
  setSchema: (schema: string) => Promise<void>;
  refreshListing: () => Promise<void>;
  loadHistory: (search?: string) => Promise<void>;
  openItem: (schema: string, name: string, kind: ItemKind, opts?: { pinned?: boolean; view?: TableView; filter?: Tab["initialFilter"] }) => void;
  openQueryTab: (sql?: string, title?: string) => string;
  openProcessesTab: () => void;
  closeTab: (id: string) => void;
  closeOtherTabs: (id: string) => void;
  activateTab: (id: string) => void;
  pinTab: (id: string) => void;
  renameTab: (id: string, title: string) => void;
  updateTab: (id: string, patch: Partial<Tab>) => void;
  setDirty: (id: string, dirty: boolean) => void;
  setTabBusy: (id: string, busy: boolean) => void;
  registerTabActions: (id: string, actions: TabActions) => void;
  toast: (kind: Toast["kind"], text: string) => void;
  dismissToast: (id: number) => void;
  setSidebarSection: (s: SidebarSection) => void;
  toggleSidebar: () => void;
  setSidebarWidth: (w: number) => void;
  showConnectionForm: (config: ConnectionConfig | null) => void;
  hideConnectionForm: () => void;
  setOpenDatabaseDialog: (v: boolean) => void;
  setSettingsDialog: (v: boolean) => void;
  setCreateTableDialog: (v: boolean) => void;
  setRenameDialog: (v: AppStore["renameDialog"]) => void;
}

let toastSeq = 1;
let tabSeq = 1;

export const defaultSettings: AppSettings = {
  defaultLimit: 300,
  estimateCountThreshold: 100000,
  theme: "default",
  editorFontSize: 13,
  historyKeepDays: 0,
  autoCloseBrackets: true,
  lastConnectionId: null,
};

export const useApp = create<AppStore>((set, get) => ({
  settings: defaultSettings,
  connections: [],
  groups: [],
  favorites: [],
  screen: "welcome",
  session: null,
  connection: null,
  connecting: false,
  connectionStatus: "connected",
  schemas: [],
  schema: "public",
  listing: null,
  listingLoading: false,
  tabs: [],
  activeTabId: null,
  sidebarSection: "items",
  sidebarVisible: true,
  sidebarWidth: 270,
  history: [],
  toasts: [],
  tabActions: {},
  busyTabs: {},
  connectionForm: { open: false, config: null },
  openDatabaseDialog: false,
  settingsDialog: false,
  themeSource: "default",
  themeOverride: { flag: null, env: null },
  createTableDialog: false,
  renameDialog: null,

  init: async () => {
    try {
      const settings = await ipc.getSettings();
      set({ settings: { ...defaultSettings, ...settings } });
    } catch (e) {
      console.warn(e);
    }
    let override = { flag: null as string | null, env: null as string | null };
    try { override = await ipc.launchTheme(); } catch (e) { console.warn(e); }
    const r = theme.resolve(override.flag, override.env, get().settings.theme);
    if (r.ok) {
      theme.setActive(r.choice.index);
      set({ themeSource: r.choice.source, themeOverride: override });
      if (r.choice.warning) get().toast("warn", r.choice.warning);
    } else {
      // A bad --theme is an error, but it must never lock the user out of the app:
      // report it and carry on with the saved/default theme.
      get().toast("err", r.error);
      const fallback = theme.resolve(null, override.env, get().settings.theme);
      if (fallback.ok) { theme.setActive(fallback.choice.index); set({ themeSource: fallback.choice.source }); }
      set({ themeOverride: override });
    }
    theme.writeCached(theme.normalize(get().settings.theme));
    await Promise.all([get().loadConnections(), get().loadFavorites()]);
  },

  loadConnections: async () => {
    try {
      const [connections, groups] = await ipc.listConnections();
      set({ connections, groups });
    } catch (e) {
      get().toast("err", errorMessage(e));
    }
  },

  loadFavorites: async () => {
    try {
      set({ favorites: await ipc.listFavorites() });
    } catch (e) {
      get().toast("err", errorMessage(e));
    }
  },

  saveSettings: async (patch) => {
    const settings = { ...get().settings, ...patch };
    set({ settings });
    try {
      await ipc.saveSettings(settings);
    } catch (e) {
      get().toast("err", errorMessage(e));
    }
  },

  saveTheme: async (id) => {
    const { themeOverride } = get();
    await get().saveSettings({ theme: id });
    theme.writeCached(id);
    const overridden = themeOverride.flag?.trim() || themeOverride.env?.trim();
    if (overridden) {
      const by = themeOverride.flag?.trim() ? "--theme" : "$TABLE_PLUS_RS_THEME";
      // The picker previewed other themes; put the override back.
      theme.setActive(theme.find((themeOverride.flag?.trim() ? themeOverride.flag : themeOverride.env) ?? "") ?? 0);
      get().toast("ok", `Theme saved. ${by} is overriding it for this run.`);
      return;
    }
    theme.setActive(theme.find(id) ?? 0);
    set({ themeSource: "saved" });
    get().toast("ok", `Theme set to ${theme.THEMES[theme.find(id) ?? 0].name}.`);
  },

  connect: async (connectionId, database) => {
    const { session } = get();
    set({ connecting: true });
    try {
      const connection = await ipc.getConnection(connectionId);
      const info = await ipc.openSession(connectionId, database);
      if (session) {
        ipc.closeSession(session.sessionId).catch(() => {});
      }
      const schemas = await ipc.listSchemas(info.sessionId);
      const preferred = connection.lastOpenedSchema && schemas.includes(connection.lastOpenedSchema)
        ? connection.lastOpenedSchema
        : schemas.includes("public") ? "public" : schemas[0] ?? "public";
      set({
        session: info,
        connection: { ...connection, password: undefined },
        screen: "workspace",
        connectionStatus: "connected",
        schemas,
        schema: preferred,
        tabs: [],
        activeTabId: null,
        listing: null,
        history: [],
      });
      await get().refreshListing();
      get().loadHistory();
      const title = `${info.connectionName} : ${info.database} - PostgreSQL ${info.serverVersion}`;
      import("@tauri-apps/api/window").then(({ getCurrentWindow }) => getCurrentWindow().setTitle(title)).catch(() => {});
      if (database && database !== connection.lastOpenedDatabase) {
        ipc.saveConnection({ ...connection, lastOpenedDatabase: database, password: undefined }).catch(() => {});
      }
    } catch (e) {
      get().toast("err", `Can't connect: ${errorMessage(e)}`);
      throw e;
    } finally {
      set({ connecting: false });
    }
  },

  disconnect: async () => {
    const { session } = get();
    if (session) await ipc.closeSession(session.sessionId).catch(() => {});
    set({ session: null, connection: null, screen: "welcome", tabs: [], activeTabId: null, listing: null, tabActions: {}, busyTabs: {} });
    import("@tauri-apps/api/window").then(({ getCurrentWindow }) => getCurrentWindow().setTitle("table-plus-rs")).catch(() => {});
    get().loadConnections();
  },

  reconnect: async () => {
    const { session } = get();
    if (!session) return;
    set({ connectionStatus: "reconnecting" });
    try {
      const info = await ipc.reconnectSession(session.sessionId);
      set({ session: info, connectionStatus: "connected" });
      get().toast("ok", "Reconnected");
      await get().refreshListing();
    } catch (e) {
      set({ connectionStatus: "disconnected" });
      get().toast("err", `Reconnect failed: ${errorMessage(e)}`);
    }
  },

  setSchema: async (schema) => {
    set({ schema });
    const { connection } = get();
    if (connection && connection.lastOpenedSchema !== schema) {
      ipc.saveConnection({ ...connection, lastOpenedSchema: schema, password: undefined }).catch(() => {});
      set({ connection: { ...connection, lastOpenedSchema: schema } });
    }
    await get().refreshListing();
  },

  refreshListing: async () => {
    const { session, schema } = get();
    if (!session) return;
    set({ listingLoading: true });
    try {
      const listing = await ipc.listSchemaItems(session.sessionId, schema);
      set({ listing, connectionStatus: "connected" });
    } catch (e) {
      const msg = errorMessage(e);
      if (/closed|connection/i.test(msg)) set({ connectionStatus: "disconnected" });
      get().toast("err", msg);
    } finally {
      set({ listingLoading: false });
    }
  },

  loadHistory: async (search) => {
    const { session } = get();
    if (!session) return;
    try {
      set({ history: await ipc.listHistory(session.connectionId, 500, search) });
    } catch (e) {
      console.warn(e);
    }
  },

  openItem: (schema, name, kind, opts = {}) => {
    const { tabs } = get();
    const view: TableView = opts.view ?? (kind === "function" || kind === "sequence" ? "definition" : "data");
    const existing = tabs.find((t) => t.type === "table" && t.schema === schema && t.name === name);
    if (existing && !opts.filter) {
      set({ activeTabId: existing.id });
      if (opts.pinned && !existing.pinned) get().pinTab(existing.id);
      if (opts.view) get().updateTab(existing.id, { view: opts.view });
      return;
    }
    const tab: Tab = {
      id: `tab-${tabSeq++}`,
      type: "table",
      title: name,
      pinned: !!opts.pinned || !!opts.filter,
      schema,
      name,
      kind,
      view,
      initialFilter: opts.filter ?? null,
    };
    // TablePlus behaviour: a single click reuses the current preview tab.
    const preview = tabs.find((t) => !t.pinned && !t.dirty);
    if (preview && !tab.pinned) {
      set({ tabs: tabs.map((t) => (t.id === preview.id ? { ...tab, id: preview.id } : t)), activeTabId: preview.id });
      return;
    }
    set({ tabs: [...tabs, tab], activeTabId: tab.id });
  },

  openQueryTab: (sql, title) => {
    const { tabs } = get();
    const n = tabs.filter((t) => t.type === "query").length + 1;
    const tab: Tab = { id: `tab-${tabSeq++}`, type: "query", title: title ?? (n === 1 ? "SQL Query" : `SQL Query ${n}`), pinned: true, sql: sql ?? "" };
    set({ tabs: [...tabs, tab], activeTabId: tab.id });
    return tab.id;
  },

  openProcessesTab: () => {
    const { tabs } = get();
    const existing = tabs.find((t) => t.type === "processes");
    if (existing) return set({ activeTabId: existing.id });
    const tab: Tab = { id: `tab-${tabSeq++}`, type: "processes", title: "Process List", pinned: true };
    set({ tabs: [...tabs, tab], activeTabId: tab.id });
  },

  closeTab: (id) => {
    const { tabs, activeTabId, tabActions } = get();
    const idx = tabs.findIndex((t) => t.id === id);
    if (idx < 0) return;
    const next = tabs.filter((t) => t.id !== id);
    let active = activeTabId;
    if (activeTabId === id) active = next[Math.min(idx, next.length - 1)]?.id ?? null;
    const { [id]: _, ...rest } = tabActions;
    const { [id]: __, ...busy } = get().busyTabs;
    set({ tabs: next, activeTabId: active, tabActions: rest, busyTabs: busy });
  },

  closeOtherTabs: (id) => {
    const { tabs } = get();
    set({ tabs: tabs.filter((t) => t.id === id || t.dirty), activeTabId: id });
  },

  activateTab: (id) => set({ activeTabId: id }),
  pinTab: (id) => set({ tabs: get().tabs.map((t) => (t.id === id ? { ...t, pinned: true } : t)) }),
  renameTab: (id, title) => set({ tabs: get().tabs.map((t) => (t.id === id ? { ...t, title, pinned: true } : t)) }),
  updateTab: (id, patch) => set({ tabs: get().tabs.map((t) => (t.id === id ? { ...t, ...patch } : t)) }),
  setDirty: (id, dirty) => {
    const t = get().tabs.find((x) => x.id === id);
    if (t && !!t.dirty !== dirty) set({ tabs: get().tabs.map((x) => (x.id === id ? { ...x, dirty, pinned: x.pinned || dirty } : x)) });
  },
  setTabBusy: (id, busy) => {
    const cur = get().busyTabs;
    if (!!cur[id] === busy) return;
    if (busy) { set({ busyTabs: { ...cur, [id]: true } }); return; }
    const { [id]: _, ...rest } = cur;
    set({ busyTabs: rest });
  },
  registerTabActions: (id, actions) => set({ tabActions: { ...get().tabActions, [id]: actions } }),

  toast: (kind, text) => {
    const id = toastSeq++;
    set({ toasts: [...get().toasts, { id, kind, text }] });
    setTimeout(() => get().dismissToast(id), kind === "err" ? 8000 : 3500);
  },
  dismissToast: (id) => set({ toasts: get().toasts.filter((t) => t.id !== id) }),

  setSidebarSection: (s) => {
    set({ sidebarSection: s, sidebarVisible: true });
    if (s === "history") get().loadHistory();
    if (s === "queries") get().loadFavorites();
  },
  toggleSidebar: () => set({ sidebarVisible: !get().sidebarVisible }),
  setSidebarWidth: (w) => set({ sidebarWidth: Math.max(180, Math.min(600, w)) }),
  showConnectionForm: (config) => set({ connectionForm: { open: true, config } }),
  hideConnectionForm: () => set({ connectionForm: { open: false, config: null } }),
  setOpenDatabaseDialog: (v) => set({ openDatabaseDialog: v }),
  setSettingsDialog: (v) => set({ settingsDialog: v }),
  setCreateTableDialog: (v) => set({ createTableDialog: v }),
  setRenameDialog: (v) => set({ renameDialog: v }),
}));

export const useActiveTab = () => useApp((s) => s.tabs.find((t) => t.id === s.activeTabId) ?? null);
