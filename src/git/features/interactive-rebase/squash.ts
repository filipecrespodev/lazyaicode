import type { CommitNode } from "@/features/commit-graph/types";
import { fullMessage, type TodoItem } from "./todo";

/** True when `selected` is 2+ commits that sit next to each other in `graph`
 * (newest first) on one linear first-parent chain, none of them a merge. */
export function isSquashable(selected: CommitNode[], graph: CommitNode[]): boolean {
  if (selected.length < 2) return false;
  const shas = new Set(selected.map((c) => c.sha));
  const idx = graph.map((c, i) => (shas.has(c.sha) ? i : -1)).filter((i) => i >= 0);
  if (idx.length !== selected.length) return false; // some selected commit isn't loaded
  for (let k = 0; k < idx.length; k++) {
    const c = graph[idx[k]];
    if (c.isMerge) return false;
    if (k > 0 && (idx[k] !== idx[k - 1] + 1 || graph[idx[k - 1]].parents[0] !== c.sha)) return false;
  }
  return true;
}

/** Oldest-first selection from any order, per `graph` (newest first). */
export function oldestFirst(selected: CommitNode[], graph: CommitNode[]): CommitNode[] {
  const pos = new Map(graph.map((c, i) => [c.sha, i] as const));
  return [...selected].sort((a, b) => (pos.get(b.sha) ?? 0) - (pos.get(a.sha) ?? 0));
}

/** The messages joined oldest first, the way `git rebase` offers when squashing. */
export function combinedMessage(oldestFirstCommits: CommitNode[]): string {
  return oldestFirstCommits.map(fullMessage).join("\n\n");
}

/** Todo for squashing `selectedShas` (oldest of them first in `plan`) into one
 * commit with `message`: the oldest is reworded, the rest fixed up into it,
 * every other commit of the plan is picked as is. */
export function squashItems(plan: CommitNode[], selectedShas: Set<string>, message: string): TodoItem[] {
  const oldest = plan.find((c) => selectedShas.has(c.sha))?.sha;
  return plan.map((c) => ({
    sha: c.sha,
    action: c.sha === oldest ? "reword" : selectedShas.has(c.sha) ? "fixup" : "pick",
    message: c.sha === oldest ? message : null,
  }));
}
