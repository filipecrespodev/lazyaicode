import { useQuery } from "@tanstack/react-query";
import { queryKeys } from "@/lib/query-keys";
import { listOpenRequests } from "./api";
import type { MergeRequestInfo, RemoteHosting } from "./types";

const POLL_INTERVAL_MS = 60_000;

/** Open merge/pull requests on the repo's hosting service, keyed by source
 * branch name. Deliberately separate from `useRepoPolling`'s 5s local-git
 * invalidation — this hits an external, rate-limit-bound API, so it gets its
 * own slower interval. */
export function useOpenMergeRequests(repoId: string, hosting: RemoteHosting | null) {
  const { data } = useQuery({
    queryKey: queryKeys.gitlabOpenMrs(repoId),
    queryFn: () => listOpenRequests(hosting!),
    enabled: hosting !== null,
    refetchInterval: POLL_INTERVAL_MS,
  });

  const byBranch = new Map<string, MergeRequestInfo>();
  for (const mr of data ?? []) byBranch.set(mr.sourceBranch, mr);
  return byBranch;
}
