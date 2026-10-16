import { call } from "@/lib/tauri-client";
import type { CommitLogPage, CommitNode } from "./types";

export const getCommitLog = (repoId: string, skip: number, limit: number) =>
  call<CommitLogPage>("get_commit_log", {
    repoId,
    query: { skip, limit, branchFilter: null },
  });

export const getHeadCommit = (repoId: string) =>
  call<CommitNode | null>("get_head_commit", { repoId });

export type SearchMode = "message" | "author" | "sha" | "file" | "diff";

export const searchCommits = (repoId: string, query: string, mode: SearchMode) =>
  call<CommitNode[]>("search_commits", { repoId, query, mode });

export const getRepoFingerprint = (repoId: string) => call<string>("get_repo_fingerprint", { repoId });
