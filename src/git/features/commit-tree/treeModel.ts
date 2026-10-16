import { groupRefs } from "@/features/commit-graph/refGroups";
import type { CommitNode, RefInfo } from "@/features/commit-graph/types";

export type TreeNode = {
  /** Commit sha, or "root" for the virtual root joining several roots. */
  id: string;
  commit: CommitNode | null;
  /** Branch/tag names at this commit (local + remote grouped, like the graph's pills). */
  refNames: string[];
  isHead: boolean;
  /** Commits folded into the link coming into this node (a linear run). */
  skipped: number;
  /** In log order (newest first); the layout decides where each one goes. */
  children: TreeNode[];
};

/**
 * Turns the commit log into a tree for the zoomable overview: every commit
 * points to its *first* parent (so a merged branch ends as its own leaf instead
 * of rejoining), and only the commits worth a node are kept — roots, branch
 * points, tips and commits with refs. Linear runs between them are folded into
 * the incoming link and counted in `skipped`.
 */
export function buildCommitTree(commits: CommitNode[], refs: RefInfo[]): TreeNode | null {
  if (commits.length === 0) return null;

  const loaded = new Set(commits.map((c) => c.sha));
  const childrenOf = new Map<string, string[]>();
  const roots: string[] = [];
  for (const c of commits) {
    const parent = c.parents[0];
    if (parent && loaded.has(parent)) childrenOf.set(parent, [...(childrenOf.get(parent) ?? []), c.sha]);
    else roots.push(c.sha);
  }

  const refsAt = new Map<string, RefInfo[]>();
  for (const r of refs) refsAt.set(r.targetSha, [...(refsAt.get(r.targetSha) ?? []), r]);

  const bySha = new Map(commits.map((c) => [c.sha, c]));
  const rootSet = new Set(roots);
  const kept = (sha: string) =>
    rootSet.has(sha) || (childrenOf.get(sha)?.length ?? 0) !== 1 || refsAt.has(sha);

  const toNode = (sha: string, skipped: number): TreeNode => {
    const groups = groupRefs(refsAt.get(sha) ?? []);
    return {
      id: sha,
      commit: bySha.get(sha)!,
      refNames: groups.map((g) => g.name),
      isHead: groups.some((g) => g.isHead),
      skipped,
      children: (childrenOf.get(sha) ?? []).map((child) => {
        // Walk down the linear run until the next commit that keeps a node.
        let cur = child;
        let folded = 0;
        while (!kept(cur)) {
          cur = childrenOf.get(cur)![0];
          folded++;
        }
        return toNode(cur, folded);
      }),
    };
  };

  if (roots.length === 1) return toNode(roots[0], 0);
  return {
    id: "root",
    commit: null,
    refNames: [],
    isHead: false,
    skipped: 0,
    children: roots.map((sha) => toNode(sha, 0)),
  };
}
