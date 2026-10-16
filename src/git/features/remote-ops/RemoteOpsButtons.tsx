import { useIsMutating, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowDownToLine, ChevronDown, Cloud, Download, Loader2, Upload } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { useContextMenu } from "@/components/ContextMenu";
import { Modal } from "@/components/Modal";
import { getRebaseStatus } from "@/features/rebase/api";
import { useRebaseStore } from "@/features/rebase/store";
import { confirm } from "@/lib/confirm-store";
import { queryKeys } from "@/lib/query-keys";
import type { GitCommandError } from "@/lib/tauri-client";
import { errorMessage } from "@/lib/tauri-client";
import { useTauriEvent } from "@/lib/tauri-events";
import { fetchRemote, getUpstreamStatus, pullBranch, pushBranch } from "./api";
import { pullSummary, pullTitle, strategyOptions } from "./pull";
import type { FetchProgressEvent, PullStrategy, PushProgressEvent } from "./types";
import { useRemotes } from "./useRemotes";
import { useT } from "../../../lib/i18n/react";

// The dropdown half of a split button: as tall as its main half (the row
// stretches both) with the chevron centered in it.
const SPLIT_ARROW =
  "flex items-center rounded-r border-l border-neutral-700 bg-neutral-800 px-1 text-neutral-300 hover:bg-neutral-700 disabled:opacity-50";

