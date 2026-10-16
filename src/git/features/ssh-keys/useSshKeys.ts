import { useQuery, useQueryClient } from "@tanstack/react-query";
import { getConfig, setConfig } from "@/lib/config-store";
import { call } from "@/lib/tauri-client";
import { wslDistro } from "../../../lib/wsl";
import { normalizeSshKeySettings, setOverride, type SshKeyInfo, type SshKeySettings } from "./settings";

const SETTINGS_KEY = ["sshKeys"] as const;

/** Keys found in ~/.ssh — of the WSL distro `dir` lives in, else of this machine. */
export function useSshKeyList(dir?: string) {
  const distro = wslDistro(dir);
  return (
    useQuery({
      queryKey: ["sshKeyList", distro],
      queryFn: () => call<SshKeyInfo[]>("list_ssh_keys", { distro }),
    }).data ?? []
  );
}

/** Which SSH key to use, globally and per workspace/repo. Read by the backend
 * straight from the config store on every connection. */
export function useSshKeySettings() {
  const queryClient = useQueryClient();
  const { data } = useQuery({
    queryKey: SETTINGS_KEY,
    queryFn: () => getConfig<SshKeySettings | null>("sshKeys", null),
  });
  const settings = normalizeSshKeySettings(data);

  // Like useProfiles: cache first so the UI follows at once, disk in the background.
  const persist = (next: SshKeySettings) => {
    queryClient.setQueryData(SETTINGS_KEY, next);
    setConfig("sshKeys", next);
  };

  return {
    settingsLoaded: data !== undefined,
    settings,
    setDefaultKey: (key: string | null) => persist({ ...settings, defaultKey: key }),
    setOverride: (dir: string, key: string | null) => persist(setOverride(settings, dir, key)),
  };
}
