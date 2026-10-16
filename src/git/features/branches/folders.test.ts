import { describe, expect, it } from "vitest";
import { buildFolders, folderBranches, nestedFolderPaths } from "./folders";
import type { BranchInfo } from "./types";

const b = (name: string): BranchInfo => ({ name, isRemote: false, isHead: false, targetSha: "x", ahead: null });

describe("buildFolders", () => {
  const root = buildFolders([b("main"), b("feat/b"), b("feat/a"), b("feat/ui/x"), b("fix/y")], "");

  it("nests by '/' and sorts folders and leaves", () => {
    expect(root.leaves.map((l) => l.label)).toEqual(["main"]);
    expect(root.folders.map((f) => f.name)).toEqual(["feat", "fix"]);
    const feat = root.folders[0];
    expect(feat.leaves.map((l) => l.label)).toEqual(["a", "b"]);
    expect(feat.folders[0].path).toBe("/feat/ui");
  });

  it("strips the prefix (remote name)", () => {
    const r = buildFolders([b("origin/feat/a")], "origin/");
    expect(r.folders[0].name).toBe("feat");
    expect(r.folders[0].leaves[0].label).toBe("a");
  });

  it("collects branches and nested folder paths", () => {
    const feat = root.folders[0];
    expect(folderBranches(feat).map((x) => x.name).sort()).toEqual(["feat/a", "feat/b", "feat/ui/x"]);
    expect(nestedFolderPaths(feat)).toEqual(["/feat/ui"]);
  });
});
