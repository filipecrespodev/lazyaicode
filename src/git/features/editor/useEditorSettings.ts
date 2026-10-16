import { useQuery, useQueryClient } from "@tanstack/react-query";
import { getConfig, setConfig } from "@/lib/config-store";
import { listInstalledEditors } from "./api";

const KEY = ["defaultEditor"] as const;

/** Chosen editor id (from list_installed_editors), or null for the system default. */
export function useDefaultEditor() {
  const queryClient = useQueryClient();
  const { data } = useQuery({ queryKey: KEY, queryFn: () => getConfig<string | null>("defaultEditor", null) });
  return {
    loaded: data !== undefined,
    editorId: data ?? null,
    setEditorId: (next: string | null) => {
      queryClient.setQueryData(KEY, next);
      setConfig("defaultEditor", next);
    },
  };
}

export const useInstalledEditors = () =>
  useQuery({ queryKey: ["installedEditors"], queryFn: listInstalledEditors, staleTime: 60_000 });
