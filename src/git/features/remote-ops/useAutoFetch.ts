import { type QueryClient, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import { openedRepos } from "@/embed";
import { getConfig, setConfig } from "@/lib/config-store";
import { queryKeys } from "@/lib/query-keys";
import { fetchRemote, isFetching, lastFetchOf } from "./api";
import { DEFAULT_AUTO_FETCH_MINUTES, dueForAutoFetch, fetchMovedRefs, groupSharedRepos } from "./remotes";
import type { FetchSummary, RemoteInfo } from "./types";

const SETTINGS_KEY = ["autoFetchMinutes"] as const;
// Cheap wake-ups: a tick only compares timestamps unless a repo is due.
const TICK_MS = 30_000;
// Gap between repos' fetches, so a dozen open tabs don't hit the network at once.
const STAGGER_MS = 3_000;

/** Minutes between periodic background fetches of open repos; 0 = off. */
export function useAutoFetchSettings() {
  const queryClient = useQueryClient();
  const { data } = useQuery({
    queryKey: SETTINGS_KEY,
    queryFn: () => getConfig<number | null>("autoFetchMinutes", null),
  });
  return {
    settingsLoaded: data !== undefined,
    minutes: data ?? DEFAULT_AUTO_FETCH_MINUTES,
    setMinutes: (minutes: number) => {
      queryClient.setQueryData(SETTINGS_KEY, minutes);
      setConfig("autoFetchMinutes", minutes);
    },
  };
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** After an on-open / periodic fetch (`summary` null: it failed), reloads the
 * ref-dependent queries only if refs may have moved — most rounds change
 * nothing, and the commit log is the expensive one to reload. */
export function refreshAfterBackgroundFetch(queryClient: QueryClient, repoId: string, summary: FetchSummary | null) {
  // Unknown remote count: assume several, i.e. refresh on failure.
  const remoteCount = queryClient.getQueryData<RemoteInfo[]>(queryKeys.remotes(repoId))?.length ?? 2;
  if (!fetchMovedRefs(summary, remoteCount)) return;
  for (const key of [
    queryKeys.branches(repoId),
    queryKeys.commitLog(repoId),
    queryKeys.headCommit(repoId),
    queryKeys.repoStatus(repoId),
  ]) {
    queryClient.invalidateQueries({ queryKey: key });
  }
}

/** Fetches every remote of the repos opened by the git panels every N minutes, in
 * the background (never prompts), one repo at a time. Paused while the
 * window is hidden; skips repos whose fetch is already running. Mounted once. */
export function useAutoFetch() {
  const queryClient = useQueryClient();
  const { minutes } = useAutoFetchSettings();

  useEffect(() => {
    if (!minutes) return;
    const intervalMs = minutes * 60_000;
    let stopped = false;
    let running = false;

    const tick = async () => {
      if (running || document.hidden) return;
      running = true;
      try {
        // Read at tick time: subscribing would re-render the app on every tab change.
        const repos = [...openedRepos.values()];
        // Worktree tabs of one repo are fetched once, by the first of them.
        const groups = groupSharedRepos(repos);
        const members = (id: string) => groups.get(id) ?? [id];
        const lastOf = (id: string) => Math.max(0, ...members(id).map((m) => lastFetchOf(m) ?? 0)) || undefined;
        const busy = (id: string) => members(id).some(isFetching);
        const due = dueForAutoFetch([...groups.keys()], lastOf, busy, Date.now(), intervalMs);
        for (const [i, repoId] of due.entries()) {
          if (i > 0) await sleep(STAGGER_MS);
          if (stopped || document.hidden) break;
          if (busy(repoId)) continue;
          // Failures (offline, a locked key, no credentials) just wait for the next round.
          const summary = await fetchRemote(repoId, null, { background: true }).catch(() => null);
          for (const id of members(repoId)) refreshAfterBackgroundFetch(queryClient, id, summary);
        }
      } finally {
        running = false;
      }
    };

    const timer = setInterval(tick, TICK_MS);
    // Catch up right away when the window comes back instead of waiting a tick.
    const onVisibility = () => {
      if (!document.hidden) tick();
    };
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      stopped = true;
      clearInterval(timer);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [minutes, queryClient]);
}
