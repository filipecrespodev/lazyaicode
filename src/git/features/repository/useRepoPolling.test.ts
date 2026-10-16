import { describe, expect, it, vi } from "vitest";

vi.mock("@/features/commit-graph/api", () => ({ getRepoFingerprint: vi.fn() }));

import { REF_KEYS, WORKING_KEYS, refsChanged } from "./useRepoPolling";

describe("refsChanged", () => {
  it("is false only when the fingerprint is known and unchanged", () => {
    expect(refsChanged("a", "a")).toBe(false);
    expect(refsChanged("a", "b")).toBe(true);
    expect(refsChanged(null, "a")).toBe(true);
    expect(refsChanged("a", null)).toBe(true);
  });
});

describe("poll key groups", () => {
  it("keep the log out of the every-tick group", () => {
    const working = WORKING_KEYS("r").map((k) => k[0]);
    expect(working).toContain("workingStatus");
    expect(working).not.toContain("commitLog");
    expect(REF_KEYS("r").map((k) => k[0])).toContain("commitLog");
  });
});
