import { call } from "@/lib/tauri-client";
import type { CommitDiff } from "./types";

export const getCommitDiff = (repoId: string, sha: string) =>
  call<CommitDiff>("get_commit_diff", { repoId, sha });

export const getRangeDiff = (repoId: string, fromSha: string, toSha: string) =>
  call<CommitDiff>("get_range_diff", { repoId, fromSha, toSha });

export const getFileContent = (repoId: string, sha: string | null, path: string) =>
  call<string>("get_file_content", { repoId, sha, path });
