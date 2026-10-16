import { useQuery } from "@tanstack/react-query";
import { queryKeys } from "@/lib/query-keys";
import { listRemotes } from "./api";

/** The repo's remotes (origin first). They rarely change outside the Remotes
 * dialog, which updates this cache itself, so it isn't polled. */
export function useRemotes(repoId: string) {
  return useQuery({
    queryKey: queryKeys.remotes(repoId),
    queryFn: () => listRemotes(repoId),
    staleTime: 5 * 60_000,
  });
}
