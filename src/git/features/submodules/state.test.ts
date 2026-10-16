import { describe, expect, it } from "vitest";
import type { SubmoduleInfo, SubmoduleState } from "./api";
import { canOpen, canUpdate, needsUpdate } from "./state";

const sm = (path: string, state: SubmoduleState): SubmoduleInfo => ({
  name: path,
  path,
  absPath: `/r/${path}`,
  url: null,
  expectedSha: "a",
  currentSha: state === "uninitialized" ? null : "a",
  initialized: state !== "uninitialized",
  state,
});

describe("submodule state", () => {
  it("updates only what's missing or on another commit", () => {
    const list = [sm("a", "uninitialized"), sm("b", "upToDate"), sm("c", "outOfDate"), sm("d", "dirty")];
    expect(needsUpdate(list).map((s) => s.path)).toEqual(["a", "c"]);
    expect(canUpdate(sm("d", "dirty"))).toBe(false);
  });

  it("opens only checked-out submodules", () => {
    expect(canOpen(sm("a", "uninitialized"))).toBe(false);
    expect(canOpen(sm("a", "dirty"))).toBe(true);
  });
});
