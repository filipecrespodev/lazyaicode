import { describe, expect, it } from "vitest";
import { pullSummary, pullTitle, strategyOptions } from "./pull";
import type { PullResult } from "./types";

const result = (over: Partial<PullResult>): PullResult => ({
  upstream: "origin/main",
  strategy: "Merge",
  outcome: "FastForwarded",
  commits: 1,
  conflicts: [],
  autostashConflicts: [],
  ...over,
});

describe("strategyOptions", () => {
  it("puts the configured strategy first and marks it", () => {
    const opts = strategyOptions("Rebase");
    expect(opts.map((o) => o.strategy)).toEqual(["Rebase", "Merge", "FfOnly"]);
    expect(opts[0].label).toMatch(/default$/);
    expect(opts.filter((o) => o.label.endsWith("default"))).toHaveLength(1);
  });

  it("lists all three without a known default", () => {
    expect(strategyOptions(undefined).map((o) => o.strategy)).toEqual(["Merge", "FfOnly", "Rebase"]);
  });
});

describe("pullTitle", () => {
  it("explains a missing upstream", () => {
    expect(pullTitle(null)).toMatch(/No upstream branch/);
  });

  it("shows counts and the default strategy", () => {
    const title = pullTitle({ upstream: "origin/main", ahead: 1, behind: 3, strategy: "FfOnly" });
    expect(title).toContain("3 behind, 1 ahead of origin/main");
    expect(title).toContain("fast-forward only");
  });

  it("says up to date when nothing differs", () => {
    expect(pullTitle({ upstream: "origin/main", ahead: 0, behind: 0, strategy: "Merge" })).toMatch(/^Up to date/);
  });
});

describe("pullSummary", () => {
  it("describes each outcome", () => {
    expect(pullSummary(result({ outcome: "UpToDate", commits: 0 }))).toBe("Already up to date with origin/main");
    expect(pullSummary(result({ commits: 2 }))).toBe("Fast-forwarded 2 commits from origin/main");
    expect(pullSummary(result({ outcome: "Merged" }))).toBe("Merged 1 commit from origin/main");
    expect(pullSummary(result({ outcome: "Rebased", commits: 4 }))).toBe("Rebased onto origin/main (4 commits received)");
  });

  it("leaves conflicts to the conflict panel", () => {
    expect(pullSummary(result({ outcome: "Conflicts", conflicts: ["a"] }))).toBeNull();
  });
});
