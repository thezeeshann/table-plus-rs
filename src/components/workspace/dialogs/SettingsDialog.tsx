import { useEffect, useRef, useState } from "react";
import { Modal, Field } from "@/components/common/Modal";
import { useApp } from "@/store/app";
import { ipc } from "@/lib/ipc";
import * as theme from "@/lib/theme";

const SWATCH_ROLES = ["accent", "good", "warn", "bad", "info", "dim", "track"] as const;

function Swatch({ t }: { t: theme.Theme }) {
  return <span className="swatch" aria-hidden>{SWATCH_ROLES.map((r) => <i key={r} style={{ background: t.swatch[r] }} />)}</span>;
}

/** Miniature of the real UI, painted with the live CSS variables so it follows the previewed theme. */
function Preview() {
  const bar = (label: string, pct: number, color: string) => (
    <div className="flex items-center gap-2"><span className="w-10 text-dim">{label}</span>
      <span className="flex-1 h-2 rounded-sm overflow-hidden" style={{ background: "var(--control-border)" }}><span className="block h-full" style={{ width: `${pct}%`, background: color }} /></span>
      <span className="w-9 text-right mono">{pct}%</span></div>
  );
  return (
    <div className="rounded-md border p-2.5 text-xs flex flex-col gap-1.5" style={{ background: "var(--content-bg)", borderColor: "var(--control-border)" }}>
      {bar("CPU", 34, "var(--success-text)")}{bar("Mem", 72, "var(--key-color)")}{bar("Disk", 91, "var(--danger-text)")}
      <div className="flex gap-3 mt-0.5"><span style={{ color: "var(--success-text)" }}>✓ ok</span><span style={{ color: "var(--key-color)" }}>⚠ warning</span><span style={{ color: "var(--danger-text)" }}>✗ critical</span><span className="text-dim">hint</span></div>
      <div className="mono rounded-sm px-1.5 py-0.5" style={{ background: "var(--selection)" }}><span style={{ color: "var(--ed-keyword)" }}>SELECT</span> <span style={{ color: "var(--ed-function)" }}>count</span>(*) <span style={{ color: "var(--ed-keyword)" }}>FROM</span> users <span style={{ color: "var(--ed-comment)" }}>-- selected</span></div>
    </div>
  );
}

function ThemePicker({ value, onPick }: { value: number; onPick: (i: number) => void }) {
  const n = theme.THEMES.length;
  const onKey = (e: React.KeyboardEvent) => {
    const k = e.key;
    if (k === "ArrowDown" || k === "j") onPick((value + 1) % n);
    else if (k === "ArrowUp" || k === "k") onPick((value + n - 1) % n);
    else if (k === "Home") onPick(0);
    else if (k === "End") onPick(n - 1);
    else return;
    e.preventDefault();
  };
  return (
    <div role="listbox" aria-label="Theme" tabIndex={0} onKeyDown={onKey} className="flex flex-col gap-0.5 outline-none focus-visible:ring-1 rounded-md" style={{ "--tw-ring-color": "var(--accent)" } as React.CSSProperties}>
      {theme.THEMES.map((t, i) => (
        <div key={t.id} role="option" aria-selected={i === value} className={`theme-row ${i === value ? "selected" : ""}`} onClick={() => onPick(i)}>
          <span className="w-3 text-center">{i === value ? "●" : ""}</span>
          <span className="flex-1 min-w-0">
            <span className="font-semibold">{t.name}</span> <span className="text-dim mono">{t.id}</span>
            <span className="block text-dim text-xs truncate-1">{t.about}</span>
          </span>
          <Swatch t={t} />
        </div>
      ))}
    </div>
  );
}

export function SettingsDialog() {
  const settings = useApp((s) => s.settings);
  const save = useApp((s) => s.saveSettings);
  const session = useApp((s) => s.session);
  const toast = useApp((s) => s.toast);
  const close = () => useApp.getState().setSettingsDialog(false);
  const [s, setS] = useState(settings);
  const themeSource = useApp((a) => a.themeSource);
  // The theme live when the dialog opened; closing without saving puts it back.
  const startIndex = useRef(theme.activeIndex());
  const committed = useRef(false);
  const [picked, setPicked] = useState<number | null>(null);
  const shown = picked ?? startIndex.current;
  const pick = (i: number) => { setPicked(i); theme.setActive(i); };
  useEffect(() => () => { if (!committed.current) theme.setActive(startIndex.current); }, []);
  return (
    <Modal title="Settings" onClose={close} width={520} footer={<>
      <button className="btn" onClick={close}>Cancel</button>
      <button className="btn btn-primary" onClick={async () => {
        await save(s);
        if (picked != null && picked !== startIndex.current) await useApp.getState().saveTheme(theme.THEMES[picked].id);
        committed.current = true;
        close();
      }}>Save</button>
    </>}>
      <div className="mb-3">
        <div className="text-dim mb-1.5">Theme</div>
        <ThemePicker value={shown} onPick={pick} />
        <div className="mt-2"><Preview /></div>
        <div className="text-dim text-xs mt-1.5">Active: {theme.THEMES[shown].name} ({picked != null ? "previewing" : theme.describe(themeSource)}). Only changes how table-plus-rs looks. Override for one run with <span className="mono">--theme NAME</span> or <span className="mono">TABLE_PLUS_RS_THEME</span>.</div>
      </div>
      <Field label="Rows per page"><input type="number" className="w-28 mono" value={s.defaultLimit} onChange={(e) => setS({ ...s, defaultLimit: Math.max(1, Number(e.target.value) || 300) })} /><span className="text-dim text-xs">default for new connections</span></Field>
      <Field label="Estimate count"><input type="number" className="w-32 mono" value={s.estimateCountThreshold} onChange={(e) => setS({ ...s, estimateCountThreshold: Number(e.target.value) || 100000 })} /><span className="text-dim text-xs">when table has more than N rows (unfiltered)</span></Field>
      <Field label="Editor font size"><input type="number" className="w-20 mono" value={s.editorFontSize} onChange={(e) => setS({ ...s, editorFontSize: Math.min(24, Math.max(9, Number(e.target.value) || 13)) })} /></Field>
      <Field label="Editor"><label className="flex items-center gap-1.5"><input type="checkbox" checked={s.autoCloseBrackets} onChange={(e) => setS({ ...s, autoCloseBrackets: e.target.checked })} />Automatically insert closing braces and quotes</label></Field>
      <Field label="History">
        <button className="btn" disabled={!session} onClick={async () => {
          if (!session || !confirm("Clear query history for this connection?")) return;
          await ipc.clearHistory(session.connectionId);
          useApp.getState().loadHistory();
          toast("ok", "History cleared");
        }}>Clear history for this connection</button>
      </Field>
      <div className="text-dim text-xs mt-3">Connections live in the app data folder; passwords in connections.json (owner-readable only; no Keychain).</div>
    </Modal>
  );
}
