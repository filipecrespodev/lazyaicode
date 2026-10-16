import { describe, expect, it } from "vitest";
import type { CommitNode } from "@/features/commit-graph/types";
import { combinedMessage, isSquashable, oldestFirst, squashItems } from "./squash";

const c = (sha: string, parent: string | null, extra: Partial<CommitNode> = {}): CommitNode => ({
  sha,
  shortSha: sha,
  summary: `msg ${sha}`,
  body: "",
  authorName: "",
  authorEmail: "",
  authorDate: 0,
  committerDate: 0,
  parents: parent ? [parent] : [],
  isMerge: false,
  ...extra,
});

// newest first: d -> c -> b -> a
const graph = [c("d", "c"), c("c", "b"), c("b", "a"), c("a", null)];

describe("isSquashable", () => {
  it("accepts adjacent commits on a linear chain, in any selection order", () => {
    expect(isSquashable([graph[2], graph[1]], graph)).toBe(true);
    expect(isSquashable([graph[0], graph[1], graph[2]], graph)).toBe(true);
  });
  it("rejects a single commit, gaps and merges", () => {
    expect(isSquashable([graph[0]], graph)).toBe(false);
    expect(isSquashable([graph[0], graph[2]], graph)).toBe(false);
    const withMerge = [c("m", "x", { parents: ["x", "y"], isMerge: true }), c("x", null)];
    expect(isSquashable(withMerge, withMerge)).toBe(false);
  });
  it("rejects adjacent rows that are not parent and child", () => {
    const g = [c("x", "other"), c("y", null)];
    expect(isSquashable(g, g)).toBe(false);
  });
});

describe("squashItems", () => {
  const plan = [graph[2], graph[1], graph[0]]; // b, c, d (a is the upstream)
  it("rewords the oldest, fixes up the rest and picks the others", () => {
    expect(squashItems(plan, new Set(["b", "c"]), "new")).toEqual([
      { sha: "b", action: "reword", message: "new" },
      { sha: "c", action: "fixup", message: null },
      { sha: "d", action: "pick", message: null },
    ]);
  });
});

describe("messages", () => {
  it("orders oldest first and joins the full messages", () => {
    const sel = oldestFirst([graph[1], graph[2]], graph);
    expect(sel.map((x) => x.sha)).toEqual(["b", "c"]);
    expect(combinedMessage(sel)).toBe("msg b\n\nmsg c");
  });
});
