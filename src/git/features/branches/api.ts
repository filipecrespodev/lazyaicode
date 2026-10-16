import type { RepoStatus } from "@/features/repository/api";
import type { StashPopResult } from "@/features/stash/types";
import { call } from "@/lib/tauri-client";
import type { BranchInfo, CherryPickResult, MergeResult, TagInfo } from "./types";

export const listBranches = (repoId: string) =>
  call<BranchInfo[]>("list_branches", { repoId });

export const listTags = (repoId: string) => call<TagInfo[]>("list_tags", { repoId });

export const createBranch = (
  repoId: string,
  name: string,
  startPoint: string | null,
  checkout: boolean,
) => call<BranchInfo>("create_branch", { repoId, name, startPoint, checkout });

export const checkoutBranch = (repoId: string, name: string) =>
  call<void>("checkout_branch", { repoId, name });

export const checkoutAt = (
  repoId: string,
  name: string,
  targetSha: string,
  remoteName: string | null,
) => call<void>("checkout_at", { repoId, name, targetSha, remoteName });

export const checkoutCommitDetached = (repoId: string, sha: string) =>
  call<void>("checkout_commit_detached", { repoId, sha });

export const cherryPick = (repoId: string, sha: string, createCommit: boolean) =>
  call<CherryPickResult>("cherry_pick", { repoId, sha, createCommit });

export const resetToCommit = (repoId: string, sha: string, hard: boolean) =>
  call<RepoStatus>("reset_to_commit", { repoId, sha, hard });

export const resetToRef = (repoId: string, refName: string, hard: boolean) =>
  call<RepoStatus>("reset_to_ref", { repoId, refName, hard });

export const syncBranchToRemote = (repoId: string, targetSha: string) =>
  call<StashPopResult>("sync_branch_to_remote", { repoId, targetSha });

export const deleteLocalBranch = (repoId: string, name: string) =>
  call<void>("delete_local_branch", { repoId, name });

export const deleteRemoteBranch = (repoId: string, remoteName: string, name: string) =>
  call<void>("delete_remote_branch", { repoId, remoteName, name });

export const deleteBranchBoth = (repoId: string, remoteNames: string[], name: string) =>
  call<void>("delete_branch_both", { repoId, remoteNames, name });

export const mergeBranch = (repoId: string, name: string) =>
  call<MergeResult>("merge_branch", { repoId, name });

export const createTag = (repoId: string, name: string, sha: string) =>
  call<void>("create_tag", { repoId, name, sha });

export const deleteLocalTag = (repoId: string, name: string) =>
  call<void>("delete_local_tag", { repoId, name });

export const pushTag = (repoId: string, remoteName: string, name: string) =>
  call<void>("push_tag", { repoId, remoteName, name });

export const deleteRemoteTag = (repoId: string, remoteName: string, name: string) =>
  call<void>("delete_remote_tag", { repoId, remoteName, name });

export const listRemoteTags = (repoId: string, remoteName: string) =>
  call<string[]>("list_remote_tags", { repoId, remoteName });
