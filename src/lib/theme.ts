/**
 * Theme system. Port of the sweepr `theme-build.md` spec to table-plus-rs's web UI.
 *
 * - `default` is index 0: the original Jade Dark look, expressed by the CSS in
 *   styles/index.css. It writes no variable overrides at all, so it cannot drift.
 * - The other four are Omarchy-derived palettes (spec section 2). A theme is just
 *   ten role colors; `derive()` expands them into every CSS variable the UI reads.
 * - The active theme is a global (like the spec's atomic): applying one rewrites
 *   the variables on <html>, and everything, CodeMirror included, restyles live.
 *
 * Keep this file free of imports so `node --test` can load it directly.
 */

export interface Palette {
  accent: string; good: string; warn: string; bad: string; info: string;
  dim: string; track: string; cursor: string; text: string; bg: string;
}

export interface Theme {
  id: string;
  name: string;
  about: string;
  /** null = the stylesheet's own values (the original look). */
  palette: Palette | null;
  /** Colors for the picker swatch; for themed entries this is derived from the palette. */
  swatch: Pick<Palette, "accent" | "good" | "warn" | "bad" | "info" | "dim" | "track">;
}

const themed = (
  accent: string, good: string, warn: string, bad: string, info: string,
  dim: string, track: string, cursor: string, text: string, bg: string,
): Palette => ({ accent, good, warn, bad, info, dim, track, cursor, text, bg });

const OMARCHY = {
  "one-dark": themed("#61afef", "#a9d98a", "#f0d399", "#ff7b86", "#66d9e8", "#7f848e", "#3e4451", "#333842", "#abb2bf", "#282c34"),
  "one-dark-pro": themed("#61afef", "#98c379", "#e5c07b", "#e06c75", "#56b6c2", "#7a828e", "#4b5263", "#3e4451", "#abb2bf", "#282c34"),
  "arc-blueberry": themed("#8eb0e6", "#3cec85", "#eacd61", "#e35535", "#69c3ff", "#6f7594", "#424761", "#313b62", "#bcc1dc", "#111422"),
  atelier: themed("#36a166", "#a7c957", "#c2b53c", "#ba6236", "#5f9182", "#7d7c6b", "#3b3a2e", "#302f27", "#dcd9d6", "#22221b"),
};

const swatchOf = (p: Palette) => ({ accent: p.accent, good: p.good, warn: p.warn, bad: p.bad, info: p.info, dim: p.dim, track: p.track });

export const THEMES: readonly Theme[] = [
  {
    id: "default",
    name: "Default",
    about: "The original table-plus-rs look: Jade Dark, ported from TablePlus",
    palette: null,
    swatch: { accent: "#529bd8", good: "#8bd36a", warn: "#f9b746", bad: "#ff7b7b", info: "#b57aff", dim: "#7e7e7e", track: "#1e4550" },
  },
  { id: "one-dark", name: "One Dark", about: "Atom's classic dark theme, with the brighter colors for status and values", palette: OMARCHY["one-dark"], swatch: swatchOf(OMARCHY["one-dark"]) },
  { id: "one-dark-pro", name: "One Dark Pro", about: "The VS Code take on One Dark: same base, standard colors, softer surfaces", palette: OMARCHY["one-dark-pro"], swatch: swatchOf(OMARCHY["one-dark-pro"]) },
  { id: "arc-blueberry", name: "Arc Blueberry", about: "Deep blue-black with soft periwinkle accents and neon green and cyan", palette: OMARCHY["arc-blueberry"], swatch: swatchOf(OMARCHY["arc-blueberry"]) },
  { id: "atelier", name: "Atelier", about: "Warm olive-black with earthy greens, ochre and clay", palette: OMARCHY.atelier, swatch: swatchOf(OMARCHY.atelier) },
];

// ---------------------------------------------------------------- color math

type Rgb = [number, number, number];

export function parseHex(h: string): Rgb {
  const s = h.replace("#", "");
  return [parseInt(s.slice(0, 2), 16), parseInt(s.slice(2, 4), 16), parseInt(s.slice(4, 6), 16)];
}

