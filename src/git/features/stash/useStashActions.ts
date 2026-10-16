import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { t } from "../../../lib/i18n/react";
import { queryKeys } from "@/lib/query-keys";
import { errorMessage } from "@/lib/tauri-client";
import { stashApply, stashDrop, stashList, stashPop, stashSave } from "./api";

export function useStashActions(repoId: string) {
  const queryClient = useQueryClient();

  const { data: stashes } = useQuery({
    queryKey: queryKeys.stashes(repoId),
    queryFn: () => stashList(repoId),
  });

  const invalidate = () => {
    queryClient.invalidateQueries({ queryKey: queryKeys.stashes(repoId) });
    queryClient.invalidateQueries({ queryKey: queryKeys.repoStatus(repoId) });
    // Pop/apply/drop change the working directory, so the staging panel's
    // file list and diff viewer (if a working-directory file is open) need to
    // pick that up immediately instead of waiting for the next poll tick.
    queryClient.invalidateQueries({ queryKey: queryKeys.workingStatus(repoId) });
    queryClient.invalidateQueries({ queryKey: ["workdirDiff", repoId] });
  };

  const reportConflicts = (key: "popConflicts" | "applyConflicts", conflicts: { path: string }[]) => {
    if (conflicts.length > 0) {
      toast.error(t(`gitStash.${key}`, { files: conflicts.map((c) => c.path).join(", ") }));
    }
  };

  const save = useMutation({
    mutationFn: (vars: { message: string | null; includeUntracked: boolean }) =>
      stashSave(repoId, vars.message, vars.includeUntracked),
    onSuccess: invalidate,
    onError: (e) => toast.error(errorMessage(e)),
  });

  const pop = useMutation({
    mutationFn: (index: number) => stashPop(repoId, index),
    onSuccess: (result) => {
      invalidate();
      reportConflicts("popConflicts", result.conflicts);
    },
    onError: (e) => toast.error(errorMessage(e)),
  });

  const apply = useMutation({
    mutationFn: (index: number) => stashApply(repoId, index),
    onSuccess: (result) => {
      invalidate();
      reportConflicts("applyConflicts", result.conflicts);
    },
    onError: (e) => toast.error(errorMessage(e)),
  });

  const drop = useMutation({
    mutationFn: (index: number) => stashDrop(repoId, index),
    onSuccess: invalidate,
    onError: (e) => toast.error(errorMessage(e)),
  });

  return { stashes, save, pop, apply, drop };
}
