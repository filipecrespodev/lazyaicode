export type AuthorStats = {
  name: string;
  email: string;
  commits: number;
  /** Unix seconds. */
  lastCommitTime: number;
};

export type RepoStats = {
  totalCommits: number;
  /** Unix seconds; null for an empty repo. */
  firstCommitTime: number | null;
  /** Most commits first. */
  authors: AuthorStats[];
  /** Author times (unix seconds) of the commits of roughly the last year. */
  recentCommitTimes: number[];
};
