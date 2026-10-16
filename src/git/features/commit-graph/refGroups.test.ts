import { describe, expect, it } from "vitest";
import { groupRefs } from "./refGroups";
import type { RefInfo } from "./types";

function ref(name: string, kind: RefInfo["kind"], isHead = false): RefInfo {
  return { name, kind, targetSha: "abc", isHead };
}

describe("groupRefs", () => {
  it("merges a local branch with its remote-tracking counterpart", () => {
    const groups = groupRefs([ref("main", "LocalBranch", true), ref("origin/main", "RemoteBranch")]);
    expect(groups).toEqual([
      { name: "main", isHead: true, isTag: false, hasLocal: true, hasRemote: true, remotes: ["origin"] },
    ]);
  });

  it("lists every remote the branch is on at that commit", () => {
    const groups = groupRefs([
      ref("origin/feat/x", "RemoteBranch"),
      ref("feat/x", "LocalBranch"),
      ref("upstream/feat/x", "RemoteBranch"),
    ]);
    expect(groups).toEqual([
      { name: "feat/x", isHead: false, isTag: false, hasLocal: true, hasRemote: true, remotes: ["origin", "upstream"] },
    ]);
  });

  it("keeps distinct branch names at the same commit as separate pills", () => {
    const groups = groupRefs([ref("feature", "LocalBranch"), ref("staging", "LocalBranch", true)]);
    expect(groups.map((g) => g.name)).toEqual(["feature", "staging"]);
  });

  it("keeps a remote-only branch (no local checkout) as its own pill", () => {
    const groups = groupRefs([ref("origin/staging", "RemoteBranch")]);
    expect(groups).toEqual([
      { name: "staging", isHead: false, isTag: false, hasLocal: false, hasRemote: true, remotes: ["origin"] },
    ]);
  });

  it("keeps tags separate even if they share a name with a branch", () => {
    const groups = groupRefs([ref("v1.0", "Tag"), ref("v1.0", "LocalBranch")]);
    expect(groups).toEqual([
      { name: "v1.0", isHead: false, isTag: true, hasLocal: false, hasRemote: false, remotes: [] },
      { name: "v1.0", isHead: false, isTag: false, hasLocal: true, hasRemote: false, remotes: [] },
    ]);
  });

  it("shows a detached HEAD as its own head pill, not grouped with any branch", () => {
    const groups = groupRefs([ref("HEAD", "DetachedHead", true)]);
    expect(groups).toEqual([
      { name: "HEAD", isHead: true, isTag: false, hasLocal: false, hasRemote: false, remotes: [] },
    ]);
  });
});
