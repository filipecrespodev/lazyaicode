import { useRef } from "react";

/** A draggable strip that reports drag deltas along one axis. Uses Pointer
 * Events with explicit capture on the handle itself — more robust than
 * window-level mouse listeners inside an embedded webview (keeps receiving
 * move/up events no matter how fast the cursor moves or briefly leaves the
 * element). */
export function ResizeHandle({
  onDrag,
  axis = "x",
  className = "",
}: {
  onDrag: (delta: number) => void;
  axis?: "x" | "y";
  className?: string;
}) {
  const last = useRef(0);

  return (
    <div
      onPointerDown={(e) => {
        e.preventDefault();
        e.currentTarget.setPointerCapture(e.pointerId);
        last.current = axis === "x" ? e.clientX : e.clientY;
      }}
      onPointerMove={(e) => {
        if (e.buttons !== 1) return;
        const pos = axis === "x" ? e.clientX : e.clientY;
        const delta = pos - last.current;
        if (delta === 0) return;
        last.current = pos;
        onDrag(delta);
      }}
      onPointerUp={(e) => {
        e.currentTarget.releasePointerCapture(e.pointerId);
      }}
      className={`shrink-0 touch-none bg-neutral-800 hover:bg-blue-500 active:bg-blue-500 ${
        axis === "x" ? "w-2 cursor-col-resize" : "h-2 cursor-row-resize"
      } ${className}`}
    />
  );
}
