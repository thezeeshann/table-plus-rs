// Run with: pnpm test   (node's built-in runner; no extra dependencies)
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  THEMES, derive, contrast, find, ids, mix, normalize, resolve, shade, setActive, activeIndex, isDefault,
  type Palette,
} from "./theme.ts";

const themed = THEMES.filter((t) => t.palette) as (typeof THEMES[number] & { palette: Palette })[];

test("ids are unique and default is first", () => {
  const all = THEMES.map((t) => t.id);
  assert.equal(new Set(all).size, all.length);
  assert.equal(THEMES[0].id, "default");
  assert.equal(THEMES[0].palette, null);
  assert.equal(THEMES.length, 5);
});

test("names resolve", () => {
  const cases: [string, string][] = [
    ["omarchy-one-dark-theme", "one-dark"], ["omarchy-one-dark-pro-theme", "one-dark-pro"],
    ["omarchy-arc-blueberry", "arc-blueberry"], ["omarchy-atelier-theme", "atelier"],
    ["One Dark Pro", "one-dark-pro"], ["ARC_BLUEBERRY", "arc-blueberry"], ["  atelier  ", "atelier"],
    ["default", "default"], ["jade-dark", "default"],
  ];
  for (const [input, id] of cases) assert.equal(THEMES[find(input)!]?.id, id, input);
  assert.equal(find("nope"), null);
  assert.equal(find(""), null);
  assert.equal(normalize(" Omarchy_One Dark_Theme "), "one-dark");
});

test("resolution order is flag > env > saved > default; blank env is ignored", () => {
  const idx = (r: ReturnType<typeof resolve>) => (r.ok ? THEMES[r.choice.index].id : "error");
  assert.equal(idx(resolve("atelier", "one-dark", "arc-blueberry")), "atelier");
  assert.equal(idx(resolve(null, "one-dark", "arc-blueberry")), "one-dark");
  assert.equal(idx(resolve(null, null, "arc-blueberry")), "arc-blueberry");
  assert.equal(idx(resolve(null, null, null)), "default");
  assert.equal(idx(resolve(null, "   ", "atelier")), "atelier");
  const r = resolve(null, "", "atelier");
  assert.ok(r.ok && r.choice.source === "saved");
});

test("unknown flag is an error listing ids; unknown env/saved fall through with a warning", () => {
  const bad = resolve("bogus");
  assert.ok(!bad.ok && bad.error.includes(ids()));
  const env = resolve(null, "bogus", "atelier");
  assert.ok(env.ok && THEMES[env.choice.index].id === "atelier" && env.choice.warning?.includes("bogus"));
  const saved = resolve(null, null, "bogus");
  assert.ok(saved.ok && saved.choice.index === 0 && saved.choice.warning?.includes("bogus"));
});

test("legacy saved value jade-dark resolves to default without a warning", () => {
  const r = resolve(null, null, "jade-dark");
  assert.ok(r.ok && r.choice.index === 0 && r.choice.source === "saved" && r.choice.warning === null);
});

test("palette contrast (spec section 8)", () => {
  for (const { id, palette: p } of themed) {
    assert.ok(contrast(p.text, p.bg) >= 6.0, `${id} text/bg ${contrast(p.text, p.bg)}`);
    for (const k of ["accent", "good", "warn", "bad", "info", "dim"] as const)
      assert.ok(contrast(p[k], p.bg) >= 3.0, `${id} ${k}/bg ${contrast(p[k], p.bg)}`);
    assert.ok(contrast(p.track, p.bg) >= 1.3, `${id} track/bg ${contrast(p.track, p.bg)}`);
    assert.ok(contrast(p.cursor, p.bg) >= 1.1, `${id} cursor/bg ${contrast(p.cursor, p.bg)}`);
    assert.ok(contrast(p.text, p.cursor) >= 4.5, `${id} text/cursor ${contrast(p.text, p.cursor)}`);
  }
});

