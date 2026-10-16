import { describe, expect, it } from "vitest";
import type { CommitNode, RefInfo } from "@/features/commit-graph/types";
import { buildCommitTree, type TreeNode } from "./treeModel";

function commit(sha: string, parents: string[]): CommitNode {
  return {
    sha,
    shortSha: sha.slice(0, 7),
    summary: sha,
    body: "",
    authorName: "test",
    authorEmail: "test@example.com",
    authorDate: 0,
    committerDate: 0,
    parents,
    isMerge: parents.length > 1,
  };
}

function ref(name: string, targetSha: string, kind: RefInfo["kind"] = "LocalBranch", isHead = false): RefInfo {
  return { name, kind, targetSha, isHead };
}

/** Compact "id(+skipped)[children]" rendering, to assert on tree shape. */
function shape(node: TreeNode): string {
  const self = `${node.id}${node.skipped ? `+${node.skipped}` : ""}`;
  return node.children.length ? `${self}[${node.children.map(shape).join(",")}]` : self;
}

describe("buildCommitTree", () => {
  it("returns null for an empty history", () => {
    expect(buildCommitTree([], [])).toBeNull();
  });

  it("compresses a linear history into root -> tip, counting skipped commits", () => {
    const tree = buildCommitTree(
      [commit("c4", ["c3"]), commit("c3", ["c2"]), commit("c2", ["c1"]), commit("c1", [])],
      [ref("main", "c4")],
    );
    expect(shape(tree!)).toBe("c1[c4+2]");
    expect(tree!.children[0].refNames).toEqual(["main"]);
  });

  it("keeps branch points and both branch tips", () => {
    // base -> m1 (main) and base -> f1 -> f2 (feature)
    const tree = buildCommitTree(
      [commit("f2", ["f1"]), commit("m1", ["base"]), commit("f1", ["base"]), commit("base", [])],
      [ref("main", "m1"), ref("feature", "f2")],
    );
    expect(shape(tree!)).toBe("base[m1,f2+1]");
  });

  it("keeps a commit with refs even in the middle of a linear run", () => {
    const tree = buildCommitTree(
      [commit("c3", ["c2"]), commit("c2", ["c1"]), commit("c1", [])],
      [ref("v1.0", "c2", "Tag")],
    );
    expect(shape(tree!)).toBe("c1[c2[c3]]");
    expect(tree!.children[0].refNames).toEqual(["v1.0"]);
  });

  it("follows first parents only, so a merged branch ends as its own leaf", () => {
    // merge m has first parent a (main) and second parent b (feature off root)
    const tree = buildCommitTree(
      [commit("m", ["a", "b"]), commit("b", ["root"]), commit("a", ["root"]), commit("root", [])],
      [ref("main", "m")],
    );
    expect(shape(tree!)).toBe("root[b,m+1]");
  });

  it("joins several roots (e.g. history cut by the load limit) under a virtual root", () => {
    const tree = buildCommitTree(
      [commit("x", ["not-loaded-1"]), commit("y", ["not-loaded-2"])],
      [],
    );
    expect(tree!.commit).toBeNull();
    expect(shape(tree!)).toBe("root[x,y]");
  });

  it("marks the checked-out commit and groups local/remote names", () => {
    const tree = buildCommitTree(
      [commit("c2", ["c1"]), commit("c1", [])],
      [ref("main", "c2", "LocalBranch", true), ref("origin/main", "c2", "RemoteBranch")],
    );
    const tip = tree!.children[0];
    expect(tip.isHead).toBe(true);
    expect(tip.refNames).toEqual(["main"]);
  });
});
