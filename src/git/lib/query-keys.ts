export const queryKeys = {
  commitLog: (repoId: string) => ["commitLog", repoId] as const,
  headCommit: (repoId: string) => ["headCommit", repoId] as const,
  commitDiff: (repoId: string, sha: string) => ["commitDiff", repoId, sha] as const,
  commitSignature: (repoId: string, sha: string) => ["commitSignature", repoId, sha] as const,
  rangeDiff: (repoId: string, fromSha: string, toSha: string) =>
    ["rangeDiff", repoId, fromSha, toSha] as const,
  branches: (repoId: string) => ["branches", repoId] as const,
  // Nested under "branches" on purpose: every invalidation of a repo's
  // branches (fetch, checkout, push…) refreshes its tags too.
  tags: (repoId: string) => ["branches", repoId, "tags"] as const,
  remoteTags: (repoId: string) => ["branches", repoId, "remote-tags"] as const,
  // Ahead/behind of the current branch: moves whenever branches do.
  upstreamStatus: (repoId: string) => ["branches", repoId, "upstream"] as const,
  repoStatus: (repoId: string) => ["repoStatus", repoId] as const,
  stashes: (repoId: string) => ["stashes", repoId] as const,
  workingStatus: (repoId: string) => ["workingStatus", repoId] as const,
  workdirDiff: (repoId: string, staged: boolean) => ["workdirDiff", repoId, staged] as const,
  remotes: (repoId: string) => ["remotes", repoId] as const,
  gitlabOpenMrs: (repoId: string) => ["gitlabOpenMrs", repoId] as const,
  gitlabEvents: (repoId: string) => ["gitlabEvents", repoId] as const,
  // Not under commitLog: it walks the whole history, so it only refreshes
  // when the statistics view is (re)opened, not on every graph poll.
  repoStats: (repoId: string) => ["repoStats", repoId] as const,
  undoStatus: (repoId: string) => ["undoStatus", repoId] as const,
  reflog: (repoId: string, refName: string) => ["reflog", repoId, refName] as const,
  // History views are keyed by resolved commit shas, so their data never
  // goes stale and stays out of the polling invalidations.
  fileHistory: (repoId: string, path: string, startSha: string) =>
    ["fileHistory", repoId, path, startSha] as const,
  commitFileDiff: (repoId: string, sha: string, path: string, oldPath: string | null) =>
    ["commitFileDiff", repoId, sha, path, oldPath] as const,
  blame: (repoId: string, path: string, sha: string) => ["blame", repoId, path, sha] as const,
  branchCompare: (repoId: string, a: string, b: string) => ["branchCompare", repoId, a, b] as const,
  treeDiff: (repoId: string, fromSha: string, toSha: string) => ["treeDiff", repoId, fromSha, toSha] as const,
};
