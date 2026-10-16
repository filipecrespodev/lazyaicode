export type FetchSummary = {
  remoteName: string;
  receivedObjects: number;
  totalObjects: number;
  /** Refs the fetch created or moved; 0 means nothing to refresh. */
  updatedRefs: number;
};

export type PushSummary = {
  remoteName: string;
  branch: string;
};

export type FetchProgressEvent = {
  repoId: string;
  receivedObjects: number;
  totalObjects: number;
  receivedBytes: number;
  /** On-open / periodic fetch: no progress toast. */
  background: boolean;
};

export type RemoteInfo = {
  name: string;
  fetchUrl: string | null;
  /** Only when set apart from the fetch URL. */
  pushUrl: string | null;
};

export type PushProgressEvent = {
  repoId: string;
  current: number;
  total: number;
  bytes: number;
};

/** How a pull integrates the upstream. */
export type PullStrategy = "FfOnly" | "Merge" | "Rebase";

export type PullResult = {
  upstream: string;
  strategy: PullStrategy;
  outcome: "UpToDate" | "FastForwarded" | "Merged" | "Rebased" | "Conflicts";
  /** Upstream commits that weren't on the branch yet. */
  commits: number;
  /** Conflicted paths when `outcome` is "Conflicts". */
  conflicts: string[];
  /** The pull went through but re-applying the stashed local changes conflicted. */
  autostashConflicts: string[];
};

/** The checked-out branch vs. its upstream, as of the last fetch. */
export type UpstreamStatus = {
  upstream: string;
  ahead: number;
  behind: number;
  /** What a plain Pull does per the repo's pull config. */
  strategy: PullStrategy;
};
