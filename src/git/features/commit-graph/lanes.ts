export const PALETTE_SIZE = 8;

/** Structural — accepts `CommitNode[]` as well as synthetic rows (WIP,
 * stashes) that only need a sha + parent shas to be placed in the graph. */
export type LaneInput = { sha: string; parents: string[] };

export type ParentConnection = {
  toLane: number;
  toRowIndex: number; // -1 when the parent isn't loaded yet (pagination boundary)
  kind: "direct" | "merge";
};

export type GraphRow = {
  sha: string;
  lane: number;
  color: number;
  parentConnections: ParentConnection[];
};

/**
 * Two passes over commits in topological+time order (newest first, same order
 * returned by `get_commit_log`).
 *
 * Pass 1 assigns each row a lane, mirroring the "active lanes" approach of
 * `git log --graph`: a lane holds the sha it expects to see next; when that
 * commit is reached, the lane is reused. This pass only decides lane
 * *packing* (how few columns we need) — it does NOT decide edges, because a
 * commit can have more than one child (a branch point), and by the time we
 * reach that commit only the first child to claim a lane would otherwise get
 * a connection drawn to it.
 *
 * Pass 2 draws one edge per (child, parent) pair using each row's now-known
 * lane, so every child — not just the first one to reach a shared ancestor —
 * gets a line connecting to it in the right column.
 */
export function computeLanes(commits: LaneInput[], pinned: ReadonlySet<string> = new Set()): GraphRow[] {
  const rowIndexBySha = new Map(commits.map((c, i) => [c.sha, i] as const));
  const active: (string | null)[] = [];
  const laneByRow: number[] = [];
  // A pinned branch (its first-parent chain) owns lane 0 outright: its commits
  // always land there and nothing else may take it, even above its tip.
  const firstFree = pinned.size > 0 ? 1 : 0;

  const takeLane = (): number => {
    for (let i = firstFree; i < active.length; i++) if (active[i] === null) return i;
    return Math.max(active.length, firstFree);
  };

  commits.forEach((commit) => {
    let lane: number;
    if (pinned.has(commit.sha)) {
      lane = 0;
    } else {
      lane = active.findIndex((expected, i) => i >= firstFree && expected === commit.sha);
      if (lane === -1) lane = takeLane();
    }
    active[lane] = null;

    // Branches that diverged with no merge commit converge back here too —
    // collapse any other lane still waiting on this same sha so it frees up.
    for (let i = 0; i < active.length; i++) {
      if (active[i] === commit.sha) active[i] = null;
    }
    laneByRow.push(lane);

    commit.parents.forEach((parentSha, i) => {
      if (i === 0) {
        active[lane] = parentSha;
        return;
      }
      let mergeLane = active.findIndex((expected, j) => j >= firstFree && expected === parentSha);
      if (mergeLane === -1 && active[0] === parentSha) mergeLane = 0;
      if (mergeLane === -1) mergeLane = takeLane();
      active[mergeLane] = parentSha;
    });
  });

  return commits.map((commit, rowIndex) => {
    const lane = laneByRow[rowIndex];
    const parentConnections: ParentConnection[] = commit.parents.map((parentSha, i) => {
      const toRowIndex = rowIndexBySha.get(parentSha) ?? -1;
      const toLane = toRowIndex === -1 ? lane : laneByRow[toRowIndex];
      return { toLane, toRowIndex, kind: i === 0 ? ("direct" as const) : ("merge" as const) };
    });
    return { sha: commit.sha, lane, color: lane % PALETTE_SIZE, parentConnections };
  });
}
