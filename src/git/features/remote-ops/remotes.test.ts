import { describe, expect, it } from "vitest";
import { defaultRemote, dueForAutoFetch, fetchMovedRefs, groupSharedRepos, remoteNameProblem, splitRemoteBranch } from "./remotes";

describe("defaultRemote", () => {
  it("prefers origin, else the first, else none", () => {
    expect(defaultRemote(["fork", "origin"])).toBe("origin");
    expect(defaultRemote(["fork", "upstream"])).toBe("fork");
    expect(defaultRemote([])).toBeNull();
  });
});

describe("splitRemoteBranch", () => {
  it("keeps slashes in the branch part", () => {
    expect(splitRemoteBranch("origin/feat/x")).toEqual({ remote: "origin", branch: "feat/x" });
    expect(splitRemoteBranch("upstream/main")).toEqual({ remote: "upstream", branch: "main" });
  });
});

describe("remoteNameProblem", () => {
  it("accepts ordinary names", () => {
    expect(remoteNameProblem("upstream", ["origin"])).toBeNull();
    expect(remoteNameProblem("my-fork_2", [])).toBeNull();
  });
  it("rejects empty, duplicate and malformed names", () => {
    expect(remoteNameProblem("  ", [])).toMatch(/Enter the name/);
    expect(remoteNameProblem("origin", ["origin"])).toMatch(/already exists/);
    for (const bad of ["has space", "a..b", "x:y", ".hidden", "end.", "a.lock", "-x"]) {
      expect(remoteNameProblem(bad, [])).toMatch(/Invalid/);
    }
  });
  it("lets a rename keep the current name", () => {
    expect(remoteNameProblem("origin", ["origin"], "origin")).toBeNull();
  });
});

describe("fetchMovedRefs", () => {
  it("refreshes only when refs moved, or when a multi-remote fetch partly failed", () => {
    expect(fetchMovedRefs({ updatedRefs: 2 }, 1)).toBe(true);
    expect(fetchMovedRefs({ updatedRefs: 0 }, 3)).toBe(false);
    expect(fetchMovedRefs(null, 1)).toBe(false);
    expect(fetchMovedRefs(null, 2)).toBe(true);
  });
});

describe("dueForAutoFetch", () => {
  const MIN = 60_000;
  const now = 100 * MIN;
  const last: Record<string, number> = { a: now - 11 * MIN, b: now - 2 * MIN, c: now - 30 * MIN };

  it("picks repos past the interval, longest-waiting first", () => {
    expect(dueForAutoFetch(["a", "b", "c"], (id) => last[id], () => false, now, 10 * MIN)).toEqual(["c", "a"]);
  });

  it("skips repos with a fetch already running", () => {
    expect(dueForAutoFetch(["a", "c"], (id) => last[id], (id) => id === "c", now, 10 * MIN)).toEqual(["a"]);
  });

  it("treats never-fetched repos as due, and lists a repo open in two tabs once", () => {
    expect(dueForAutoFetch(["new", "new", "b"], (id) => last[id], () => false, now, 10 * MIN)).toEqual(["new"]);
  });
});

describe("groupSharedRepos", () => {
  it("groups a repo with its worktrees, keyed by the first tab", () => {
    const groups = groupSharedRepos([
      { repoId: "main", commonDir: "/src/app/.git" },
      { repoId: "other", commonDir: "C:\\work\\lib\\.git" },
      { repoId: "wt", commonDir: "/src/app/.git/" },
      { repoId: "wt2", commonDir: "C:/work/lib/.git" },
    ]);
    expect([...groups]).toEqual([
      ["main", ["main", "wt"]],
      ["other", ["other", "wt2"]],
    ]);
  });

  it("keeps repos without a known common dir apart, and a repo open twice once", () => {
    const groups = groupSharedRepos([
      { repoId: "a" },
      { repoId: "b" },
      { repoId: "c", commonDir: "/x/.git" },
      { repoId: "c", commonDir: "/x/.git" },
    ]);
    expect([...groups.values()]).toEqual([["a"], ["b"], ["c"]]);
  });
});
