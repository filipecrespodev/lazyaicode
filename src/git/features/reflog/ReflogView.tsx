import { useIsMutating, useQuery } from "@tanstack/react-query";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { Loader2, Search, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Select } from "@/components/Select";
import { toast } from "sonner";
import { useContextMenu } from "@/components/ContextMenu";
import { listBranches } from "@/features/branches/api";
import { CreateBranchModal } from "@/features/branches/CreateBranchModal";
import { useBranchActions } from "@/features/branches/useBranchActions";
import { formatDateTime } from "@/lib/format-date";
import { gravatarUrl } from "@/lib/gravatar";
import { queryKeys } from "@/lib/query-keys";
import { errorMessage } from "@/lib/tauri-client";
import { getReflog } from "./api";
import { parseReflogMessage, type ReflogKind } from "./reflogAction";
import type { ReflogEntry } from "./types";
import { overlayClass, overlayOrInline } from "@/embed";
import { useT } from "../../../lib/i18n/react";

const KIND_CLASSES: Record<ReflogKind, string> = {
  commit: "bg-emerald-900 text-emerald-200",
  checkout: "bg-neutral-800 text-neutral-300",
  reset: "bg-red-900 text-red-300",
  merge: "bg-violet-900 text-violet-200",
  rebase: "bg-amber-900 text-amber-200",
  "cherry-pick": "bg-amber-900 text-amber-200",
  branch: "bg-neutral-800 text-neutral-300",
  other: "bg-neutral-800 text-neutral-400",
};

const isZeroSha = (sha: string) => /^0+$/.test(sha);

/** Full-screen reflog of HEAD (or a local branch): every position the ref
 * has been at, newest first — the way back to commits lost to a reset,
 * rebase or deleted branch. Right-click an entry to recover it. */
