import { describe, expect, it } from "vitest";
import { reachableFrom } from "./reachable";

const c = (sha: string, ...parents: string[]) => ({ sha, parents });

describe("reachableFrom", () => {
  // m merges a and b; both descend from root; x is unrelated
  const commits = [c("m", "a", "b"), c("a", "root"), c("b", "root"), c("root"), c("x", "root")];

  it("follows every parent of a merge", () => {
    expect([...reachableFrom(commits, "m")].sort()).toEqual(["a", "b", "m", "root"]);
  });

  it("excludes commits on other branches", () => {
    expect(reachableFrom(commits, "a").has("b")).toBe(false);
    expect(reachableFrom(commits, "a").has("x")).toBe(false);
  });

  it("ignores unknown tips and parents outside the loaded list", () => {
    expect(reachableFrom(commits, "nope").size).toBe(0);
    expect([...reachableFrom([c("a", "gone")], "a")]).toEqual(["a"]);
  });
});
