export type StashInfo = {
  index: number;
  message: string;
  sha: string;
  baseSha: string;
  /** When the stash was made (unix seconds). */
  date: number;
};

export type StashPopResult = {
  conflicts: { path: string }[];
};
