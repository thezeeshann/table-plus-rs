// Statement splitting that matches src-tauri/src/pg/sqlgen.rs so "Run Current
// Query" picks the same statement the backend would.

export interface StatementSpan { text: string; from: number; to: number }

export function splitStatements(sql: string): StatementSpan[] {
  const out: StatementSpan[] = [];
  const n = sql.length;
  let i = 0;
  let start = 0;
  const push = (end: number, next: number) => {
    const raw = sql.slice(start, end);
    const text = raw.trim();
    if (text) {
      const lead = raw.length - raw.trimStart().length;
      out.push({ text, from: start + lead, to: next });
    }
    start = next;
  };
  while (i < n) {
    const c = sql[i];
    if (c === "-" && sql[i + 1] === "-") {
      while (i < n && sql[i] !== "\n") i++;
      continue;
    }
    if (c === "/" && sql[i + 1] === "*") {
      let depth = 1;
      i += 2;
      while (i < n && depth > 0) {
        if (sql[i] === "/" && sql[i + 1] === "*") { depth++; i += 2; }
        else if (sql[i] === "*" && sql[i + 1] === "/") { depth--; i += 2; }
        else i++;
      }
      continue;
    }
    if (c === "'") {
      const prev = sql[i - 1];
      const escaped = (prev === "E" || prev === "e") && !(i >= 2 && /[A-Za-z0-9_]/.test(sql[i - 2]));
      i++;
      while (i < n) {
        if (escaped && sql[i] === "\\") { i += 2; continue; }
        if (sql[i] === "'") {
          if (sql[i + 1] === "'") { i += 2; continue; }
          i++;
          break;
        }
        i++;
      }
      continue;
    }
    if (c === '"') {
      i++;
      while (i < n) {
        if (sql[i] === '"') {
          if (sql[i + 1] === '"') { i += 2; continue; }
          i++;
          break;
        }
        i++;
      }
      continue;
    }
    if (c === "$") {
      let j = i + 1;
      while (j < n && /[A-Za-z0-9_]/.test(sql[j])) j++;
      if (sql[j] === "$" && (j === i + 1 || !/[0-9]/.test(sql[i + 1]))) {
        const tag = sql.slice(i, j + 1);
        const pos = sql.indexOf(tag, j + 1);
        i = pos < 0 ? n : pos + tag.length;
        continue;
      }
    }
    if (c === ";") {
      push(i, i + 1);
      i++;
      continue;
    }
    i++;
  }
  push(n, n);
  return out;
}

/** Statement containing the cursor (or the nearest previous one). */
export function statementAt(sql: string, pos: number): StatementSpan | null {
  const spans = splitStatements(sql);
  if (!spans.length) return null;
  for (const s of spans) {
    if (pos >= s.from && pos <= s.to) return s;
  }
  // cursor in whitespace between statements: prefer the previous statement
  let best = spans[0];
  for (const s of spans) if (s.from <= pos) best = s;
  return best;
}

export function leadingKeyword(sql: string): string {
  let s = sql.trimStart();
  for (;;) {
    if (s.startsWith("--")) s = s.slice(s.indexOf("\n") + 1 || s.length).trimStart();
    else if (s.startsWith("/*")) { const e = s.indexOf("*/"); s = (e < 0 ? "" : s.slice(e + 2)).trimStart(); }
    else break;
  }
  return (s.match(/^[A-Za-z_]+/)?.[0] ?? "").toUpperCase();
}

export function quoteIdent(s: string): string {
  return `"${s.replace(/"/g, '""')}"`;
}

export function quoteLiteral(s: string): string {
  return `'${s.replace(/'/g, "''")}'`;
}

export function isMutation(sql: string): boolean {
  const kw = leadingKeyword(sql);
  return !["SELECT", "WITH", "SHOW", "EXPLAIN", "TABLE", "VALUES", "SET", "BEGIN", "COMMIT", "ROLLBACK"].includes(kw);
}
