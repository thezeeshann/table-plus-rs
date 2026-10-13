import { Plus, X } from "lucide-react";
import type { FilterOp, FilterSpec } from "@/lib/types";

export const FILTER_OPS: { op: FilterOp; label: string; args: 0 | 1 | 2 }[] = [
  { op: "eq", label: "=", args: 1 }, { op: "neq", label: "!=", args: 1 },
  { op: "gt", label: ">", args: 1 }, { op: "gte", label: ">=", args: 1 },
  { op: "lt", label: "<", args: 1 }, { op: "lte", label: "<=", args: 1 },
  { op: "contains", label: "contains", args: 1 }, { op: "not_contains", label: "not contains", args: 1 },
  { op: "starts_with", label: "has prefix", args: 1 }, { op: "ends_with", label: "has suffix", args: 1 },
  { op: "like", label: "LIKE", args: 1 }, { op: "ilike", label: "ILIKE", args: 1 }, { op: "not_like", label: "NOT LIKE", args: 1 },
  { op: "in", label: "IN (a,b,c)", args: 1 }, { op: "not_in", label: "NOT IN", args: 1 },
  { op: "between", label: "BETWEEN", args: 2 },
  { op: "is_null", label: "IS NULL", args: 0 }, { op: "is_not_null", label: "IS NOT NULL", args: 0 },
  { op: "is_true", label: "IS TRUE", args: 0 }, { op: "is_false", label: "IS FALSE", args: 0 },
  { op: "raw", label: "raw SQL", args: 1 },
];

export function FilterBar({ columns, filters, onChange, onApply }: {
  columns: string[]; filters: FilterSpec[]; onChange: (f: FilterSpec[]) => void; onApply: () => void;
}) {
  const upd = (i: number, p: Partial<FilterSpec>) => onChange(filters.map((f, j) => (j === i ? { ...f, ...p } : f)));
  const add = () => onChange([...filters, { column: columns[0] ?? "", op: "eq", value: "", enabled: true }]);
  return (
    <div className="border-b border-border bg-app px-2 py-1 flex flex-col gap-1 shrink-0">
      {filters.map((f, i) => {
        const def = FILTER_OPS.find((o) => o.op === f.op)!;
        return (
          <div key={i} className="flex items-center gap-1">
            <input type="checkbox" checked={f.enabled} onChange={(e) => upd(i, { enabled: e.target.checked })} />
            {f.op === "raw" ? (
              <input type="text" className="flex-1 mono !py-[3px]" placeholder="WHERE clause, e.g. created_at > now() - interval '1 day'" value={f.value ?? ""} onChange={(e) => upd(i, { value: e.target.value })} onKeyDown={(e) => e.key === "Enter" && onApply()} />
            ) : (
              <>
                <select className="!py-[3px] mono w-44" value={f.column} onChange={(e) => upd(i, { column: e.target.value })}>
                  {columns.map((c) => <option key={c} value={c}>{c}</option>)}
                </select>
                <select className="!py-[3px] w-36" value={f.op} onChange={(e) => upd(i, { op: e.target.value as FilterOp })}>
                  {FILTER_OPS.map((o) => <option key={o.op} value={o.op}>{o.label}</option>)}
                </select>
                {def.args >= 1 && <input type="text" className="flex-1 mono !py-[3px]" value={f.value ?? ""} onChange={(e) => upd(i, { value: e.target.value })} onKeyDown={(e) => e.key === "Enter" && onApply()} autoFocus={i === filters.length - 1} />}
                {def.args === 2 && <input type="text" className="flex-1 mono !py-[3px]" placeholder="and" value={f.value2 ?? ""} onChange={(e) => upd(i, { value2: e.target.value })} onKeyDown={(e) => e.key === "Enter" && onApply()} />}
              </>
            )}
            {f.op !== "raw" && <select className="!py-[3px] w-20 text-xs" value="" onChange={(e) => { if (e.target.value === "raw") upd(i, { op: "raw", value: "" }); }}><option value="">…</option><option value="raw">raw SQL</option></select>}
            <button className="icon-btn" onClick={() => onChange(filters.filter((_, j) => j !== i))}><X size={13} /></button>
          </div>
        );
      })}
      <div className="flex items-center gap-2">
        <button className="btn btn-ghost text-xs" onClick={add}><Plus size={12} /> Add filter</button>
        <button className="btn btn-primary text-xs !py-[2px]" onClick={onApply}>Apply (⏎)</button>
        <span className="text-dim text-xs">Filters are combined with AND.</span>
      </div>
    </div>
  );
}
