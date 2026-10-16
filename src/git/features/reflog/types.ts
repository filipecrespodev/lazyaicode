export type ReflogEntry = {
  /** `HEAD@{3}` style selector. */
  selector: string;
  newSha: string;
  /** All zeros when the ref was created by this entry. */
  oldSha: string;
  committerName: string;
  committerEmail: string;
  /** Unix seconds. */
  time: number;
  /** e.g. "commit: fix login", "checkout: moving from main to dev". */
  message: string;
  /** Null when the commit no longer exists (garbage-collected). */
  commitSummary: string | null;
};
