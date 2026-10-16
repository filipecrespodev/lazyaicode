import { useQuery, useQueryClient } from "@tanstack/react-query";
import { getConfig, setConfig } from "@/lib/config-store";

const KEY = ["signCommits"] as const;

/** App-wide "sign every commit with the repo's SSH key". Read by the backend
 * (src-tauri/src/git/signing.rs) from the config store on every commit;
 * never written to git config. */
export function useSignCommits() {
  const queryClient = useQueryClient();
  const { data } = useQuery({ queryKey: KEY, queryFn: () => getConfig<boolean>("signCommits", false) });
  return {
    loaded: data !== undefined,
    signCommits: data ?? false,
    setSignCommits: (next: boolean) => {
      queryClient.setQueryData(KEY, next);
      setConfig("signCommits", next);
    },
  };
}
