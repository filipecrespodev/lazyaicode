import { useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { queryKeys } from "@/lib/query-keys";
import { errorMessage } from "@/lib/tauri-client";
import { abortRebase, continueRebase as continueRebaseApi, resolveRebaseConflict } from "./api";
import { useRebaseStore } from "./store";
import type { ConflictResolution } from "./types";

/** Shared rebase-conflict lifecycle actions, used by both the sidebar's quick
 * resolve buttons and the two-pane merge editor's Save button, so "resolving
 * the last conflict auto-continues the rebase" isn't implemented twice. */
export function useResolveConflict(repoId: string) {
  const setStatus = useRebaseStore((s) => s.setStatus);
  const queryClient = useQueryClient();

  const invalidateAll = () => {
    queryClient.invalidateQueries({ queryKey: queryKeys.commitLog(repoId) });
    queryClient.invalidateQueries({ queryKey: queryKeys.headCommit(repoId) });
    queryClient.invalidateQueries({ queryKey: queryKeys.branches(repoId) });
    queryClient.invalidateQueries({ queryKey: queryKeys.repoStatus(repoId) });
    queryClient.invalidateQueries({ queryKey: queryKeys.workingStatus(repoId) });
    queryClient.invalidateQueries({ queryKey: queryKeys.undoStatus(repoId) });
  };

  const continueRebase = async () => {
    try {
      const next = await continueRebaseApi(repoId);
      if (next.state === "Complete") {
        setStatus(null);
        invalidateAll();
      } else {
        setStatus(next);
      }
    } catch (e) {
      const message = errorMessage(e);
      // Backend has no session to continue (e.g. lost after a crash/race) — nothing to
      // show a stuck panel for, dismiss instead of leaving the user with no way out.
      if (message.includes("no rebase in progress")) {
        setStatus(null);
        invalidateAll();
      } else {
        toast.error(message);
      }
    }
  };

  const resolve = async (path: string, resolution: ConflictResolution): Promise<boolean> => {
    try {
      const next = await resolveRebaseConflict(repoId, path, resolution);
      if (next.conflicts.length === 0) {
        // Last conflict of this step just got resolved — auto-advance instead
        // of waiting for a manual click; the rebase only pauses for conflicts.
        await continueRebase();
      } else {
        setStatus(next);
      }
      return true;
    } catch (e) {
      toast.error(errorMessage(e));
      return false;
    }
  };

  const abort = async () => {
    try {
      await abortRebase(repoId);
    } catch (e) {
      toast.error(errorMessage(e));
    }
    setStatus(null);
    invalidateAll();
  };

  return { resolve, continueRebase, abort };
}
