import { useEffect, useRef } from "react";
import { EditorState, Compartment, type Extension } from "@codemirror/state";
import { EditorView, keymap, lineNumbers, highlightActiveLine, highlightActiveLineGutter, drawSelection, highlightSpecialChars, rectangularSelection, crosshairCursor } from "@codemirror/view";
import { defaultKeymap, history, historyKeymap, indentWithTab, toggleComment } from "@codemirror/commands";
import { sql, PostgreSQL, type SQLConfig } from "@codemirror/lang-sql";
import { autocompletion, closeBrackets, closeBracketsKeymap, completionKeymap } from "@codemirror/autocomplete";
import { bracketMatching, indentOnInput, syntaxHighlighting, HighlightStyle, foldGutter } from "@codemirror/language";
import { searchKeymap, highlightSelectionMatches, search } from "@codemirror/search";
import { tags as t } from "@lezer/highlight";
import { statementAt } from "@/lib/sql";

/** Jade Dark editor colours (from TablePlus' theme JSON). */
const theme = EditorView.theme({
  "&": { backgroundColor: "var(--content-bg)", color: "var(--ed-ident)", height: "100%" },
  ".cm-content": { caretColor: "#fff", fontFamily: "var(--font-mono)" },
  ".cm-cursor": { borderLeftColor: "#fff" },
  "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection": { backgroundColor: "var(--ed-selection) !important" },
  ".cm-activeLine": { backgroundColor: "var(--ed-current)" },
  ".cm-activeLineGutter": { backgroundColor: "var(--ed-current)" },
  ".cm-gutters": { backgroundColor: "var(--content-bg)", color: "var(--text-dim)", border: "none" },
  ".cm-matchingBracket": { backgroundColor: "#8f2f3180", outline: "none" },
  ".cm-tooltip": { backgroundColor: "#0f2f3a", border: "1px solid var(--control-border)", color: "var(--text)" },
  ".cm-tooltip-autocomplete ul li[aria-selected]": { backgroundColor: "var(--tab-selected)", color: "#fff" },
  ".cm-panels": { backgroundColor: "var(--app-bg)", color: "var(--text)" },
  ".cm-panels input": { background: "var(--control-bg)", color: "var(--text)", border: "1px solid var(--control-border)" },
  ".cm-searchMatch": { backgroundColor: "#8b682680" },
  ".cm-searchMatch.cm-searchMatch-selected": { backgroundColor: "#8b6826" },
  ".cm-selectionMatch": { backgroundColor: "#004f6c80" },
  ".cm-current-statement": { backgroundColor: "#00374580" },
  ".cm-error-line": { backgroundColor: "#8f2f3160" },
}, { dark: true });

const highlight = HighlightStyle.define([
  { tag: t.keyword, color: "var(--ed-keyword)", fontWeight: "600" },
  { tag: [t.operator, t.operatorKeyword], color: "var(--ed-keyword)" },
  { tag: [t.string, t.special(t.string)], color: "var(--ed-string)" },
  { tag: [t.number, t.integer, t.float, t.bool, t.null], color: "var(--ed-number)" },
  { tag: [t.comment, t.lineComment, t.blockComment], color: "var(--ed-comment)", fontStyle: "italic" },
  { tag: [t.function(t.variableName), t.function(t.propertyName), t.standard(t.name)], color: "var(--ed-function)" },
  { tag: [t.typeName, t.className], color: "#7cb342" },
  { tag: [t.variableName, t.propertyName, t.name], color: "var(--ed-ident)" },
  { tag: t.punctuation, color: "#9aa5a8" },
]);

export interface SqlEditorProps {
  value: string;
  onChange?: (v: string) => void;
  readOnly?: boolean;
  schema?: SQLConfig["schema"];
  defaultSchema?: string;
  onRunCurrent?: () => void;
  onRunAll?: () => void;
  fontSize?: number;
  autoCloseBrackets?: boolean;
  /** Receives the view so the parent can query selection / cursor. */
  viewRef?: React.MutableRefObject<EditorView | null>;
  highlightCurrent?: boolean;
}

