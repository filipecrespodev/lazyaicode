import { call } from "@/lib/tauri-client";

export type WorktreeInfo = {
  /** null for the main worktree. */
  name: string | null;
  path: string;
  branch: string | null;
  headSha: string | null;
  isMain: boolean;
  isCurrent: boolean;
  locked: boolean;
  /** Its folder is gone; prune forgets it. */
  prunable: boolean;
};

export type AddWorktreeArgs = {
  path: string;
  branch: string;
  newBranch: boolean;
  startPoint: string | null;
};

export const worktreesKey = (repoId: string) => ["worktrees", repoId] as const;

export const listWorktrees = (repoId: string) => call<WorktreeInfo[]>("list_worktrees", { repoId });

export const addWorktree = (repoId: string, args: AddWorktreeArgs) =>
  call<WorktreeInfo[]>("add_worktree", { repoId, args });

export const removeWorktree = (repoId: string, name: string, force: boolean) =>
  call<WorktreeInfo[]>("remove_worktree", { repoId, name, force });

export const pruneWorktrees = (repoId: string) => call<WorktreeInfo[]>("prune_worktrees", { repoId });
