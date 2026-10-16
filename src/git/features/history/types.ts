import type { CommitNode } from "@/features/commit-graph/types";
import type { FileStatus } from "@/features/diff-viewer/types";
import type { ConflictedFile } from "@/features/rebase/types";

export type RevertResult = {
  /** The revert commit; null when it stopped on conflicts. */
  sha: string | null;
  conflicts: ConflictedFile[];
};

/** Opaque resume point of the walk (its queued commits and the followed name). */
export type FileHistoryCursor = { frontier: string[]; path: string };

export type FileHistoryEntry = {
  commit: CommitNode;
  /** The file's name in this commit. */
  path: string;
  /** Its previous name, when this commit renamed it. */
  oldPath: string | null;
  status: FileStatus;
};

export type FileHistoryPage = {
  entries: FileHistoryEntry[];
  nextCursor: FileHistoryCursor | null;
};

export type BlameHunk = {
  startLine: number;
  lineCount: number;
  sha: string;
  origPath: string | null;
};

export type BlameChunk = {
  sha: string;
  totalLines: number;
  startLine: number;
  lines: string[];
  hunks: BlameHunk[];
  commits: CommitNode[];
};

export type BranchCompare = {
  aSha: string;
  bSha: string;
  mergeBase: string | null;
  onlyInA: CommitNode[];
  onlyInB: CommitNode[];
  onlyInATotal: number;
  onlyInBTotal: number;
};
