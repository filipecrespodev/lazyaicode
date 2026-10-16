import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef } from "react";
import { getRepoFingerprint } from "@/features/commit-graph/api";
import { queryKeys } from "@/lib/query-keys";

// ponytail: fixed 5s cadence, bump if external changes (terminal commits,
// stashes, checkouts) need to show up faster than that.
const POLL_INTERVAL_MS = 5000;

/** Re-read on every tick: the working tree changes without touching refs.
 * The (heavy) workdir diffs are not here: they re-read when the working
 * status' signature changes, or when an action invalidates them. */
export const WORKING_KEYS = (repoId: string) => [queryKeys.repoStatus(repoId), queryKeys.workingStatus(repoId)];
/** Re-read only when the refs/HEAD/stash fingerprint moved (the log refetches every loaded page). */
export const REF_KEYS = (repoId: string) => [
  queryKeys.commitLog(repoId),
  queryKeys.headCommit(repoId),
  queryKeys.branches(repoId),
  queryKeys.stashes(repoId),
];

/** Whether a poll tick must refresh the ref-derived queries: yes unless the
 * fingerprint is known and unchanged (an unreadable fingerprint refreshes, to be safe). */
export function refsChanged(previous: string | null, current: string | null): boolean {
  return previous === null || current === null || previous !== current;
}

export function useRepoPolling(repoId: string, active: boolean) {
  const queryClient = useQueryClient();
  const wasActive = useRef(false);

  useEffect(() => {
    // Background tabs stay mounted (just hidden), and a minimized window
    // keeps its timers: without this, every open tab reloaded its status,
    // log and diffs every 5s. Coming back catches up right away instead.
    if (!active) return;
    const invalidate = (keys: readonly (readonly unknown[])[]) =>
      keys.forEach((queryKey) => queryClient.invalidateQueries({ queryKey }));
    const invalidateAll = () => invalidate([...WORKING_KEYS(repoId), ...REF_KEYS(repoId)]);

    if (wasActive.current) invalidateAll();
    wasActive.current = true;

    // The first tick only records the fingerprint: the queries just loaded.
    let lastFingerprint: string | null = null;
    let first = true;
    let busy = false;
    const tick = async () => {
      if (document.hidden || busy) return;
      busy = true;
      try {
        const fingerprint = await getRepoFingerprint(repoId).catch(() => null);
        invalidate(WORKING_KEYS(repoId));
        if (!first && refsChanged(lastFingerprint, fingerprint)) invalidate(REF_KEYS(repoId));
        first = false;
        lastFingerprint = fingerprint;
      } finally {
        busy = false;
      }
    };
    tick(); // records the starting fingerprint right away
    const interval = setInterval(tick, POLL_INTERVAL_MS);
    window.addEventListener("focus", invalidateAll);
    return () => {
      clearInterval(interval);
      window.removeEventListener("focus", invalidateAll);
    };
  }, [repoId, queryClient, active]);
}