export function SqlEditor({ value, onChange, readOnly, schema, defaultSchema, onRunCurrent, onRunAll, fontSize = 13, autoCloseBrackets = true, viewRef, highlightCurrent }: SqlEditorProps) {
  const host = useRef<HTMLDivElement>(null);
  const view = useRef<EditorView | null>(null);
  const langComp = useRef(new Compartment());
  const fontComp = useRef(new Compartment());
  const bracketsComp = useRef(new Compartment());
  const handlers = useRef({ onChange, onRunCurrent, onRunAll });
  handlers.current = { onChange, onRunCurrent, onRunAll };

  useEffect(() => {
    if (!host.current) return;
    const runKeys = keymap.of([
      { key: "Mod-Enter", run: () => { handlers.current.onRunCurrent?.(); return true; } },
      { key: "Mod-Shift-Enter", run: () => { handlers.current.onRunAll?.(); return true; } },
      { key: "Mod-/", run: toggleComment },
    ]);
    const extensions: Extension[] = [
      runKeys,
      lineNumbers(), foldGutter(), highlightActiveLine(), highlightActiveLineGutter(), highlightSpecialChars(), drawSelection(),
      rectangularSelection(), crosshairCursor(), history(), bracketMatching(), indentOnInput(), search({ top: true }), highlightSelectionMatches(),
      bracketsComp.current.of(autoCloseBrackets ? closeBrackets() : []),
      autocompletion({ activateOnTyping: true }),
      keymap.of([...closeBracketsKeymap, ...defaultKeymap, ...historyKeymap, ...searchKeymap, ...completionKeymap, indentWithTab]),
      langComp.current.of(sql({ dialect: PostgreSQL, schema, defaultSchema, upperCaseKeywords: true })),
      syntaxHighlighting(highlight), theme,
      fontComp.current.of(EditorView.theme({ "&": { fontSize: `${fontSize}px` } })),
      EditorView.lineWrapping,
      EditorState.readOnly.of(!!readOnly),
      EditorView.updateListener.of((u) => { if (u.docChanged) handlers.current.onChange?.(u.state.doc.toString()); }),
    ];
    const v = new EditorView({ state: EditorState.create({ doc: value, extensions }), parent: host.current });
    view.current = v;
    if (viewRef) viewRef.current = v;
    return () => { v.destroy(); view.current = null; if (viewRef) viewRef.current = null; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    const v = view.current; if (!v) return;
    if (v.state.doc.toString() !== value) v.dispatch({ changes: { from: 0, to: v.state.doc.length, insert: value } });
  }, [value]);

  useEffect(() => {
    view.current?.dispatch({ effects: langComp.current.reconfigure(sql({ dialect: PostgreSQL, schema, defaultSchema, upperCaseKeywords: true })) });
  }, [schema, defaultSchema]);
  useEffect(() => { view.current?.dispatch({ effects: fontComp.current.reconfigure(EditorView.theme({ "&": { fontSize: `${fontSize}px` } })) }); }, [fontSize]);
  useEffect(() => { view.current?.dispatch({ effects: bracketsComp.current.reconfigure(autoCloseBrackets ? closeBrackets() : []) }); }, [autoCloseBrackets]);

  return <div ref={host} className="h-full w-full" data-highlight={highlightCurrent ? "1" : undefined} />;
}

/** Text to execute for "Run Current Query": selection if any, else statement at cursor. */
export function currentStatement(view: EditorView | null): { sql: string; from: number; to: number } | null {
  if (!view) return null;
  const sel = view.state.selection.main;
  if (!sel.empty) return { sql: view.state.sliceDoc(sel.from, sel.to), from: sel.from, to: sel.to };
  const span = statementAt(view.state.doc.toString(), sel.head);
  return span ? { sql: span.text, from: span.from, to: span.to } : null;
}
