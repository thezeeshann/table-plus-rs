import { useRef, useState } from "react";

/** Drag handle between two panes. `onDrag` receives the pointer delta since the last event. */
export function Splitter({ direction, onDrag, onEnd }: { direction: "h" | "v"; onDrag: (delta: number) => void; onEnd?: () => void }) {
  const [dragging, setDragging] = useState(false);
  const last = useRef(0);
  return (
    <div
      className={`${direction === "h" ? "splitter-h" : "splitter-v"} ${dragging ? "dragging" : ""}`}
      onMouseDown={(e) => {
        e.preventDefault();
        setDragging(true);
        last.current = direction === "h" ? e.clientX : e.clientY;
        const move = (ev: MouseEvent) => {
          const cur = direction === "h" ? ev.clientX : ev.clientY;
          onDrag(cur - last.current);
          last.current = cur;
        };
        const up = () => {
          setDragging(false);
          onEnd?.();
          window.removeEventListener("mousemove", move);
          window.removeEventListener("mouseup", up);
        };
        window.addEventListener("mousemove", move);
        window.addEventListener("mouseup", up);
      }}
    />
  );
}
