import { describe, expect, it } from "vitest";
import { lineReach, svgRowIndices, visibleRange } from "./virtualWindow";

describe("visibleRange", () => {
  it("covers the viewport plus overscan, clamped to the list", () => {
    expect(visibleRange(0, 100, 1000, 10, 5)).toEqual({ start: 0, end: 15 });
    expect(visibleRange(500, 100, 1000, 10, 5)).toEqual({ start: 45, end: 65 });
    expect(visibleRange(9990, 100, 1000, 10, 5)).toEqual({ start: 994, end: 1000 });
  });
  it("handles an empty list and a negative scroll", () => {
    expect(visibleRange(-20, 100, 0, 10, 5)).toEqual({ start: 0, end: 0 });
  });
});

describe("svgRowIndices", () => {
  const rows = [
    { parentConnections: [{ toRowIndex: 9 }] }, // long line from row 0 down to row 9
    { parentConnections: [{ toRowIndex: 2 }] },
    { parentConnections: [{ toRowIndex: -1 }] },
    { parentConnections: [] },
  ];
  it("computes how far each row's lines reach", () => {
    expect(lineReach(rows)).toEqual([9, 2, 2, 3]);
  });
  it("adds earlier rows whose lines cross into the window", () => {
    expect(svgRowIndices(lineReach(rows), 3, 4)).toEqual([0, 3]);
    expect(svgRowIndices(lineReach(rows), 2, 4)).toEqual([0, 1, 2, 3]);
    expect(svgRowIndices(lineReach(rows), 0, 4)).toEqual([0, 1, 2, 3]);
  });
});
