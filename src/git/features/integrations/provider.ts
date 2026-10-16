import type { HostingInstance, HostingProvider } from "./types";

export type ParsedRemote = { host: string; path: string };

/** Parses a git remote URL into `{ host, path }`, handling both the scp-like
 * SSH form (`git@host:owner/repo.git`) and URL forms
 * (`https://host/owner/repo.git`, `ssh://git@host:port/owner/repo.git`).
 * `path` is the project path with any `.git` suffix and leading/trailing
 * slashes stripped (e.g. `group/subgroup/repo`). Returns `null` for anything
 * that doesn't match a recognizable remote URL shape. */
export function parseRemoteUrl(url: string): ParsedRemote | null {
  const trimmed = url.trim();
  if (!trimmed) return null;

  const schemeMatch = trimmed.match(/^[a-z][a-z0-9+.-]*:\/\/(?:[^@/]+@)?([^/:]+)(?::\d+)?\/(.+)$/i);
  if (schemeMatch) {
    const [, host, path] = schemeMatch;
    return { host, path: stripProjectPath(path) };
  }

  const scpMatch = trimmed.match(/^(?:[^@/]+@)?([^:/]+):(.+)$/);
  if (scpMatch) {
    const [, host, path] = scpMatch;
    return { host, path: stripProjectPath(path) };
  }

  return null;
}

function stripProjectPath(path: string): string {
  return path.replace(/\.git$/, "").replace(/^\/+/, "").replace(/\/+$/, "");
}

/** Finds the configured instance whose base URL hostname matches the repo's
 * remote URL, or `null` if none does (including when the remote URL can't be
 * parsed at all) — the caller uses `null` to hide hosting-specific UI. */
export function matchInstance(remoteUrl: string, instances: HostingInstance[]): HostingInstance | null {
  const parsed = parseRemoteUrl(remoteUrl);
  if (!parsed) return null;
  const remoteHost = parsed.host.toLowerCase();
  return (
    instances.find((instance) => {
      try {
        return new URL(instance.baseUrl).hostname.toLowerCase() === remoteHost;
      } catch {
        return false;
      }
    }) ?? null
  );
}

/** GitLab's "new personal access token" page, prefilled with a name and the
 * `api` scope (creating merge requests needs more than `read_api`). */
export function gitLabTokenUrl(baseUrl: string): string | null {
  try {
    const origin = new URL(baseUrl).origin;
    return `${origin}/-/user_settings/personal_access_tokens?name=LazyAICode&scopes=api`;
  } catch {
    return null;
  }
}

/** GitHub's "new personal access token (classic)" page, prefilled with a
 * description and the `repo` scope (reading and opening pull requests). */
export function gitHubTokenUrl(baseUrl: string): string | null {
  try {
    const origin = new URL(baseUrl).origin;
    return `${origin}/settings/tokens/new?description=LazyAICode&scopes=repo`;
  } catch {
    return null;
  }
}

export const tokenUrl = (provider: HostingProvider, baseUrl: string) =>
  provider === "github" ? gitHubTokenUrl(baseUrl) : gitLabTokenUrl(baseUrl);

/** Each service's own words for the same things. */
export const HOSTING_LABELS = {
  gitlab: { name: "GitLab", request: "merge request", short: "MR", refPrefix: "!" },
  github: { name: "GitHub", request: "pull request", short: "PR", refPrefix: "#" },
} as const satisfies Record<HostingProvider, { name: string; request: string; short: string; refPrefix: string }>;
