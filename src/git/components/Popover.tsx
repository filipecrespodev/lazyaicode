import { useEffect, useRef, useState, type ReactNode } from "react";

export function Popover({
  trigger,
  children,
  up = false,
}: {
  trigger: (props: { onClick: () => void }) => ReactNode;
  children: ReactNode;
  /** Open above the trigger (for triggers at the bottom of the window). */
  up?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  // Fixed (not absolute) so scroll containers around the trigger don't clip it.
  const [pos, setPos] = useState<{ top?: number; bottom?: number; left: number }>({ left: 0 });

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (e: PointerEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    const close = () => setOpen(false);
    document.addEventListener("pointerdown", onPointerDown);
    window.addEventListener("resize", close);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
      window.removeEventListener("resize", close);
    };
  }, [open]);

  return (
    <div className="relative" ref={containerRef}>
      {trigger({
        onClick: () => {
          const r = containerRef.current?.getBoundingClientRect();
          if (r) setPos({ ...(up ? { bottom: window.innerHeight - r.top + 4 } : { top: r.bottom + 4 }), left: Math.max(8, Math.min(r.left, window.innerWidth - 264)) });
          setOpen((o) => !o);
        },
      })}
      {open && (
        <div style={pos} className="fixed z-20 w-64 rounded border border-neutral-700 bg-neutral-900 p-2 shadow-xl">
          {children}
        </div>
      )}
    </div>
  );
}
