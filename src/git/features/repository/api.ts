import { call } from "@/lib/tauri-client";

export type RepoSummary = {
  repoId: string;
  path: string;
  isBare: boolean;
  currentBranch: string | null;
  headSha: string | null;
  /** Shared by a repo and its worktrees. */
  commonDir: string;
};

export type RepoStatus = {
  currentBranch: string | null;
  headDetached: boolean;
  dirtyFiles: number;
  stagedFiles: number;
  ahead: number;
  behind: number;
  inProgressOp: "Rebase" | "Merge" | "CherryPick" | "Revert" | null;
};

export const openRepository = (path: string) =>
  call<RepoSummary>("open_repository", { path });

export const getRepoStatus = (repoId: string) =>
  call<RepoStatus>("get_repo_status", { repoId });

export const closeRepository = (repoId: string) =>
  call<void>("close_repository", { repoId });