export function RemoteOpsButtons({ repoId, branch }: { repoId: string; branch: string | null }) {
  const queryClient = useQueryClient();
  const t = useT();
  const anyMutating = useIsMutating() > 0;
  const remoteNames = (useRemotes(repoId).data ?? []).map((r) => r.name);
  const fetchMenu = useContextMenu();
  // Push target picker, when the branch has no upstream and there's no `origin`.
  const [picking, setPicking] = useState<{ remotes: string[]; resolve: (r: string | null) => void } | null>(null);

  const setRebaseStatus = useRebaseStore((s) => s.setStatus);
  const pullMenu = useContextMenu();

  // One cheap ahead/behind lookup (no network); refreshed with the branches
  // query on every poll, fetch and checkout since its key nests under it.
  const { data: upstream } = useQuery({
    queryKey: queryKeys.upstreamStatus(repoId),
    queryFn: () => getUpstreamStatus(repoId),
    enabled: branch !== null,
    staleTime: 2000,
  });

  const invalidate = () => {
    queryClient.invalidateQueries({ queryKey: queryKeys.branches(repoId) });
    queryClient.invalidateQueries({ queryKey: queryKeys.commitLog(repoId) });
    queryClient.invalidateQueries({ queryKey: queryKeys.headCommit(repoId) });
    queryClient.invalidateQueries({ queryKey: queryKeys.repoStatus(repoId) });
  };

  // Plain toasts, not `.loading(...)` — sonner hardcodes no close button for
  // loading-type toasts, but these can run long (a big fetch) and closing the
  // toast doesn't cancel the operation, so the user should be free to dismiss it.
  useTauriEvent<FetchProgressEvent>("git://fetch-progress", (p) => {
    if (p.repoId !== repoId || p.background) return;
    toast(t("gitRemote.fetchProgress", { received: p.receivedObjects, total: p.totalObjects }), {
      id: "fetch",
      icon: <Loader2 size={14} className="animate-spin" />,
    });
  });
  useTauriEvent<PushProgressEvent>("git://push-progress", (p) => {
    if (p.repoId !== repoId) return;
    toast(t("gitRemote.pushProgress", { current: p.current, total: p.total }), {
      id: "push",
      icon: <Loader2 size={14} className="animate-spin" />,
    });
  });

  /** null: every remote. */
  const fetchMutation = useMutation({
    mutationFn: (remote: string | null) => fetchRemote(repoId, remote),
  });
  const pushMutation = useMutation({
    mutationFn: (vars: { remote: string | null; force: boolean }) =>
      pushBranch(repoId, vars.remote, branch as string, vars.force),
  });

  const doFetch = (remote: string | null = null) => {
    toast.promise(fetchMutation.mutateAsync(remote), {
      id: "fetch",
      loading: remote ? t("gitRemote.fetchingFrom", { remote }) : t("gitRemote.fetchingAll"),
      success: (s) => {
        invalidate();
        return t(s.receivedObjects === 1 ? "gitRemote.fetchDoneOne" : "gitRemote.fetchDoneOther", { count: s.receivedObjects });
      },
      // Some remotes may still have been fetched.
      error: (e) => {
        invalidate();
        return errorMessage(e);
      },
    });
  };

  const pickRemote = (remotes: string[]) =>
    new Promise<string | null>((resolve) => setPicking({ remotes, resolve }));

  /** `remote` null: the branch's upstream remote, else origin, else the only one — asks when ambiguous. */
  const doPush = async (force = false, skipFetch = false, remote: string | null = null) => {
    if (!branch) return;

    if (!skipFetch) {
      toast.loading(t("gitRemote.fetchingBeforePush"), { id: "push" });
      try {
        await fetchMutation.mutateAsync(remote);
      } catch (e) {
        // With several remotes, one being down mustn't block pushing to another.
        if (remote || remoteNames.length <= 1) {
          toast.error(t("gitRemote.fetchBeforePushFailed", { error: errorMessage(e) }), { id: "push" });
          return;
        }
      }
      invalidate();
    }

    toast.loading(t(force ? "gitRemote.forcePushing" : "gitRemote.pushing", { branch }), { id: "push" });
    try {
      const s = await pushMutation.mutateAsync({ remote, force });
      invalidate();
      toast.success(t("gitRemote.pushDone", { branch: s.branch, remote: s.remoteName }), { id: "push" });
    } catch (e) {
      const err = e as GitCommandError;
      if (err?.kind === "RemoteRequired" && err.details.remotes.length > 0) {
        toast.dismiss("push");
        const chosen = await pickRemote(err.details.remotes);
        if (chosen) doPush(force, true, chosen);
        return;
      }
      if (!force && err?.kind === "PushRejectedNonFastForward") {
        toast.dismiss("push");
        const confirmed = await confirm(
          t("gitRemote.forcePushConfirm"),
          { title: t("gitRemote.forcePushTitle"), kind: "warning" },
        );
        if (confirmed) doPush(true, true, remote);
        return;
      }
      toast.error(errorMessage(e), { id: "push" });
    }
  };

  const pullMutation = useMutation({
    mutationFn: (strategy: PullStrategy | null) => pullBranch(repoId, strategy),
  });

  // `strategy` null: whatever the repo's pull.rebase / pull.ff say.
  const doPull = async (strategy: PullStrategy | null = null) => {
    toast.loading(t("gitRemote.pulling", { branch: branch ?? "" }), { id: "pull" });
    try {
      const r = await pullMutation.mutateAsync(strategy);
      toast.dismiss("fetch");
      invalidate();
      queryClient.invalidateQueries({ queryKey: queryKeys.workingStatus(repoId) });
      queryClient.invalidateQueries({ queryKey: ["workdirDiff", repoId] });
      // The pull is now the top undo entry.
      queryClient.invalidateQueries({ queryKey: queryKeys.undoStatus(repoId) });
      const summary = pullSummary(r);
      if (summary && r.autostashConflicts.length > 0) {
        // Pulled fine, but the stashed local edits clash with what came in.
        toast.warning(
          t(r.autostashConflicts.length === 1 ? "gitRemote.pullAutostashConflictsOne" : "gitRemote.pullAutostashConflictsOther", {
            summary,
            count: r.autostashConflicts.length,
          }),
          { id: "pull" },
        );
        setRebaseStatus(await getRebaseStatus(repoId));
        return;
      }
      if (summary) {
        toast.success(summary, { id: "pull" });
        return;
      }
      toast.error(t(r.conflicts.length === 1 ? "gitRemote.pullConflictsOne" : "gitRemote.pullConflictsOther", { count: r.conflicts.length }), {
        id: "pull",
      });
      setRebaseStatus(await getRebaseStatus(repoId));
    } catch (e) {
      toast.dismiss("fetch");
      invalidate();
      const err = e as GitCommandError;
      if (err?.kind === "PullNotFastForward") {
        toast.dismiss("pull");
        const merge = await confirm(
          t("gitRemote.pullNotFastForwardConfirm", { upstream: err.details.upstream }),
          { title: t("gitRemote.pull"), kind: "warning" },
        );
        if (merge) doPull("Merge");
        return;
      }
      toast.error(errorMessage(e), { id: "pull" });
    }
  };

  const openPullMenu = (e: React.MouseEvent) =>
    pullMenu.open(
      e,
      strategyOptions(upstream?.strategy).map((o) => ({ label: o.label, onClick: () => doPull(o.strategy) })),
    );

  const behind = upstream?.behind ?? 0;
  const pullDisabled = !branch || anyMutating || upstream === null;

  const answerPick = (remote: string | null) => {
    picking?.resolve(remote);
    setPicking(null);
  };

  return (
    <>
      <div className="flex items-stretch">
        <button
          onClick={() => doFetch()}
          disabled={anyMutating}
          title={remoteNames.length > 1 ? t("gitRemote.fetchAllRemotes") : undefined}
          className={`flex items-center gap-1.5 bg-neutral-800 px-3 py-1.5 text-sm hover:bg-neutral-700 disabled:opacity-50 ${
            remoteNames.length > 1 ? "rounded-l" : "rounded"
          }`}
        >
          {fetchMutation.isPending ? <Loader2 size={14} className="animate-spin" /> : <Download size={14} />}{" "}
          {t("gitRemote.fetch")}
        </button>
        {remoteNames.length > 1 && (
          <button
            onClick={(e) =>
              fetchMenu.open(e, [
                { label: t("gitRemote.fetchAllRemotes"), onClick: () => doFetch() },
                ...remoteNames.map((name) => ({ label: t("gitRemote.fetchFromRemote", { remote: name }), onClick: () => doFetch(name) })),
              ])
            }
            disabled={anyMutating}
            title={t("gitRemote.fetchOneRemote")}
            className={SPLIT_ARROW}
          >
            <ChevronDown size={14} />
          </button>
        )}
      </div>
      {fetchMenu.menu}
      <div className="flex items-stretch" onContextMenu={pullDisabled ? undefined : openPullMenu}>
        <button
          onClick={() => doPull()}
          disabled={pullDisabled}
          title={pullTitle(upstream)}
          className="flex items-center gap-1.5 rounded-l bg-neutral-800 px-3 py-1.5 text-sm hover:bg-neutral-700 disabled:opacity-50"
        >
          {pullMutation.isPending ? (
            <Loader2 size={14} className="animate-spin" />
          ) : (
            <ArrowDownToLine size={14} />
          )}{" "}
          {t("gitRemote.pull")}
          {behind > 0 && (
            <span className="rounded-full bg-sky-700 px-1.5 text-[10px] leading-4 text-sky-50">{behind}</span>
          )}
        </button>
        <button
          onClick={openPullMenu}
          disabled={pullDisabled}
          title={t("gitRemote.pullStrategyMenu")}
          className={SPLIT_ARROW}
        >
          <ChevronDown size={14} />
        </button>
      </div>
      {pullMenu.menu}
      <button
        onClick={() => doPush()}
        disabled={!branch || anyMutating}
        className="flex items-center gap-1.5 rounded bg-neutral-800 px-3 py-1.5 text-sm hover:bg-neutral-700 disabled:opacity-50"
      >
        {pushMutation.isPending ? <Loader2 size={14} className="animate-spin" /> : <Upload size={14} />}{" "}
        {t("gitRemote.push")}
      </button>
      {picking && (
        <Modal onClose={() => answerPick(null)} width="w-[360px]">
          <div className="p-4">
            <h2 className="mb-1 text-sm font-semibold text-neutral-100">{t("gitRemote.pushTo", { branch: branch ?? "" })}</h2>
            <p className="mb-3 text-xs text-neutral-500">
              {t("gitRemote.pushToHint")}
            </p>
            <div className="flex flex-col gap-1">
              {picking.remotes.map((name) => (
                <button
                  key={name}
                  onClick={() => answerPick(name)}
                  className="flex items-center gap-2 rounded bg-neutral-800 px-3 py-1.5 text-left text-sm text-neutral-100 hover:bg-neutral-700"
                >
                  <Cloud size={13} className="shrink-0 text-neutral-400" /> {name}
                </button>
              ))}
            </div>
            <div className="mt-4 flex justify-end">
              <button
                onClick={() => answerPick(null)}
                className="rounded bg-neutral-800 px-3 py-1.5 text-sm text-neutral-200 hover:bg-neutral-700"
              >
                {t("gitCommon.cancel")}
              </button>
            </div>
          </div>
        </Modal>
      )}
    </>
  );
}
