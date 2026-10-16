import { useQuery } from "@tanstack/react-query";
import { defaultRemote } from "@/features/remote-ops/remotes";
import { useRemotes } from "@/features/remote-ops/useRemotes";
import { queryKeys } from "@/lib/query-keys";
import { listRemoteTags } from "./api";

/** Tag names on the default remote (origin, else the first) — one network
 * call per repo, shared by every tag row/pill; failures just mean "unknown"
 * (undefined list → []). Other remotes aren't queried, to keep it cheap. */
export function useRemoteTags(repoId: string, enabled: boolean) {
  const remote = defaultRemote((useRemotes(repoId).data ?? []).map((r) => r.name));
  const tags = useQuery({
    queryKey: [...queryKeys.remoteTags(repoId), remote],
    queryFn: () => listRemoteTags(repoId, remote as string).catch(() => [] as string[]),
    enabled: enabled && remote !== null,
  }).data;
  return { remote, tags };
}
