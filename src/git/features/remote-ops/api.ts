import { call } from "@/lib/tauri-client";
import type { FetchSummary, PullResult, PullStrategy, PushSummary, RemoteInfo, UpstreamStatus } from "./types";

// Every fetch goes through `fetchRemote`, so these say whether one is running
// for a repo and when the last one ended — the periodic auto-fetch reads them
// to never pile a fetch on top of another (user, on-open or periodic).
const inFlight = new Set<string>();
const lastFetchedAt = new Map<string, number>();
export const isFetching = (repoId: string) => inFlight.has(repoId);
export const lastFetchOf = (repoId: string) => lastFetchedAt.get(repoId);

/** `remoteName` null: every remote. `background`: not asked for by the user,
 * so it won't prompt for an SSH key passphrase or HTTPS credentials. */
export const fetchRemote = async (repoId: string, remoteName: string | null, opts?: { background?: boolean }) => {
  inFlight.add(repoId);
  try {
    return await call<FetchSummary>("fetch_remote", { repoId, remoteName, background: opts?.background ?? false });
  } finally {
    inFlight.delete(repoId);
    lastFetchedAt.set(repoId, Date.now());
  }
};

/** `remoteName` null: where `git push` would go; fails with `RemoteRequired` if that's ambiguous. */
export const pushBranch = (repoId: string, remoteName: string | null, branch: string, force: boolean) =>
  call<PushSummary>("push_branch", { repoId, remoteName, branch, force });

export const getRemoteUrl = (repoId: string, remoteName: string) =>
  call<string | null>("get_remote_url", { repoId, remoteName });

export const listRemotes = (repoId: string) => call<RemoteInfo[]>("list_remotes", { repoId });

// Each returns the updated remote list.
export const addRemote = (repoId: string, name: string, url: string) =>
  call<RemoteInfo[]>("add_remote", { repoId, name, url });

export const removeRemote = (repoId: string, name: string) =>
  call<RemoteInfo[]>("remove_remote", { repoId, name });

export const renameRemote = (repoId: string, name: string, newName: string) =>
  call<RemoteInfo[]>("rename_remote", { repoId, name, newName });

export const setRemoteUrl = (repoId: string, name: string, url: string) =>
  call<RemoteInfo[]>("set_remote_url", { repoId, name, url });

/** `strategy`: overrides pull.rebase / pull.ff for this one pull. */
export const pullBranch = (repoId: string, strategy: PullStrategy | null) =>
  call<PullResult>("pull_branch", { repoId, strategy });

export const getUpstreamStatus = (repoId: string) =>
  call<UpstreamStatus | null>("get_upstream_status", { repoId });