export function ReflogView({
  repoId,
  currentBranch,
  hasLocalChanges,
  onClose,
}: {
  repoId: string;
  currentBranch: string | null;
  hasLocalChanges: boolean;
  onClose?: () => void;
}) {
  const t = useT();
  const [refName, setRefName] = useState("HEAD");
  const [filter, setFilter] = useState("");
  const [createBranchAt, setCreateBranchAt] = useState<string | null>(null);
  const { open: openMenu, menu } = useContextMenu();
  const menuOpen = menu !== null;
  const anyMutating = useIsMutating() > 0;
  const { checkoutDetachedWithFeedback, resetToWithFeedback } = useBranchActions(repoId);

  const { data: branches } = useQuery({
    queryKey: queryKeys.branches(repoId),
    queryFn: () => listBranches(repoId),
  });
  const localBranches = (branches ?? []).filter((b) => !b.isRemote).map((b) => b.name);

  const { data: entries, isPending, error } = useQuery({
    queryKey: queryKeys.reflog(repoId, refName),
    queryFn: () => getReflog(repoId, refName),
    refetchOnMount: "always",
  });

  const visible = useMemo(() => {
    const needle = filter.trim().toLowerCase();
    if (!needle) return entries ?? [];
    return (entries ?? []).filter(
      (e) =>
        e.message.toLowerCase().includes(needle) ||
        e.newSha.startsWith(needle) ||
        (e.commitSummary ?? "").toLowerCase().includes(needle),
    );
  }, [entries, filter]);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      // Esc closes the open menu or modal first, not the whole view.
      if (e.key === "Escape" && !createBranchAt && !menuOpen) onClose?.();
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [onClose, createBranchAt, menuOpen]);

  const onEntryContextMenu = (e: React.MouseEvent, entry: ReflogEntry) => {
    const sha = entry.newSha;
    const exists = entry.commitSummary !== null;
    openMenu(e, [
      {
        label: t("gitReflog.copySha"),
        onClick: () => {
          writeText(sha);
          toast.success(t("gitReflog.shaCopied"));
        },
      },
      { label: t("gitReflog.createBranchHere"), onClick: () => setCreateBranchAt(sha), disabled: anyMutating || !exists },
      {
        label: t("gitReflog.checkoutCommit"),
        onClick: () => checkoutDetachedWithFeedback(sha),
        disabled: anyMutating || !exists,
      },
      ...(currentBranch
        ? [
            {
              label: t("gitReflog.resetTo", { branch: currentBranch }),
              submenu: [
                {
                  label: t("gitReflog.resetSoft"),
                  onClick: () => resetToWithFeedback({ sha, hard: false }),
                  disabled: anyMutating || hasLocalChanges || !exists,
                },
                {
                  label: t("gitReflog.resetHard"),
                  onClick: () => resetToWithFeedback({ sha, hard: true }),
                  disabled: anyMutating || hasLocalChanges || !exists,
                },
              ],
            },
          ]
        : []),
    ]);
  };

  return overlayOrInline(
    <div className={`${overlayClass(!!onClose)} flex flex-col bg-neutral-950 text-neutral-200`}>
      <div className="flex shrink-0 items-center justify-between gap-3 border-b border-neutral-800 px-3 py-2">
        <div className="flex min-w-0 items-baseline gap-3">
          <span className="text-sm font-semibold">Reflog</span>
          <span className="truncate text-xs text-neutral-500">
            {t("gitReflog.subtitle", { ref: refName })}
          </span>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <Select
            value={refName}
            onChange={setRefName}
            options={[{ value: "HEAD", label: "HEAD" }, ...localBranches.map((b) => ({ value: b, label: b }))]}
            className="max-w-56"
          />
          <label className="flex items-center gap-1.5 rounded bg-neutral-900 px-2 py-1 text-neutral-500">
            <Search size={13} className="shrink-0" />
            <input
              value={filter}
              onChange={(e) => setFilter(e.currentTarget.value)}
              placeholder={t("gitReflog.filterPlaceholder")}
              className="w-56 bg-transparent text-sm text-neutral-200 outline-none placeholder:text-neutral-500"
            />
          </label>
          {onClose && (
            <button
              title={t("gitReflog.close")}
              onClick={onClose}
              className="rounded p-1 text-neutral-400 hover:bg-neutral-800 hover:text-neutral-100"
            >
              <X size={15} />
            </button>
          )}
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-auto">
        {isPending && (
          <div className="flex items-center gap-2 p-4 text-sm text-neutral-500">
            <Loader2 size={14} className="animate-spin" /> {t("gitReflog.loading")}
          </div>
        )}
        {error && <p className="p-4 text-sm text-red-400">{errorMessage(error)}</p>}
        {entries && visible.length === 0 && (
          <p className="p-4 text-sm text-neutral-500">
            {entries.length === 0 ? t("gitReflog.noReflog", { ref: refName }) : t("gitReflog.noMatch")}
          </p>
        )}
        {visible.length > 0 && (
          <table className="w-full text-sm">
            <thead className="sticky top-0 bg-neutral-950">
              <tr className="border-b border-neutral-800 text-left text-[11px] uppercase text-neutral-500">
                <th className="w-28 px-3 py-1.5 font-normal">{t("gitReflog.colEntry")}</th>
                <th className="w-20 py-1.5 font-normal">{t("gitReflog.colSha")}</th>
                <th className="w-36 py-1.5 font-normal">{t("gitReflog.colAction")}</th>
                <th className="py-1.5 font-normal">{t("gitReflog.colDescription")}</th>
                <th className="w-48 py-1.5 font-normal">{t("gitReflog.colAuthor")}</th>
                <th className="w-36 px-3 py-1.5 text-right font-normal">{t("gitReflog.colDate")}</th>
              </tr>
            </thead>
            <tbody>
              {visible.map((entry) => {
                const action = parseReflogMessage(entry.message);
                const gone = entry.commitSummary === null;
                return (
                  <tr
                    key={entry.selector}
                    onContextMenu={(e) => onEntryContextMenu(e, entry)}
                    className="cursor-default border-b border-neutral-800/60 hover:bg-neutral-900"
                  >
                    <td className="px-3 py-1.5 font-mono text-xs text-neutral-500">{entry.selector}</td>
                    <td
                      className={`py-1.5 font-mono text-xs ${gone ? "text-neutral-600 line-through" : "text-blue-500"}`}
                      title={gone ? t("gitReflog.commitGone") : entry.newSha}
                    >
                      {entry.newSha.slice(0, 7)}
                    </td>
                    <td className="py-1.5">
                      {action.label ? (
                        <span className={`rounded px-1.5 py-0.5 text-[11px] ${KIND_CLASSES[action.kind]}`}>
                          {action.label}
                        </span>
                      ) : null}
                    </td>
                    <td className="max-w-0 py-1.5 pr-3">
                      <div className="truncate text-neutral-200" title={entry.message}>
                        {action.detail}
                      </div>
                      {entry.commitSummary && entry.commitSummary !== action.detail && (
                        <div className="truncate text-xs text-neutral-500">{entry.commitSummary}</div>
                      )}
                      {isZeroSha(entry.oldSha) && (
                        <div className="text-xs text-neutral-500">{t("gitReflog.refCreated")}</div>
                      )}
                    </td>
                    <td className="max-w-0 py-1.5 pr-3">
                      <div className="flex items-center gap-2" title={entry.committerEmail}>
                        <img
                          src={gravatarUrl(entry.committerEmail, 40)}
                          alt=""
                          className="h-5 w-5 shrink-0 rounded-full"
                        />
                        <span className="truncate text-neutral-400">{entry.committerName}</span>
                      </div>
                    </td>
                    <td className="px-3 py-1.5 text-right tabular-nums text-neutral-400">
                      {formatDateTime(entry.time)}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
      </div>
      {menu}
      {createBranchAt && (
        <CreateBranchModal repoId={repoId} startSha={createBranchAt} onClose={() => setCreateBranchAt(null)} />
      )}
    </div>,
    !!onClose,
  );
}
