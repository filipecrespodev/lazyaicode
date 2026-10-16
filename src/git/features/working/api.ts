import type { CommitNode } from "@/features/commit-graph/types";
import type { LineSelection } from "@/features/diff-viewer/hunkSelection";
import type { CommitDiff } from "@/features/diff-viewer/types";
import { call } from "@/lib/tauri-client";
import type { WorkingStatus } from "./types";

export const getWorkingStatus = (repoId: string) =>
  call<WorkingStatus>("get_working_status", { repoId });

export const stagePath = (repoId: string, path: string) => call<void>("stage_path", { repoId, path });

export const stageAll = (repoId: string) => call<void>("stage_all", { repoId });

export const unstagePath = (repoId: string, path: string) =>
  call<void>("unstage_path", { repoId, path });

export const unstageAll = (repoId: string) => call<void>("unstage_all", { repoId });

export const discardPath = (repoId: string, path: string) =>
  call<void>("discard_path", { repoId, path });

export const discardAll = (repoId: string) => call<void>("discard_all", { repoId });

export type LineAction = "stage" | "unstage" | "discard";

/** Stages/unstages/discards only the given changed lines of one file. */
export const applyLines = (repoId: string, path: string, action: LineAction, selection: LineSelection) =>
  call<void>("apply_lines", { repoId, path, action, selection });

/** Resolves to `false` when the pattern was already listed. */
export const addIgnorePattern = (repoId: string, pattern: string, local: boolean) =>
  call<boolean>("add_ignore_pattern", { repoId, pattern, local });

export const getWorkdirDiff = (repoId: string, staged: boolean) =>
  call<CommitDiff>("get_workdir_diff", { repoId, staged });

export const commitChanges = (
  repoId: string,
  summary: string,
  body: string,
  authorName: string | null,
  authorEmail: string | null,
  amend: boolean,
  noVerify = false,
) => call<string>("git_commit_changes", { repoId, summary, body, authorName, authorEmail, amend, noVerify });

export const amendCommitMessage = (repoId: string, sha: string, summary: string, body: string, noVerify = false) =>
  call<CommitNode>("amend_commit_message", { repoId, sha, summary, body, noVerify });