test("good, warn and bad are pairwise distinct", () => {
  for (const { id, palette: p } of themed) {
    assert.equal(new Set([p.good, p.warn, p.bad]).size, 3, id);
  }
});

test("every themed palette has all ten roles as #rrggbb", () => {
  for (const { id, palette: p } of themed) {
    for (const [k, v] of Object.entries(p)) assert.match(v, /^#[0-9a-f]{6}$/, `${id}.${k}`);
  }
});

test("derived UI colors keep text readable", () => {
  const WHITE = "#ffffff";
  for (const { id, palette: p } of themed) {
    const v = derive(p);
    for (const bg of ["content-bg", "sidebar-bg", "app-bg", "control-bg", "menu-bg", "grid-even", "tab-bg", "tab-hover", "selection", "ed-current"])
      assert.ok(contrast(v.text, v[bg]) >= 4.5, `${id} text on ${bg}: ${contrast(v.text, v[bg]).toFixed(2)}`);
    for (const bg of ["tab-bg", "tab-hover"])
      assert.ok(contrast(v["tab-text"], v[bg]) >= 4.5, `${id} tab-text on ${bg}`);
    for (const fill of ["tab-selected", "row-inserted", "row-modified", "row-deleted", "status-ok", "status-warn", "status-err"])
      assert.ok(contrast(WHITE, v[fill]) >= 4.5, `${id} white on ${fill}: ${contrast(WHITE, v[fill]).toFixed(2)}`);
    assert.ok(contrast(WHITE, v["btn-primary-hover"]) >= 3.5, `${id} white on btn-primary-hover`);
    for (const fg of ["danger-text", "success-text", "key-color", "ed-keyword", "ed-string", "ed-number", "ed-comment", "ed-function", "ed-punct"])
      assert.ok(contrast(v[fg], v["content-bg"]) >= 3.0, `${id} ${fg} on content-bg: ${contrast(v[fg], v["content-bg"]).toFixed(2)}`);
  }
});

test("row states stay distinguishable from each other", () => {
  for (const { id, palette: p } of themed) {
    const v = derive(p);
    assert.equal(new Set([v["row-inserted"], v["row-modified"], v["row-deleted"]]).size, 3, id);
  }
});

test("derive emits the same keys for every theme", () => {
  const keys = Object.keys(derive(themed[0].palette)).sort();
  for (const t of themed) assert.deepEqual(Object.keys(derive(t.palette)).sort(), keys, t.id);
});

test("mix and shade", () => {
  assert.equal(mix("#000000", "#ffffff", 0), "#000000");
  assert.equal(mix("#000000", "#ffffff", 1), "#ffffff");
  assert.equal(mix("#000000", "#ffffff", 0.5), "#808080");
  assert.ok(contrast("#ffffff", shade("#ffff00", "#000000", "#ffffff", 0.9)) >= 4.5);
});

test("setActive applies and clears CSS variables; default writes none", () => {
  const props = new Map<string, string>();
  const root = {
    dataset: {} as Record<string, string>,
    style: { setProperty: (k: string, v: string) => props.set(k, v), removeProperty: (k: string) => props.delete(k) },
  } as unknown as HTMLElement;
  try {
    setActive(find("atelier")!, root);
    assert.equal(props.get("--content-bg"), "#22221b");
    assert.equal(props.get("--text"), "#dcd9d6");
    assert.equal((root.dataset as Record<string, string>).theme, "atelier");
    assert.ok(!isDefault());
    setActive(find("one-dark")!, root);
    assert.equal(props.get("--content-bg"), "#282c34");
    setActive(0, root);
    assert.equal(props.size, 0, "default must leave no overrides behind");
    assert.ok(isDefault());
    setActive(99, root);
    assert.equal(activeIndex(), THEMES.length - 1, "clamps to the last theme");
  } finally {
    setActive(0, root);
  }
});
