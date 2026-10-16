import { useRemotes } from "@/features/remote-ops/useRemotes";
import { matchInstance, parseRemoteUrl } from "./provider";
import type { RemoteHosting } from "./types";
import { useIntegrationsSettings } from "./useIntegrationsSettings";

/** Resolves the repo's remotes to a configured GitLab instance or GitHub
 * account: the first remote (origin first) whose host matches. `null` means
 * no remote matches anything configured — callers use that to hide
 * hosting-specific UI. Reads the cached remote list, so it costs no extra call. */
export function useHostingRemote(repoId: string): RemoteHosting | null {
  const { settings } = useIntegrationsSettings();
  const { data: remotes } = useRemotes(repoId);

  for (const remote of remotes ?? []) {
    const remoteUrl = remote.fetchUrl;
    if (!remoteUrl) continue;
    const projectPath = parseRemoteUrl(remoteUrl)?.path;
    if (!projectPath) continue;
    const gitlab = matchInstance(remoteUrl, settings.gitlab.instances);
    if (gitlab) return { provider: "gitlab", instance: gitlab, projectPath };
    const github = matchInstance(remoteUrl, settings.github.instances);
    if (github) return { provider: "github", instance: github, projectPath };
  }
  return null;
}
