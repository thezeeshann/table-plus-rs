import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";

export interface MenuItem {
  label?: string;
  shortcut?: string;
  onClick?: () => void;
  disabled?: boolean;
  danger?: boolean;
  separator?: boolean;
  icon?: ReactNode;
}

export interface MenuState { x: number; y: number; items: MenuItem[] }

export function ContextMenu({ menu, onClose }: { menu: MenuState | null; onClose: () => void }) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ x: 0, y: 0 });
  useLayoutEffect(() => {
    if (!menu || !ref.current) return;
    const r = ref.current.getBoundingClientRect();
    const x = Math.min(menu.x, window.innerWidth - r.width - 8);
    const y = Math.min(menu.y, window.innerHeight - r.height - 8);
    setPos({ x: Math.max(4, x), y: Math.max(4, y) });
  }, [menu]);
  useEffect(() => {
    if (!menu) return;
    const close = (e: Event) => {
      if (ref.current && e.target instanceof Node && ref.current.contains(e.target)) return;
      onClose();
    };
    const key = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    window.addEventListener("mousedown", close, true);
    window.addEventListener("blur", onClose);
    window.addEventListener("keydown", key, true);
    window.addEventListener("wheel", close, true);
    return () => {
      window.removeEventListener("mousedown", close, true);
      window.removeEventListener("blur", onClose);
      window.removeEventListener("keydown", key, true);
      window.removeEventListener("wheel", close, true);
    };
  }, [menu, onClose]);
  if (!menu) return null;
  return createPortal(
    <div ref={ref} className="menu" style={{ left: pos.x, top: pos.y }} onContextMenu={(e) => e.preventDefault()}>
      {menu.items.map((it, i) =>
        it.separator ? (
          <div key={i} className="menu-sep" />
        ) : (
          <div
            key={i}
            className={`menu-item ${it.disabled ? "disabled" : ""} ${it.danger ? "danger" : ""}`}
            onClick={() => { if (!it.disabled) { onClose(); it.onClick?.(); } }}
          >
            <span className="flex items-center gap-2">{it.icon}{it.label}</span>
            {it.shortcut && <span className="kbd">{it.shortcut}</span>}
          </div>
        ),
      )}
    </div>,
    document.body,
  );
}

export function useContextMenu() {
  const [menu, setMenu] = useState<MenuState | null>(null);
  const open = (e: { clientX: number; clientY: number; preventDefault?: () => void }, items: MenuItem[]) => {
    e.preventDefault?.();
    setMenu({ x: e.clientX, y: e.clientY, items });
  };
  const close = () => setMenu(null);
  return { menu, open, close };
}
