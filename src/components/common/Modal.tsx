import { useEffect, type ReactNode } from "react";

export function Modal({ title, children, footer, onClose, width }: {
  title: ReactNode; children: ReactNode; footer?: ReactNode; onClose: () => void; width?: number;
}) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") { e.stopPropagation(); onClose(); } };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [onClose]);
  return (
    <div className="modal-backdrop" onMouseDown={(e) => { if (e.target === e.currentTarget) onClose(); }}>
      <div className="modal" style={width ? { width } : undefined} onMouseDown={(e) => e.stopPropagation()}>
        <div className="modal-header">{title}</div>
        <div className="modal-body">{children}</div>
        {footer && <div className="modal-footer">{footer}</div>}
      </div>
    </div>
  );
}

export function Field({ label, children, hint }: { label: string; children: ReactNode; hint?: string }) {
  return (
    <label className="flex items-center gap-3 mb-2.5">
      <span className="w-32 text-right text-dim shrink-0">{label}</span>
      <span className="flex-1 flex items-center gap-2">{children}</span>
      {hint && <span className="text-dim text-xs">{hint}</span>}
    </label>
  );
}
