import type { CommitDiff, FileDiff } from "@/features/diff-viewer/types";
import { call } from "@/lib/tauri-client";
import type { BlameChunk, BranchCompare, FileHistoryCursor, FileHistoryPage, RevertResult } from "./types";

export const revertCommit = (repoId: string, sha: string, mainline: number | null) =>
  call<RevertResult>("revert_commit", { repoId, sha, mainline });

export const getFileHistory = (
  repoId: string,
  path: string,
  startSha: string,
  cursor: FileHistoryCursor | null,
  limit: number,
) => call<FileHistoryPage>("get_file_history", { repoId, query: { path, startSha, cursor, limit } });

export const getCommitFileDiff = (repoId: string, sha: string, path: string, oldPath: string | null) =>
  call<FileDiff | null>("get_commit_file_diff", { repoId, sha, path, oldPath });

export const getBlame = (repoId: string, path: string, sha: string, startLine: number, maxLines: number) =>
  call<BlameChunk>("get_blame", { repoId, path, sha, startLine, maxLines });

export const getBranchCompare = (repoId: string, a: string, b: string, limit: number) =>
  call<BranchCompare>("get_branch_compare", { repoId, a, b, limit });

export const getTreeDiff = (repoId: string, fromSha: string, toSha: string) =>
  call<CommitDiff>("get_tree_diff", { repoId, fromSha, toSha });