export function toHex([r, g, b]: Rgb): string {
  const c = (n: number) => Math.max(0, Math.min(255, Math.round(n))).toString(16).padStart(2, "0");
  return `#${c(r)}${c(g)}${c(b)}`;
}

/** Linear blend: t=0 is `a`, t=1 is `b`. */
export function mix(a: string, b: string, t: number): string {
  const x = parseHex(a), y = parseHex(b);
  return toHex([x[0] + (y[0] - x[0]) * t, x[1] + (y[1] - x[1]) * t, x[2] + (y[2] - x[2]) * t]);
}

export function luminance(h: string): number {
  const [r, g, b] = parseHex(h).map((v) => {
    const c = v / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** WCAG contrast ratio, (L1+.05)/(L2+.05) with L1 the lighter. */
export function contrast(a: string, b: string): number {
  const la = luminance(a), lb = luminance(b);
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}

const alpha = (h: string, a: number) => h + Math.round(a * 255).toString(16).padStart(2, "0");

/**
 * `color` blended into `base`, as strongly as `target` allows while `fg` stays
 * readable on the result. Used for filled areas (selected tab, edited rows, toasts)
 * so a bright palette color never ends up behind text it cannot carry.
 */
export function shade(color: string, base: string, fg: string, target: number, min = 4.5): string {
  let t = target;
  while (t > 0 && contrast(fg, mix(base, color, t)) < min) t -= 0.03;
  return mix(base, color, Math.max(t, 0)); // 0 = `base` itself when `color` can never carry `fg`
}

// ------------------------------------------------------------------ deriving

const WHITE = "#ffffff";
const BLACK = "#000000";

/** Expand ten role colors into every themeable CSS variable (without the leading `--`). */
export function derive(p: Palette): Record<string, string> {
  const appBg = mix(p.bg, BLACK, 0.28);
  const sidebar = mix(p.bg, BLACK, 0.14);
  const control = mix(p.bg, p.cursor, 0.55);
  const tabText = p.text; // no headroom to dim it: One Dark Pro text/cursor is barely 4.5:1
  // Hover lifts toward the accent; when the tab color has no headroom left it sinks toward the background.
  const lifted = shade(p.accent, p.cursor, tabText, 0.2);
  const tabHover = lifted !== p.cursor ? lifted : mix(p.cursor, p.bg, 0.45);
  const tabSelected = shade(p.accent, p.cursor, WHITE, 0.4);
  const selection = shade(p.accent, p.bg, p.text, 0.32);
  const gridEven = mix(p.bg, p.cursor, 0.5);
  const current = mix(p.bg, p.cursor, 0.6);
  const ok = shade(p.good, p.bg, WHITE, 0.55);
  const warn = shade(p.warn, p.bg, WHITE, 0.55);
  const err = shade(p.bad, p.bg, WHITE, 0.55);
  const menu = p.cursor;
  return {
    "app-bg": appBg,
    "content-bg": p.bg,
    "sidebar-bg": sidebar,
    "control-bg": control,
    "control-border": p.track,
    "separator": mix(p.bg, BLACK, 0.5),
    "tab-bg": p.cursor,
    "tab-hover": tabHover,
    "tab-selected": tabSelected,
    "tab-text": tabText,
    "text": p.text,
    "text-dim": p.dim,
    "accent": p.accent,
    "accent-strong": tabSelected,
    "selection": selection,
    "selection-border": p.bad,
    "grid-even": gridEven,
    "grid-header": p.bg,
    "row-inserted": ok,
    "row-modified": warn,
    "row-deleted": err,
    "status-ok": ok,
    "status-warn": warn,
    "status-err": err,
    "status-border": p.track,
    "caret": p.text,
    "menu-bg": menu,
    "scroll-thumb": p.track,
    "scroll-thumb-hover": mix(p.track, p.text, 0.2),
    "btn-primary-hover": mix(tabSelected, WHITE, 0.12),
    "danger-text": p.bad,
    "success-text": p.good,
    "key-color": p.warn,
    "ed-keyword": p.accent,
    "ed-string": p.good,
    "ed-number": p.warn,
    "ed-comment": p.dim,
    "ed-function": p.info,
    "ed-ident": p.text,
    "ed-type": p.info,
    "ed-punct": mix(p.dim, p.text, 0.35),
    "ed-current": current,
    "ed-selection": selection,
    "ed-bracket-match": alpha(p.bad, 0.5),
    "ed-search-match": alpha(p.warn, 0.5),
    "ed-search-match-current": warn,
    "ed-selection-match": alpha(selection, 0.5),
    "ed-current-statement": alpha(current, 0.5),
    "ed-error-line": alpha(p.bad, 0.38),
  };
}

// ------------------------------------------------------ names and selection

/** Names the stored settings used before themes existed. */
const ALIASES: Record<string, string> = { "jade-dark": "default" };

/** Trim, lowercase, '_' and ' ' to '-', strip an "omarchy-" prefix and a "-theme" suffix. */
export function normalize(name: string): string {
  let n = name.trim().toLowerCase().replace(/[_ ]/g, "-");
  if (n.startsWith("omarchy-")) n = n.slice("omarchy-".length);
  if (n.endsWith("-theme")) n = n.slice(0, -"-theme".length);
  return ALIASES[n] ?? n;
}

export function find(name: string): number | null {
  const id = normalize(name);
  const i = THEMES.findIndex((t) => t.id === id);
  return i < 0 ? null : i;
}

export const ids = (): string => THEMES.map((t) => t.id).join(", ");

export type Source = "flag" | "env" | "saved" | "default";

export function describe(source: Source): string {
  return { flag: "from --theme", env: "from $TABLE_PLUS_RS_THEME", saved: "saved in settings", default: "the default" }[source];
}

export interface Choice { index: number; source: Source; warning: string | null }

/**
 * Precedence: --theme, then $TABLE_PLUS_RS_THEME, then the saved setting, then default.
 * A bad flag is an error (the caller reports it); a bad env var or saved name
 * falls through with a warning, so a typo can never lock the user out.
 */
export function resolve(flag?: string | null, env?: string | null, saved?: string | null): { ok: true; choice: Choice } | { ok: false; error: string } {
  if (flag != null && flag.trim() !== "") {
    const i = find(flag);
    if (i == null) return { ok: false, error: `unknown theme \`${flag}\`; choose one of: ${ids()}` };
    return { ok: true, choice: { index: i, source: "flag", warning: null } };
  }
  let warning: string | null = null;
  if (env != null && env.trim() !== "") {
    const i = find(env);
    if (i != null) return { ok: true, choice: { index: i, source: "env", warning: null } };
    warning = `unknown theme \`${env}\` in $TABLE_PLUS_RS_THEME; ignoring it`;
  }
  if (saved != null && saved.trim() !== "") {
    const i = find(saved);
    if (i != null) return { ok: true, choice: { index: i, source: "saved", warning } };
    warning = `${warning ? warning + "; " : ""}unknown saved theme \`${saved}\`; using the default`;
  }
  return { ok: true, choice: { index: 0, source: "default", warning } };
}

// -------------------------------------------------------------- active theme

let active = 0;
let appliedKeys: string[] = [];

export const activeIndex = (): number => active;
export const activeTheme = (): Theme => THEMES[active];
export const isDefault = (): boolean => active === 0;

/** Make theme `i` the live one. Clamps like the spec's `set_active`. */
export function setActive(i: number, root: HTMLElement = document.documentElement): void {
  active = Math.max(0, Math.min(THEMES.length - 1, i));
  for (const k of appliedKeys) root.style.removeProperty(`--${k}`);
  appliedKeys = [];
  const p = THEMES[active].palette;
  root.dataset.theme = THEMES[active].id;
  if (!p) return; // default: the stylesheet is the source of truth
  const vars = derive(p);
  for (const [k, v] of Object.entries(vars)) root.style.setProperty(`--${k}`, v);
  appliedKeys = Object.keys(vars);
}

// Boot cache: the saved theme lives in the backend settings, which load async.
// Mirroring the id in localStorage lets the first paint use it without a flash.
const CACHE_KEY = "table-plus-rs.theme";
export function readCached(): string | null {
  try { return localStorage.getItem(CACHE_KEY); } catch { return null; }
}
export function writeCached(id: string): void {
  try { localStorage.setItem(CACHE_KEY, id); } catch { /* storage unavailable */ }
}
