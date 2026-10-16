import { describe, expect, it } from "vitest";
import { truncateDiff } from "./prDiff";

describe("truncateDiff", () => {
  it("keeps small diffs whole and skips binary files", () => {
    const out = truncateDiff(
      [
        { path: "a.ts", patch: "+x" },
        { path: "img.png", patch: "", isBinary: true },
      ],
      1000,
    );
    expect(out).toBe("--- a.ts\n+x\n");
  });

  it("shares the budget so a huge file doesn't crowd out the others", () => {
    const out = truncateDiff(
      [
        { path: "big.ts", patch: "x".repeat(10_000) },
        { path: "small.ts", patch: "+y" },
      ],
      2000,
    );
    expect(out).toContain("--- small.ts\n+y");
    expect(out).toContain("(truncated)");
    expect(out.length).toBeLessThan(2200);
  });

  it("notes files that no longer fit", () => {
    const files = Array.from({ length: 10 }, (_, i) => ({ path: `f${i}`, patch: "z".repeat(600) }));
    expect(truncateDiff(files, 1000)).toMatch(/more file\(s\) omitted/);
  });
});
