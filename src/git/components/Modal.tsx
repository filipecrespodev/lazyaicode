import { useEffect } from "react";
import { createPortal } from "react-dom";
import { portalRoot } from "@/embed";

export function Modal({
  onClose,
  children,
  align = "center",
  width = "w-[560px]",
  dismissible = true,
}: {
  onClose: () => void;
  children: React.ReactNode;
  align?: "center" | "top";
  /** Tailwind width class for the dialog. */
  width?: string;
  /** false: neither Escape nor a backdrop click closes it (only explicit buttons). */
  dismissible?: boolean;
}) {
  useEffect(() => {
    if (!dismissible) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [onClose, dismissible]);

  return createPortal(
    <div
      className={`fixed inset-0 z-[200] flex justify-center bg-black/60 ${
        align === "top" ? "items-start pt-[15vh]" : "items-center"
      }`}
      onMouseDown={(e) => {
        if (dismissible && e.target === e.currentTarget) onClose();
      }}
    >
      <div className={`${width} max-w-[90vw] overflow-hidden rounded-lg border border-neutral-800 bg-neutral-900 shadow-2xl`}>
        {children}
      </div>
    </div>,
    portalRoot(),
  );
}
