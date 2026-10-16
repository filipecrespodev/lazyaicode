import { describe, expect, it } from "vitest";
import { blameBlocks, compareRange, nextBlameLine } from "./model";
import type { BlameChunk, BranchCompare } from "./types";

const chunk = (startLine: number, lines: string[], hunks: BlameChunk["hunks"], totalLines = 100): BlameChunk => ({
  sha: "head",
  totalLines,
  startLine,
  lines,
  hunks,
  commits: [],
});

describe("blameBlocks", () => {
  it("splits lines into one block per hunk", () => {
    const blocks = blameBlocks(
      chunk(1, ["a", "b", "c"], [
        { startLine: 1, lineCount: 2, sha: "x", origPath: null },
        { startLine: 3, lineCount: 1, sha: "y", origPath: "old.txt" },
      ]),
      null,
    );
    expect(blocks).toEqual([
      { sha: "x", startLine: 1, lines: ["a", "b"], origPath: null, continued: false },
      { sha: "y", startLine: 3, lines: ["c"], origPath: "old.txt", continued: false },
    ]);
  });

  it("keeps lines no hunk covers and clamps hunks to the chunk", () => {
    const blocks = blameBlocks(
      chunk(11, ["k", "l", "m"], [{ startLine: 12, lineCount: 50, sha: "x", origPath: null }]),
      null,
    );
    expect(blocks.map((b) => [b.sha, b.startLine, b.lines])).toEqual([
      ["", 11, ["k"]],
      ["x", 12, ["l", "m"]],
    ]);
  });

  it("marks a block that continues the previous chunk's last commit", () => {
    const c = chunk(5, ["e"], [{ startLine: 5, lineCount: 1, sha: "x", origPath: null }]);
    expect(blameBlocks(c, "x")[0].continued).toBe(true);
    expect(blameBlocks(c, "y")[0].continued).toBe(false);
  });
});

describe("nextBlameLine", () => {
  it("points past the chunk until the file is covered", () => {
    expect(nextBlameLine(chunk(1, ["a", "b"], [], 5))).toBe(3);
    expect(nextBlameLine(chunk(4, ["d", "e"], [], 5))).toBeUndefined();
    expect(nextBlameLine(chunk(1, [], [], 0))).toBeUndefined();
  });
});

describe("compareRange", () => {
  const cmp: BranchCompare = {
    aSha: "a",
    bSha: "b",
    mergeBase: "base",
    onlyInA: [],
    onlyInB: [],
    onlyInATotal: 2,
    onlyInBTotal: 0,
  };

  it("diffs a side from the merge base", () => {
    expect(compareRange(cmp, "a")).toEqual({ from: "base", to: "a" });
  });

  it("is null for a side with no commits of its own", () => {
    expect(compareRange(cmp, "b")).toBeNull();
  });

  it("falls back to the other tip for unrelated histories", () => {
    expect(compareRange({ ...cmp, mergeBase: null }, "a")).toEqual({ from: "b", to: "a" });
  });
});
