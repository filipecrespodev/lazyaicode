import { useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { getRebaseStatus } from "@/features/rebase/api";
import { useRebaseStore } from "@/features/rebase/store";
import { queryKeys } from "@/lib/query-keys";
import { errorMessage } from "@/lib/tauri-client";
import { revertCommit } from "./api";
import { t } from "../../../lib/i18n/react";

/** "Revert commit": a clean revert commits right away; conflicts hand off to
 * the shared conflict panel (Continue finishes the revert commit, Abort
 * restores HEAD), like merge conflicts do. */
export function useRevertCommit(repoId: string) {
  const queryClient = useQueryClient();
  const setRebaseStatus = useRebaseStore((s) => s.setStatus);

  const revert = useMutation({
    mutationFn: (vars: { sha: string; mainline: number | null }) => revertCommit(repoId, vars.sha, vars.mainline),
    onSuccess: async (result) => {
      for (const key of [
        queryKeys.commitLog(repoId),
        queryKeys.headCommit(repoId),
        queryKeys.branches(repoId),
        queryKeys.repoStatus(repoId),
        queryKeys.workingStatus(repoId),
        ["workdirDiff", repoId],
      ]) {
        queryClient.invalidateQueries({ queryKey: key });
      }
      if (result.conflicts.length > 0) setRebaseStatus(await getRebaseStatus(repoId));
    },
  });

  const revertWithFeedback = (vars: { sha: string; mainline: number | null }) =>
    toast.promise(revert.mutateAsync(vars), {
      loading: t("gitHistory.reverting", { sha: vars.sha.slice(0, 7) }),
      success: (r) =>
        r.conflicts.length > 0
          ? t(r.conflicts.length === 1 ? "gitHistory.revertConflictsOne" : "gitHistory.revertConflictsOther", {
                sha: vars.sha.slice(0, 7),
                count: r.conflicts.length,
              })
          : t("gitHistory.reverted", { sha: vars.sha.slice(0, 7) }),
      error: (e) => errorMessage(e),
    });

  return { revert, revertWithFeedback };
}
