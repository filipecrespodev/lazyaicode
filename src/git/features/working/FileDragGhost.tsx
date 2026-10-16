import { portalRoot } from "@/embed";
import { useEffect } from "react";
import { createPortal } from "react-dom";
import { dropAction, useFileDragStore } from "./fileDragStore";

/** Label following the cursor while a file is dragged; also owns the
 * window-level mousemove/mouseup of the gesture and reports the drop. */
export function FileDragGhost({ onDrop }: { onDrop: (path: string, action: "stage" | "unstage") => void }) {
  const drag = useFileDragStore((s) => s.drag);
  const position = useFileDragStore((s) => s.position);

  useEffect(() => {
    if (!drag) return;
    const { move, stop } = useFileDragStore.getState();
    const onMove = (e: MouseEvent) => move(e.clientX, e.clientY);
    const onUp = () => {
      const { drag: d, hoverSide } = useFileDragStore.getState();
      stop();
      const action = d && dropAction(d.from, hoverSide);
      if (d && action) onDrop(d.path, action);
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    const previous = document.body.style.userSelect;
    document.body.style.userSelect = "none";
    return () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
      document.body.style.userSelect = previous;
    };
  }, [drag, onDrop]);

  if (!drag) return null;
  return createPortal(
    <div
      style={{ position: "fixed", left: position.x + 14, top: position.y + 14, zIndex: 200 }}
      className="pointer-events-none max-w-64 truncate rounded bg-neutral-700 px-1.5 py-0.5 text-xs text-neutral-100 shadow-lg"
    >
      {drag.path}
    </div>,
    portalRoot(),
  );
}
