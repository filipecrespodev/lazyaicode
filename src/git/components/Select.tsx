import { Check, ChevronDown } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { portalRoot } from "@/embed";

export type SelectOption = { value: string; label: string };

/** Theme-aware replacement for the native <select>, whose popup is drawn by
 * the OS and ignores the app theme. The list portals to <body> (fixed
 * position) so scrolling/overflow-hidden ancestors never clip it. */
export function Select({
  value,
  options,
  onChange,
  className = "",
}: {
  value: string;
  options: SelectOption[];
  onChange: (value: string) => void;
  className?: string;
}) {
  const triggerRef = useRef<HTMLButtonElement>(null);
  const [rect, setRect] = useState<DOMRect | null>(null);

  useEffect(() => {
    if (!rect) return;
    const close = () => setRect(null);
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && close();
    document.addEventListener("keydown", onKey);
    window.addEventListener("resize", close);
    return () => {
      document.removeEventListener("keydown", onKey);
      window.removeEventListener("resize", close);
    };
  }, [rect]);

  const selected = options.find((o) => o.value === value);

  return (
    <>
      <button
        ref={triggerRef}
        type="button"
        onClick={() => setRect(triggerRef.current?.getBoundingClientRect() ?? null)}
        className={`flex w-full items-center justify-between gap-2 rounded bg-neutral-800 px-2 py-1 text-left text-sm text-neutral-100 hover:bg-neutral-700 ${className}`}
      >
        <span className="truncate">{selected?.label ?? value}</span>
        <ChevronDown size={13} className="shrink-0 text-neutral-400" />
      </button>
      {rect &&
        createPortal(
          <>
            <div className="fixed inset-0 z-[300]" onMouseDown={() => setRect(null)} />
            <ul
              role="listbox"
              style={{
                left: rect.left,
                width: rect.width,
                // open upward when there's no room below
                ...(window.innerHeight - rect.bottom < 220
                  ? { bottom: window.innerHeight - rect.top + 4 }
                  : { top: rect.bottom + 4 }),
              }}
              className="fixed z-[301] max-h-52 overflow-y-auto rounded border border-neutral-700 bg-neutral-900 py-1 text-sm shadow-xl"
            >
              {options.map((o) => (
                <li
                  key={o.value}
                  role="option"
                  aria-selected={o.value === value}
                  onClick={() => {
                    onChange(o.value);
                    setRect(null);
                  }}
                  className={`flex cursor-pointer items-center justify-between gap-2 px-2 py-1 hover:bg-neutral-800 ${
                    o.value === value ? "text-emerald-400" : "text-neutral-200"
                  }`}
                >
                  <span className="truncate">{o.label}</span>
                  {o.value === value && <Check size={12} className="shrink-0" />}
                </li>
              ))}
            </ul>
          </>,
          portalRoot(),
        )}
    </>
  );
}
