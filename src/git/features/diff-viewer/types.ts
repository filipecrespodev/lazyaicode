export type FileStatus = "Added" | "Deleted" | "Modified" | "Renamed" | "Copied" | "TypeChange" | "Other";

export type FileDiff = {
  path: string;
  oldPath: string | null;
  status: FileStatus;
  isBinary: boolean;
  patch: string;
};

export type CommitDiff = {
  sha: string;
  parentShas: string[];
  files: FileDiff[];
};
