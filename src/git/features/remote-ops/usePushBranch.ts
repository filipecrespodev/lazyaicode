import { useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { t } from "../../../lib/i18n/react";
import { confirm } from "@/lib/confirm-store";
import { queryKeys } from "@/lib/query-keys";
import type { GitCommandError } from "@/lib/tauri-client";
import { errorMessage } from "@/lib/tauri-client";
import { fetchRemote, pushBranch } from "./api";

/** Push as the top-bar button does it (to the upstream remote, else origin, else the only one): fetch first, then push, and
 * on a non-fast-forward rejection offer a force push. Shared by the Push
 * button and the branch context menus. */
export function usePushBranch(repoId: string) {
  const queryClient = useQueryClient();

  const invalidate = () => {
    queryClient.invalidateQueries({ queryKey: queryKeys.branches(repoId) });
    queryClient.invalidateQueries({ queryKey: queryKeys.commitLog(repoId) });
    queryClient.invalidateQueries({ queryKey: queryKeys.headCommit(repoId) });
    queryClient.invalidateQueries({ queryKey: queryKeys.repoStatus(repoId) });
  };

  const fetchMutation = useMutation({
    mutationFn: () => fetchRemote(repoId, null),
  });
  const pushMutation = useMutation({
    mutationFn: (vars: { branch: string; force: boolean }) => pushBranch(repoId, null, vars.branch, vars.force),
  });

  const push = async (branch: string, force = false, skipFetch = false): Promise<void> => {
    if (!skipFetch) {
      toast.loading(t("gitRemote.fetchingBeforePush"), { id: "push" });
      try {
        await fetchMutation.mutateAsync();
      } catch (e) {
        toast.error(t("gitRemote.fetchBeforePushFailed", { error: errorMessage(e) }), { id: "push" });
        return;
      }
      invalidate();
    }

    toast.loading(t(force ? "gitRemote.forcePushing" : "gitRemote.pushing", { branch }), { id: "push" });
    try {
      const s = await pushMutation.mutateAsync({ branch, force });
      invalidate();
      toast.success(t("gitRemote.pushDone", { branch: s.branch, remote: s.remoteName }), { id: "push" });
    } catch (e) {
      const err = e as GitCommandError;
      if (!force && err?.kind === "PushRejectedNonFastForward") {
        toast.dismiss("push");
        const confirmed = await confirm(
          t("gitRemote.forcePushConfirm"),
          { title: t("gitRemote.forcePushTitle"), kind: "warning" },
        );
        if (confirmed) return push(branch, true, true);
        return;
      }
      toast.error(errorMessage(e), { id: "push" });
    }
  };

  return { push, fetchMutation, pushMutation, invalidate };
}
