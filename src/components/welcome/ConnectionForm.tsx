import { useState } from "react";
import { open as openFile } from "@tauri-apps/plugin-dialog";
import { useApp } from "@/store/app";
import { Modal, Field } from "@/components/common/Modal";
import { ipc, errorMessage } from "@/lib/ipc";
import type { ConnectionConfig } from "@/lib/types";
import { CONNECTION_COLORS } from "./WelcomeScreen";

const empty = (): ConnectionConfig => ({
  id: "", name: "", groupId: null, color: "1", host: "127.0.0.1", port: 5432, user: "postgres", password: "",
  database: "postgres", sslMode: "prefer", sslRootCert: null, startupCommands: "", limitRowsReturned: 300,
  showSystemSchemas: false, keepAlive: true, environment: "", safeMode: 0,
});

/** Parses postgres://user:pass@host:port/db?sslmode=x into form fields. */
function parseUrl(url: string): Partial<ConnectionConfig> | null {
  try {
    const u = new URL(url.replace(/^postgres(ql)?(\+ssh)?:/, "http:"));
    const out: Partial<ConnectionConfig> = {
      host: u.hostname || "127.0.0.1",
      port: u.port ? Number(u.port) : 5432,
      user: decodeURIComponent(u.username),
      password: decodeURIComponent(u.password),
      database: decodeURIComponent(u.pathname.replace(/^\//, "")) || "postgres",
    };
    const ssl = u.searchParams.get("sslmode");
    if (ssl) out.sslMode = ssl as ConnectionConfig["sslMode"];
    return out;
  } catch { return null; }
}

export function ConnectionForm() {
  const form = useApp((s) => s.connectionForm);
  const groups = useApp((s) => s.groups);
  const hide = useApp((s) => s.hideConnectionForm);
  const loadConnections = useApp((s) => s.loadConnections);
  const connect = useApp((s) => s.connect);
  const toast = useApp((s) => s.toast);
  const [cfg, setCfg] = useState<ConnectionConfig>(() => (form.config ? { ...form.config, password: "" } : empty()));
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ ok: boolean; text: string } | null>(null);
  const [saving, setSaving] = useState(false);
  const [showImport, setShowImport] = useState(false);
  const [importUrl, setImportUrl] = useState("");

  const patch = (p: Partial<ConnectionConfig>) => setCfg((c) => ({ ...c, ...p }));
  const valid = cfg.name.trim() && cfg.host.trim() && cfg.user.trim() && cfg.port > 0;

  const save = async (): Promise<ConnectionConfig | null> => {
    setSaving(true);
    try {
      const saved = await ipc.saveConnection({ ...cfg, password: cfg.password || undefined });
      await loadConnections();
      return saved;
    } catch (e) {
      toast("err", errorMessage(e));
      return null;
    } finally { setSaving(false); }
  };

  const test = async () => {
    setTesting(true); setTestResult(null);
    try {
      const v = await ipc.testConnection(cfg);
      setTestResult({ ok: true, text: `Connection is OK · ${v}` });
    } catch (e) {
      setTestResult({ ok: false, text: errorMessage(e) });
    } finally { setTesting(false); }
  };

  return (
    <Modal
      title={
        <div className="flex items-center justify-between">
          <span>{form.config ? "Edit connection" : "Create connection"} · PostgreSQL</span>
          <button className="btn btn-ghost text-xs" onClick={() => setShowImport((v) => !v)}>Import from URL</button>
        </div>
      }
      onClose={hide}
      width={620}
      footer={
        <>
          <div className="flex-1 flex items-center gap-2 text-xs selectable">
            {testResult && <span className={testResult.ok ? "text-[#8bd36a]" : "text-[#ff7b7b]"}>{testResult.text}</span>}
          </div>
          <button className="btn" onClick={test} disabled={!valid || testing}>{testing ? "Testing..." : "Test"}</button>
          <button className="btn" onClick={async () => { if (await save()) hide(); }} disabled={!valid || saving}>Save</button>
          <button className="btn btn-primary" onClick={async () => { const s = await save(); if (s) { hide(); connect(s.id).catch(() => {}); } }} disabled={!valid || saving}>Connect</button>
        </>
      }
    >
      {showImport && (
        <div className="flex gap-2 mb-3">
          <input type="text" className="flex-1 mono" placeholder="postgresql://user:password@host:5432/database?sslmode=require" value={importUrl} onChange={(e) => setImportUrl(e.target.value)} />
          <button className="btn" onClick={() => { const p = parseUrl(importUrl); if (p) { patch(p); setShowImport(false); } else toast("err", "Invalid connection URL"); }}>Apply</button>
        </div>
      )}
      <Field label="Name">
        <input type="text" className="flex-1" autoFocus value={cfg.name} onChange={(e) => patch({ name: e.target.value })} placeholder="dev-db" />
        <span className="flex gap-1">
          {Object.entries(CONNECTION_COLORS).map(([k, v]) => (
            <button key={k} className="w-5 h-5 rounded-full border-2" style={{ background: v, borderColor: cfg.color === k ? "#fff" : "transparent" }} onClick={() => patch({ color: k })} title={`Color ${k}`} />
          ))}
        </span>
      </Field>
      <Field label="Tag">
        <select className="flex-1" value={cfg.environment} onChange={(e) => patch({ environment: e.target.value })}>
          <option value="">None</option><option value="Local">Local</option><option value="Testing">Testing</option><option value="Staging">Staging</option><option value="Production">Production</option>
        </select>
        <select className="flex-1" value={cfg.groupId ?? ""} onChange={(e) => patch({ groupId: e.target.value || null })}>
          <option value="">No group</option>
          {groups.map((g) => <option key={g.id} value={g.id}>{g.name}</option>)}
        </select>
      </Field>
      <Field label="Host">
        <input type="text" className="flex-1 mono" value={cfg.host} onChange={(e) => patch({ host: e.target.value })} />
        <span className="text-dim">Port</span>
        <input type="number" className="w-20 mono" value={cfg.port} onChange={(e) => patch({ port: Number(e.target.value) || 5432 })} />
      </Field>
      <Field label="User">
        <input type="text" className="flex-1 mono" value={cfg.user} onChange={(e) => patch({ user: e.target.value })} />
      </Field>
      <Field label="Password">
        <input type="password" className="flex-1 mono" value={cfg.password ?? ""} onChange={(e) => patch({ password: e.target.value })} placeholder={form.config ? "(unchanged – stored in Keychain)" : ""} />
      </Field>
      <Field label="Database">
        <input type="text" className="flex-1 mono" value={cfg.database} onChange={(e) => patch({ database: e.target.value })} />
      </Field>
      <Field label="SSL mode">
        <select className="flex-1" value={cfg.sslMode} onChange={(e) => patch({ sslMode: e.target.value as ConnectionConfig["sslMode"] })}>
          <option value="disable">Disable</option>
          <option value="prefer">Prefer (encrypt if available)</option>
          <option value="require">Require (no verification)</option>
          <option value="verify-ca">Verify CA</option>
          <option value="verify-full">Verify Full</option>
        </select>
        {(cfg.sslMode === "verify-ca" || cfg.sslMode === "verify-full") && (
          <button className="btn" onClick={async () => {
            const p = await openFile({ multiple: false, title: "Choose CA certificate" });
            if (typeof p === "string") patch({ sslRootCert: p });
          }}>{cfg.sslRootCert ? "CA ✓" : "Choose CA..."}</button>
        )}
      </Field>
      <details className="mt-2">
        <summary className="text-dim cursor-default mb-2">Advanced</summary>
        <Field label="Row limit">
          <input type="number" className="w-28 mono" value={cfg.limitRowsReturned} onChange={(e) => patch({ limitRowsReturned: Number(e.target.value) || 300 })} />
          <span className="text-dim text-xs">rows per page in table view</span>
        </Field>
        <Field label="Options">
          <label className="flex items-center gap-1.5"><input type="checkbox" checked={cfg.keepAlive} onChange={(e) => patch({ keepAlive: e.target.checked })} />Keep connection alive</label>
          <label className="flex items-center gap-1.5"><input type="checkbox" checked={cfg.showSystemSchemas} onChange={(e) => patch({ showSystemSchemas: e.target.checked })} />Show system schemas</label>
        </Field>
        <Field label="Startup SQL">
          <textarea className="flex-1 mono" rows={3} value={cfg.startupCommands} onChange={(e) => patch({ startupCommands: e.target.value })} placeholder="SET search_path TO public; SET statement_timeout = '30s';" />
        </Field>
      </details>
    </Modal>
  );
}
