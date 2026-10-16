import { describe, expect, it } from "vitest";
import { collapseContext } from "./collapseContext";

type T = "normal" | "insert" | "delete";
const hunk = (types: T[]) => ({
  content: "h",
  oldStart: 1,
  newStart: 1,
  oldLines: types.filter((t) => t !== "insert").length,
  newLines: types.filter((t) => t !== "delete").length,
  changes: types.map((type) => ({ type })),
});
const N = (k: number): T[] => Array(k).fill("normal");

describe("collapseContext", () => {
  it("keeps context around changes and hides the long unchanged runs", () => {
    const pieces = collapseContext(hunk([...N(20), "insert", ...N(20)]), 3);
    expect(pieces.map((p) => p.kind)).toEqual(["gap", "hunk", "gap"]);
    expect(pieces[0]).toMatchObject({ count: 17 });
    expect(pieces[1].hunk.changes).toHaveLength(7);
    expect(pieces[1].hunk).toMatchObject({ oldStart: 18, newStart: 18, oldLines: 6, newLines: 7 });
  });

  it("does not hide gaps shorter than the minimum", () => {
    const pieces = collapseContext(hunk(["delete", ...N(5), "insert"]), 1);
    expect(pieces.map((p) => p.kind)).toEqual(["hunk"]);
    expect(pieces[0].hunk.changes).toHaveLength(7);
  });

  it("splits two distant changes into two hunks around a gap", () => {
    const pieces = collapseContext(hunk(["insert", ...N(20), "delete"]), 2);
    expect(pieces.map((p) => p.kind)).toEqual(["hunk", "gap", "hunk"]);
    expect(pieces[2].hunk).toMatchObject({ newStart: 20, oldStart: 19, oldLines: 3, newLines: 2 });
  });

  it("returns the hunk untouched when nothing changed", () => {
    const h = hunk(N(10));
    expect(collapseContext(h, 3)).toEqual([{ kind: "hunk", hunk: h }]);
  });
});
