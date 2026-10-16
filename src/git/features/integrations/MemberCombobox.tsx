import { Search } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { GitLabUser } from "./types";
import { portalRoot } from "@/embed";
import { useT } from "../../../lib/i18n/react";

/** Searchable single-select for GitLab project members — same click-outside
 * popover behavior as components/Popover.tsx and the same search-input/list
 * styling as features/search/RepoSearchPalette.tsx. The panel is portaled to
 * <body> (like components/ContextMenu.tsx) instead of being an absolutely
 * positioned child: this combobox sits inside the create-MR form's
 * `overflow-auto` scroll container, and a position:absolute panel clipped by
 * a scrolling ancestor causes WebKitGTK to leave stale compositing artifacts
 * on screen after the panel closes. */
export function MemberCombobox({
  selectedId,
  users,
  onChange,
}: {
  selectedId: number | null;
  users: GitLabUser[];
  onChange: (id: number | null) => void;
}) {
  const t = useT();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [rect, setRect] = useState<{ top: number; left: number; width: number } | null>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!open) return;
    const trigger = triggerRef.current;
    if (trigger) {
      const r = trigger.getBoundingClientRect();
      setRect({ top: r.bottom, left: r.left, width: r.width });
    }
    inputRef.current?.focus();
    const onPointerDown = (e: PointerEvent) => {
      const target = e.target as Node;
      if (
        triggerRef.current &&
        !triggerRef.current.contains(target) &&
        panelRef.current &&
        !panelRef.current.contains(target)
      ) {
        setOpen(false);
      }
    };
    document.addEventListener("pointerdown", onPointerDown);
    // The trigger lives inside the modal's scrollable form, so a stale fixed-
    // position panel would drift from it on scroll — just close instead.
    const onScroll = () => setOpen(false);
    document.addEventListener("scroll", onScroll, true);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
      document.removeEventListener("scroll", onScroll, true);
    };
  }, [open]);

  const selectedUser = users.find((u) => u.id === selectedId) ?? null;

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return users;
    return users.filter((u) => u.name.toLowerCase().includes(q) || u.username.toLowerCase().includes(q));
  }, [users, query]);

  const select = (id: number | null) => {
    onChange(id);
    setOpen(false);
    setQuery("");
  };

  return (
    <>
      <button
        ref={triggerRef}
        type="button"
        onClick={() => setOpen((o) => !o)}
        className="mt-0.5 flex w-full items-center justify-between gap-2 rounded bg-neutral-800 px-2 py-1 text-left text-sm text-neutral-100"
      >
        <span className="truncate">
          {selectedUser ? `${selectedUser.name} (@${selectedUser.username})` : t("gitIntegrations.memberNone")}
        </span>
      </button>
      {open &&
        rect &&
        createPortal(
          <div
            ref={panelRef}
            style={{ position: "fixed", top: rect.top, left: rect.left, width: rect.width }}
            className="z-[300] mt-1 rounded border border-neutral-700 bg-neutral-900 shadow-xl"
          >
            <div className="flex items-center gap-2 border-b border-neutral-800 px-2 py-1.5">
              <Search size={13} className="shrink-0 text-neutral-500" />
              <input
                ref={inputRef}
                value={query}
                onChange={(e) => setQuery(e.currentTarget.value)}
                placeholder={t("gitIntegrations.searchMember")}
                className="min-w-0 flex-1 bg-transparent text-sm text-neutral-100 outline-none placeholder:text-neutral-500"
              />
            </div>
            <ul className="max-h-48 overflow-auto p-1">
              <li
                onClick={() => select(null)}
                className={`cursor-pointer rounded px-2 py-1.5 text-sm ${
                  selectedId === null ? "bg-neutral-800 text-neutral-100" : "text-neutral-400 hover:bg-neutral-800"
                }`}
              >
                {t("gitIntegrations.memberNone")}
              </li>
              {filtered.map((u) => (
                <li
                  key={u.id}
                  onClick={() => select(u.id)}
                  className={`cursor-pointer truncate rounded px-2 py-1.5 text-sm ${
                    u.id === selectedId
                      ? "bg-neutral-800 text-neutral-100"
                      : "text-neutral-200 hover:bg-neutral-800"
                  }`}
                >
                  {u.name} <span className="text-neutral-500">(@{u.username})</span>
                </li>
              ))}
              {filtered.length === 0 && (
                <li className="px-2 py-1.5 text-sm text-neutral-500">{t("gitIntegrations.noMembers")}</li>
              )}
            </ul>
          </div>,
          portalRoot(),
        )}
    </>
  );
}
