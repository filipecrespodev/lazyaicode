import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, Loader2 } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { Modal } from "@/components/Modal";
import { Select } from "@/components/Select";
import { listBranches } from "@/features/branches/api";
import { isRepoOpen, openFolder } from "@/embed";
import { confirm } from "@/lib/confirm-store";
import { queryKeys } from "@/lib/query-keys";
import { errorMessage } from "@/lib/tauri-client";
import { addWorktree, listWorktrees, pruneWorktrees, removeWorktree, worktreesKey, type WorktreeInfo } from "./api";
import { suggestWorktreePath } from "./paths";
import { useT } from "../../../lib/i18n/react";

export function WorktreesModal({
  repoId,
  repoPath,
  onClose,
}: {
  repoId: string;
  repoPath: string;
  onClose: () => void;
}) {
  const t = useT();
  const queryClient = useQueryClient();
  const { data: worktrees, error, isPending } = useQuery({
    queryKey: worktreesKey(repoId),
    queryFn: () => listWorktrees(repoId),
    staleTime: 10_000,
  });
  const { data: branches } = useQuery({
    queryKey: queryKeys.branches(repoId),
    queryFn: () => listBranches(repoId),
  });

  const [newBranch, setNewBranch] = useState(true);
  const [branch, setBranch] = useState("");
  const [path, setPath] = useState("");
  // The path follows the branch name until edited by hand.
  const [pathEdited, setPathEdited] = useState(false);
  const chooseMode = (create: boolean) => {
    setNewBranch(create);
    setBranch("");
  };
  const shownPath = pathEdited ? path : branch ? suggestWorktreePath(repoPath, branch) : "";

  const checkedOut = new Set((worktrees ?? []).map((w) => w.branch).filter(Boolean));
  const freeBranches = (branches ?? []).filter((b) => !b.isRemote && !checkedOut.has(b.name));

  const onListed = (list: WorktreeInfo[]) => queryClient.setQueryData(worktreesKey(repoId), list);

  const add = useMutation({
    mutationFn: () => addWorktree(repoId, { path: shownPath, branch, newBranch, startPoint: null }),
    onSuccess: (list) => {
      onListed(list);
      queryClient.invalidateQueries({ queryKey: queryKeys.branches(repoId) });
      queryClient.invalidateQueries({ queryKey: queryKeys.commitLog(repoId) });
      const added = shownPath;
      toast.success(t("gitWorktrees.created", { path: added }), {
        action: { label: t("gitWorktrees.open"), onClick: () => openFolder(added) },
      });
      setBranch("");
      setPath("");
      setPathEdited(false);
    },
    onError: (e) => toast.error(errorMessage(e)),
  });

  const remove = useMutation({
    mutationFn: (vars: { name: string; force: boolean }) => removeWorktree(repoId, vars.name, vars.force),
    onSuccess: onListed,
  });

  const prune = useMutation({
    mutationFn: () => pruneWorktrees(repoId),
    onSuccess: onListed,
    onError: (e) => toast.error(errorMessage(e)),
  });

  const removeWithConfirm = async (wt: WorktreeInfo) => {
    if (!wt.name) return;
    const openInTab = isRepoOpen(wt.path);
    const ok = await confirm(
      t("gitWorktrees.removeConfirm", { path: wt.path }) + (openInTab ? ` ${t("gitWorktrees.removeOpenInTab")}` : ""),
      {
        title: t("gitWorktrees.removeTitle"),
        kind: "warning",
      },
    );
    if (!ok) return;
    try {
      await remove.mutateAsync({ name: wt.name, force: false });
    } catch (e) {
      const message = errorMessage(e);
      if (!/não commitadas|bloquead/.test(message)) return void toast.error(message);
      const force = await confirm(t("gitWorktrees.forceConfirm", { message }), {
        title: t("gitWorktrees.forceTitle"),
        kind: "error",
      });
      if (force) {
        await remove.mutateAsync({ name: wt.name, force: true }).catch((e2) => toast.error(errorMessage(e2)));
      }
    }
  };

  const browse = async () => {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked !== "string") return;
    const leaf = suggestWorktreePath(repoPath, branch).split(/[\\/]/).pop() ?? "worktree";
    const sep = picked.includes("\\") ? "\\" : "/";
    setPath(`${picked.replace(/[\\/]+$/, "")}${sep}${leaf}`);
    setPathEdited(true);
  };

  const anyPrunable = (worktrees ?? []).some((w) => w.prunable);
  const canAdd = !!branch.trim() && !!shownPath.trim() && !add.isPending;

  return (
    <Modal onClose={onClose} align="top" width="w-[680px]">
      <div className="flex max-h-[75vh] flex-col text-sm">
        <div className="flex shrink-0 items-center justify-between border-b border-neutral-800 px-3 py-2">
          <span className="font-semibold text-neutral-200">{t("gitWorktrees.title")}</span>
          {anyPrunable && (
            <button
              onClick={() => prune.mutate()}
              disabled={prune.isPending}
              title={t("gitWorktrees.pruneHint")}
              className="rounded bg-neutral-800 px-2 py-1 text-xs hover:bg-neutral-700 disabled:opacity-50"
            >
              {t("gitWorktrees.prune")}
            </button>
          )}
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto">
          {isPending ? (
            <div className="flex items-center gap-2 p-4 text-neutral-400">
              <Loader2 size={14} className="animate-spin" /> {t("gitWorktrees.loading")}
            </div>
          ) : error ? (
            <p className="p-4 text-red-400">{errorMessage(error)}</p>
          ) : (
            <ul>
              {worktrees?.map((wt) => (
                <li key={wt.path} className="flex items-center gap-2 border-b border-neutral-800 px-3 py-2">
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-1.5">
                      <span className="truncate text-neutral-100">{wt.branch ?? (wt.headSha ? t("gitWorktrees.detached", { sha: wt.headSha.slice(0, 7) }) : "—")}</span>
                      {wt.isMain && <Badge>{t("gitWorktrees.badgeMain")}</Badge>}
                      {wt.isCurrent && <Badge tone="emerald">{t("gitWorktrees.badgeCurrent")}</Badge>}
                      {wt.locked && <Badge tone="amber">{t("gitWorktrees.badgeLocked")}</Badge>}
                      {wt.prunable && <Badge tone="red">{t("gitWorktrees.badgeMissing")}</Badge>}
                    </div>
                    <p className="truncate text-xs text-neutral-500" title={wt.path}>
                      {wt.path}
                    </p>
                  </div>
                  {!wt.isCurrent && !wt.prunable && (
                    <button
                      onClick={() => {
                        openFolder(wt.path);
                        onClose();
                      }}
                      className="shrink-0 rounded bg-neutral-800 px-2 py-1 text-xs hover:bg-neutral-700"
                    >
                      {t("gitWorktrees.openInTab")}
                    </button>
                  )}
                  {!wt.isMain && !wt.isCurrent && (
                    <button
                      onClick={() => removeWithConfirm(wt)}
                      disabled={remove.isPending}
                      className="shrink-0 rounded bg-red-950 px-2 py-1 text-xs text-red-200 hover:bg-red-900 disabled:opacity-50"
                    >
                      {t("gitWorktrees.remove")}
                    </button>
                  )}
                </li>
              ))}
            </ul>
          )}
        </div>

        <form
          onSubmit={(e) => {
            e.preventDefault();
            if (canAdd) add.mutate();
          }}
          className="shrink-0 space-y-2 border-t border-neutral-800 p-3"
        >
          <div className="text-xs uppercase text-neutral-500">{t("gitWorktrees.addWorktree")}</div>
          <div className="flex gap-3 text-xs text-neutral-300">
            <label className="flex items-center gap-1">
              <input type="radio" checked={newBranch} onChange={() => chooseMode(true)} /> {t("gitWorktrees.newBranch")}
            </label>
            <label className="flex items-center gap-1">
              <input type="radio" checked={!newBranch} onChange={() => chooseMode(false)} /> {t("gitWorktrees.existingBranch")}
            </label>
          </div>
          {newBranch ? (
            <input
              value={branch}
              onChange={(e) => setBranch(e.currentTarget.value)}
              placeholder={t("gitWorktrees.newBranchPlaceholder")}
              className="w-full rounded bg-neutral-800 px-2 py-1 text-sm text-neutral-100"
            />
          ) : (
            <Select
              value={branch}
              options={[
                { value: "", label: freeBranches.length ? t("gitWorktrees.chooseBranch") : t("gitWorktrees.noFreeBranch") },
                ...freeBranches.map((b) => ({ value: b.name, label: b.name })),
              ]}
              onChange={setBranch}
            />
          )}
          <div className="flex gap-1">
            <input
              value={shownPath}
              onChange={(e) => {
                setPath(e.currentTarget.value);
                setPathEdited(true);
              }}
              placeholder={t("gitWorktrees.pathPlaceholder")}
              className="min-w-0 flex-1 rounded bg-neutral-800 px-2 py-1 font-mono text-xs text-neutral-100"
            />
            <button
              type="button"
              onClick={browse}
              title={t("gitWorktrees.browse")}
              className="shrink-0 rounded bg-neutral-800 px-2 hover:bg-neutral-700"
            >
              <FolderOpen size={14} />
            </button>
          </div>
          <div className="flex justify-end gap-2 pt-1">
            <button
              type="button"
              onClick={onClose}
              className="rounded border border-neutral-700 px-3 py-1.5 text-sm text-neutral-300 hover:bg-neutral-900"
            >
              {t("gitWorktrees.close")}
            </button>
            <button
              type="submit"
              disabled={!canAdd}
              className="flex items-center gap-1.5 rounded bg-emerald-700 px-3 py-1.5 text-sm text-emerald-50 hover:bg-emerald-800 disabled:opacity-50"
            >
              {add.isPending && <Loader2 size={13} className="animate-spin" />} {t("gitWorktrees.addWorktree")}
            </button>
          </div>
        </form>
      </div>
    </Modal>
  );
}

function Badge({ children, tone = "neutral" }: { children: React.ReactNode; tone?: "neutral" | "emerald" | "amber" | "red" }) {
  const tones = {
    neutral: "bg-neutral-800 text-neutral-300",
    emerald: "bg-emerald-950 text-emerald-300",
    amber: "bg-amber-950 text-amber-300",
    red: "bg-red-950 text-red-300",
  };
  return <span className={`shrink-0 rounded px-1.5 py-px text-[10px] ${tones[tone]}`}>{children}</span>;
}
