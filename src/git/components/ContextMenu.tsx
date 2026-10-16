import { ChevronRight } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { portalRoot } from "@/embed";

export type ContextMenuItem = {
  label: string;
  onClick?: () => void;
  disabled?: boolean;
  submenu?: ContextMenuItem[];
};

/** Custom right-click menu. Use `open` as the `onContextMenu` handler (with
 * the items for that specific target) and render `menu` once anywhere in the
 * tree — it portals to <body> so it's never clipped by an ancestor's
 * `overflow-hidden`. Pairs with the global native-menu suppression in
 * App.tsx: elements that don't call `open` simply show no menu at all. */
export function useContextMenu() {
  const [state, setState] = useState<{ x: number; y: number; items: ContextMenuItem[] } | null>(
    null,
  );

  const open = (e: React.MouseEvent, items: ContextMenuItem[]) => {
    e.preventDefault();
    e.stopPropagation();
    setState({ x: e.clientX, y: e.clientY, items });
  };

  const close = () => setState(null);

  const menu = state && createPortal(<Overlay {...state} onClose={close} />, portalRoot());

  return { open, menu };
}

function Overlay({
  x,
  y,
  items,
  onClose,
}: {
  x: number;
  y: number;
  items: ContextMenuItem[];
  onClose: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [openSubmenu, setOpenSubmenu] = useState<string | null>(null);

  useEffect(() => {
    const onMouseDown = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    };
    // Capture phase + preventDefault: Escape closes just the menu, not the
    // full-screen view underneath that also listens for it.
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      onClose();
    };
    document.addEventListener("mousedown", onMouseDown);
    document.addEventListener("keydown", onKeyDown, true);
    return () => {
      document.removeEventListener("mousedown", onMouseDown);
      document.removeEventListener("keydown", onKeyDown, true);
    };
  }, [onClose]);

  return (
    <div
      ref={ref}
      // Above full-screen views (z 150) and modals (z 200): a menu is always the topmost, short-lived layer.
      style={{ position: "fixed", left: x, top: y, zIndex: 300 }}
      className="min-w-[200px] rounded border border-neutral-700 bg-neutral-900 py-1 text-sm shadow-xl"
    >
      {items.map((item) => (
        <div
          key={item.label}
          className="relative"
          onMouseEnter={() => item.submenu && setOpenSubmenu(item.label)}
          onMouseLeave={() =>
            item.submenu && setOpenSubmenu((s) => (s === item.label ? null : s))
          }
        >
          <button
            onClick={() => {
              if (item.submenu) return;
              item.onClick?.();
              onClose();
            }}
            disabled={item.disabled}
            className="flex w-full items-center justify-between gap-3 px-3 py-1.5 text-left text-neutral-200 hover:bg-neutral-800 disabled:opacity-40"
          >
            <span>{item.label}</span>
            {item.submenu && <ChevronRight size={12} className="shrink-0 opacity-60" />}
          </button>
          {item.submenu && openSubmenu === item.label && (
            <div className="absolute left-full top-0 min-w-[180px] rounded border border-neutral-700 bg-neutral-900 py-1 shadow-xl">
              {item.submenu.map((sub) => (
                <button
                  key={sub.label}
                  onClick={() => {
                    sub.onClick?.();
                    onClose();
                  }}
                  disabled={sub.disabled}
                  className="block w-full px-3 py-1.5 text-left text-neutral-200 hover:bg-neutral-800 disabled:opacity-40"
                >
                  {sub.label}
                </button>
              ))}
            </div>
          )}
        </div>
      ))}
    </div>
  );
}
