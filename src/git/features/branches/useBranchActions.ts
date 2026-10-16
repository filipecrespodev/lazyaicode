import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { openCompare } from "@/features/history/store";
import { t } from "../../../lib/i18n/react";
import { getRebaseStatus, startRebase } from "@/features/rebase/api";
import { useRebaseStore } from "@/features/rebase/store";
import { usePushBranch } from "@/features/remote-ops/usePushBranch";
import { confirm } from "@/lib/confirm-store";
import { queryKeys } from "@/lib/query-keys";
import { errorMessage } from "@/lib/tauri-client";
import {
  checkoutAt,
  checkoutBranch,
  checkoutCommitDetached,
  cherryPick,
  createBranch,
  createTag,
  deleteLocalTag,
  deleteRemoteTag,
  pushTag,
  deleteBranchBoth,
  deleteLocalBranch,
  deleteRemoteBranch,
  listBranches,
  mergeBranch,
  resetToCommit,
  resetToRef,
  syncBranchToRemote,
} from "./api";

export function useBranchActions(repoId: string) {
  const queryClient = useQueryClient();
  const setRebaseStatus = useRebaseStore((s) => s.setStatus);
  const rebaseInProgress = useRebaseStore((s) => s.status !== null);

  const { data: branches } = useQuery({
    queryKey: queryKeys.branches(repoId),
    queryFn: () => listBranches(repoId),
  });
  const localBranchNames = new Set((branches ?? []).filter((b) => !b.isRemote).map((b) => b.name));
  // Something to push: never pushed to origin, or commits origin doesn't have yet.
  const hasCommitsToPush = (name: string) => {
    const local = branches?.find((b) => !b.isRemote && b.name === name);
    return !!local && (local.ahead === null || local.ahead > 0);
  };
  const { push: pushToOrigin } = usePushBranch(repoId);

  const invalidate = () => {
    queryClient.invalidateQueries({ queryKey: queryKeys.branches(repoId) });
    queryClient.invalidateQueries({ queryKey: queryKeys.commitLog(repoId) });
    queryClient.invalidateQueries({ queryKey: queryKeys.headCommit(repoId) });
    queryClient.invalidateQueries({ queryKey: queryKeys.repoStatus(repoId) });
  };

  const checkout = useMutation({
    mutationFn: (name: string) => checkoutBranch(repoId, name),
    onSuccess: invalidate,
  });

  const checkoutAtCommit = useMutation({
    mutationFn: (vars: { name: string; targetSha: string; remoteName: string | null }) =>
      checkoutAt(repoId, vars.name, vars.targetSha, vars.remoteName),
    onSuccess: invalidate,
  });

  // Wrapped with toast.promise for a visible loading/success/error animation
  // during the branch switch, instead of the raw `.mutate()` failing silently.
  const checkoutWithFeedback = (name: string) =>
    toast.promise(checkout.mutateAsync(name), {
      loading: t("gitBranches.switchingTo", { name }),
      success: t("gitBranches.currentBranch", { name }),
      error: (e) => errorMessage(e),
    });

  const checkoutAtWithFeedback = (vars: { name: string; targetSha: string; remoteName: string | null }) =>
    toast.promise(checkoutAtCommit.mutateAsync(vars), {
      loading: t("gitBranches.switchingTo", { name: vars.name }),
      success: t("gitBranches.currentBranch", { name: vars.name }),
      error: (e) => errorMessage(e),
    });

  const create = useMutation({
    mutationFn: (vars: { name: string; startPoint: string | null }) =>
      createBranch(repoId, vars.name, vars.startPoint, true),
    onSuccess: invalidate,
    onError: (e) => toast.error(errorMessage(e)),
  });

  const refreshTags = () => {
    queryClient.invalidateQueries({ queryKey: queryKeys.tags(repoId) });
    queryClient.invalidateQueries({ queryKey: queryKeys.remoteTags(repoId) });
    queryClient.invalidateQueries({ queryKey: queryKeys.commitLog(repoId) });
  };
  const tagMutation = <V,>(fn: (v: V) => Promise<void>) =>
    // eslint-disable-next-line react-hooks/rules-of-hooks
    useMutation({ mutationFn: fn, onSuccess: refreshTags, onError: (e) => toast.error(errorMessage(e)) });
  const createTagAt = tagMutation((v: { name: string; sha: string }) => createTag(repoId, v.name, v.sha));
  const deleteTagLocal = tagMutation((name: string) => deleteLocalTag(repoId, name));
  const pushTagToRemote = tagMutation((v: { name: string; remote: string }) => pushTag(repoId, v.remote, v.name));
  const deleteTagOnRemote = tagMutation((v: { name: string; remote: string }) =>
    deleteRemoteTag(repoId, v.remote, v.name),
  );

  const rebaseOnto = useMutation({
    mutationFn: (onto: string) => startRebase(repoId, onto),
    onSuccess: (status) => {
      if (status.state === "Complete") invalidate();
      else setRebaseStatus(status);
    },
    onError: (e) => toast.error(errorMessage(e)),
  });

  const checkoutDetached = useMutation({
    mutationFn: (sha: string) => checkoutCommitDetached(repoId, sha),
    onSuccess: invalidate,
  });

  const checkoutDetachedWithFeedback = (sha: string) =>
    toast.promise(checkoutDetached.mutateAsync(sha), {
      loading: t("gitBranches.checkingOut", { sha: sha.slice(0, 7) }),
      success: t("gitBranches.detachedAt", { sha: sha.slice(0, 7) }),
      error: (e) => errorMessage(e),
    });

  const cherryPickCommit = useMutation({
    mutationFn: (vars: { sha: string; createCommit: boolean }) =>
      cherryPick(repoId, vars.sha, vars.createCommit),
    onSuccess: (result) => {
      invalidate();
      if (result.conflicts.length > 0) {
        toast.error(
          t(result.conflicts.length === 1 ? "gitBranches.cherryPickConflictsOne" : "gitBranches.cherryPickConflictsOther", {
            count: result.conflicts.length,
            files: result.conflicts.map((c) => c.path).join(", "),
          }),
        );
      }
    },
    onError: (e) => toast.error(errorMessage(e)),
  });

  const cherryPickWithFeedback = (vars: { sha: string; createCommit: boolean }) =>
    toast.promise(cherryPickCommit.mutateAsync(vars), {
      loading: t("gitBranches.cherryPicking", { sha: vars.sha.slice(0, 7) }),
      success: (result) =>
        result.conflicts.length > 0
          ? t("gitBranches.cherryPickConflicted", { sha: vars.sha.slice(0, 7) })
          : t("gitBranches.cherryPickApplied", { sha: vars.sha.slice(0, 7) }),
      error: (e) => errorMessage(e),
    });

  // Cherry-picking a range only makes sense as commits (there's no sane
  // "changes only" for more than one), so it always creates a commit per
  // pick, oldest to newest, and stops at the first conflict — the rest are
  // left un-picked for the user to retry once that one's resolved.
  const cherryPickManyWithFeedback = async (shas: string[]) => {
    const toastId = "cherry-pick-many";
    toast.loading(t("gitBranches.cherryPickingMany", { count: shas.length }), { id: toastId });
    for (let i = 0; i < shas.length; i++) {
      const sha = shas[i];
      try {
        const result = await cherryPickCommit.mutateAsync({ sha, createCommit: true });
        if (result.conflicts.length > 0) {
          toast.error(
            t("gitBranches.cherryPickStopped", {
              index: i + 1,
              total: shas.length,
              sha: sha.slice(0, 7),
              files: result.conflicts.map((c) => c.path).join(", "),
            }),
            { id: toastId },
          );
          return;
        }
      } catch (e) {
        toast.error(errorMessage(e), { id: toastId });
        return;
      }
    }
    toast.success(t("gitBranches.cherryPickManyDone", { count: shas.length }), { id: toastId });
  };

  const resetTo = useMutation({
    mutationFn: (vars: { sha: string; hard: boolean }) => resetToCommit(repoId, vars.sha, vars.hard),
    onSuccess: () => {
      invalidate();
      queryClient.invalidateQueries({ queryKey: queryKeys.workingStatus(repoId) });
      queryClient.invalidateQueries({ queryKey: ["workdirDiff", repoId] });
    },
    onError: (e) => toast.error(errorMessage(e)),
  });

  const resetToWithFeedback = (vars: { sha: string; hard: boolean }) =>
    toast.promise(resetTo.mutateAsync(vars), {
      loading: t("gitBranches.resetting", { sha: vars.sha.slice(0, 7) }),
      success: t("gitBranches.resetDone", { sha: vars.sha.slice(0, 7) }),
      error: (e) => errorMessage(e),
    });

  // "Sync current branch to remote": autostash uncommitted changes, hard
  // reset to the remote's commit, re-apply the stash. A clean re-apply just
  // invalidates like any other checkout; a conflicted one hands off to the
  // same conflict-resolution UI rebase conflicts use (conflicts are
  // conflicts regardless of what produced them).
  const syncToRemote = useMutation({
    mutationFn: (targetSha: string) => syncBranchToRemote(repoId, targetSha),
    onSuccess: async (result) => {
      invalidate();
      if (result.conflicts.length === 0) return;
      const status = await getRebaseStatus(repoId);
      setRebaseStatus(status);
    },
    onError: (e) => toast.error(errorMessage(e)),
  });

  const syncToRemoteWithFeedback = (targetSha: string) =>
    toast.promise(syncToRemote.mutateAsync(targetSha), {
      loading: t("gitBranches.syncing"),
      success: (r) =>
        r.conflicts.length > 0
          ? t(r.conflicts.length === 1 ? "gitBranches.syncConflictsOne" : "gitBranches.syncConflictsOther", { count: r.conflicts.length })
          : t("gitBranches.synced"),
      error: (e) => errorMessage(e),
    });

  // Merges `origin` into `target`, checking `target` out first if it isn't
  // already current (a harmless no-op checkout when it already is). Always
  // creates a merge commit (never fast-forwards) named "Merge branch
  // '<origin>' into '<target>'". Conflicts hand off to the same
  // conflict-resolution UI rebase conflicts use (conflicts are conflicts
  // regardless of what produced them).
  const mergeInto = useMutation({
    mutationFn: async (vars: { origin: string; target: string }) => {
      await checkoutBranch(repoId, vars.target);
      return mergeBranch(repoId, vars.origin);
    },
    onSuccess: async (result) => {
      invalidate();
      if (result.conflicts.length === 0) return;
      const status = await getRebaseStatus(repoId);
      setRebaseStatus(status);
    },
    onError: (e) => toast.error(errorMessage(e)),
  });

  const mergeIntoWithFeedback = (vars: { origin: string; target: string }) =>
    toast.promise(mergeInto.mutateAsync(vars), {
      loading: t("gitBranches.merging", vars),
      success: (r) =>
        r.conflicts.length > 0
          ? t(r.conflicts.length === 1 ? "gitBranches.mergeConflictsOne" : "gitBranches.mergeConflictsOther", { ...vars, count: r.conflicts.length })
          : t("gitBranches.mergeDone", vars),
      error: (e) => errorMessage(e),
    });

  // Checks `target` out (no-op if already current) then hard-resets it to
  // `origin`'s tip — for "make this branch match that one" drag flows.
  // Destructive (any commits on target not reachable from origin are left
  // behind), so callers should confirm before calling the *WithConfirm form.
  const resetBranchTo = useMutation({
    mutationFn: async (vars: { origin: string; target: string }) => {
      await checkoutBranch(repoId, vars.target);
      return resetToRef(repoId, vars.origin, true);
    },
    onSuccess: () => {
      invalidate();
      queryClient.invalidateQueries({ queryKey: queryKeys.workingStatus(repoId) });
      queryClient.invalidateQueries({ queryKey: ["workdirDiff", repoId] });
    },
    onError: (e) => toast.error(errorMessage(e)),
  });

  const resetBranchToWithConfirm = async (vars: { origin: string; target: string }) => {
    const confirmed = await confirm(
      t("gitBranches.resetBranchConfirm", vars),
      { title: t("gitBranches.resetBranchTitle"), kind: "warning" },
    );
    if (!confirmed) return;
    toast.promise(resetBranchTo.mutateAsync(vars), {
      loading: t("gitBranches.resettingBranch", vars),
      success: t("gitBranches.resetBranchDone", vars),
      error: (e) => errorMessage(e),
    });
  };

  const deleteLocal = useMutation({
    mutationFn: (name: string) => deleteLocalBranch(repoId, name),
    onSuccess: invalidate,
    onError: (e) => toast.error(errorMessage(e)),
  });
  const deleteRemote = useMutation({
    mutationFn: (v: { name: string; remote: string }) => deleteRemoteBranch(repoId, v.remote, v.name),
    onSuccess: invalidate,
    onError: (e) => toast.error(errorMessage(e)),
  });
  const deleteBoth = useMutation({
    mutationFn: (v: { name: string; remotes: string[] }) => deleteBranchBoth(repoId, v.remotes, v.name),
    onSuccess: invalidate,
    onError: (e) => toast.error(errorMessage(e)),
  });

  const deleteLocalWithConfirm = async (name: string) => {
    const confirmed = await confirm(t("gitBranches.deleteLocalConfirm", { name }), {
      title: t("gitBranches.deleteBranchTitle"),
      kind: "warning",
    });
    if (confirmed) deleteLocal.mutate(name);
  };
  const deleteRemoteWithConfirm = async (name: string, remote: string) => {
    const confirmed = await confirm(t("gitBranches.deleteRemoteConfirm", { name, remote }), {
      title: t("gitBranches.deleteRemoteTitle"),
      kind: "warning",
    });
    if (confirmed) deleteRemote.mutate({ name, remote });
  };
  const deleteBothWithConfirm = async (name: string, remotes: string[]) => {
    const confirmed = await confirm(
      t("gitBranches.deleteBothConfirm", { name, remotes: remotes.join(", ") }),
      { title: t("gitBranches.deleteBranchTitle"), kind: "warning" },
    );
    if (confirmed) deleteBoth.mutate({ name, remotes });
  };

  const compareBranches = (a: string, b: string) => openCompare(repoId, a, b);

  return {
    hasCommitsToPush,
    pushToOrigin,
    compareBranches,
    checkout,
    checkoutAtCommit,
    checkoutWithFeedback,
    checkoutAtWithFeedback,
    create,
    createTagAt,
    deleteTagLocal,
    pushTagToRemote,
    deleteTagOnRemote,
    rebaseOnto,
    rebaseInProgress,
    localBranchNames,
    checkoutDetached,
    checkoutDetachedWithFeedback,
    cherryPickCommit,
    cherryPickWithFeedback,
    cherryPickManyWithFeedback,
    resetTo,
    resetToWithFeedback,
    syncToRemote,
    syncToRemoteWithFeedback,
    mergeInto,
    mergeIntoWithFeedback,
    resetBranchTo,
    resetBranchToWithConfirm,
    deleteLocal,
    deleteRemote,
    deleteBoth,
    deleteLocalWithConfirm,
    deleteRemoteWithConfirm,
    deleteBothWithConfirm,
  };
}
