import { describe, expect, it } from "vitest";
import {
  type ActivityEntry,
  appendEntry,
  commandLine,
  commandRepoPath,
  isRead,
  isSkipped,
  redact,
  samePath,
} from "./format";

describe("classification", () => {
  it("treats lookups as reads and mutations as not", () => {
    for (const cmd of ["get_repo_status", "list_branches", "list_ssh_keys", "github_list_assignees", "gitlab_get_project"]) {
      expect(isRead(cmd)).toBe(true);
    }
    for (const cmd of ["fetch_remote", "push_branch", "git_commit_changes", "github_create_pull_request", "stash_save"]) {
      expect(isRead(cmd)).toBe(false);
    }
  });

  it("skips keystroke-level terminal traffic", () => {
    expect(isSkipped("terminal_write")).toBe(true);
    expect(isSkipped("terminal_spawn")).toBe(false);
  });
});

describe("redact", () => {
  it("masks secrets at any depth", () => {
    expect(
      redact({ passphrase: "hunter2", settings: { apiKey: "sk-1", token: "t", name: "ok" }, list: [{ password: "p" }] }),
    ).toEqual({ passphrase: "•••", settings: { apiKey: "•••", token: "•••", name: "ok" }, list: [{ password: "•••" }] });
  });

  it("masks a token embedded in a remote URL, keeping the rest", () => {
    expect(redact({ name: "origin", url: "https://bot:ghp_abc123@github.com/o/r.git" })).toEqual({
      name: "origin",
      url: "https://bot:•••@github.com/o/r.git",
    });
    expect(redact("https://user@host/o/r.git")).toBe("https://user@host/o/r.git");
    expect(redact("git@github.com:o/r.git")).toBe("git@github.com:o/r.git");
  });

  it("keeps an absent secret absent (cancelled prompt)", () => {
    expect(redact({ passphrase: null })).toEqual({ passphrase: null });
  });

  it("cuts long strings and lists", () => {
    const out = redact({ message: "x".repeat(500), paths: Array.from({ length: 25 }, (_, i) => `f${i}`) }) as {
      message: string;
      paths: string[];
    };
    expect(out.message).toMatch(/… \(500 characters\)$/);
    expect(out.paths).toHaveLength(21);
    expect(out.paths[20]).toBe("… 5 more");
  });
});

describe("commandLine", () => {
  it("reads like a command, without ids or false flags", () => {
    expect(commandLine("fetch_remote", { repoId: "r1", remoteName: "origin", background: true })).toBe(
      "fetch_remote origin background=true",
    );
    expect(commandLine("push_branch", { repoId: "r1", remoteName: "origin", branch: "main", force: false })).toBe(
      "push_branch origin main",
    );
    expect(commandLine("submit_ssh_passphrase", { passphrase: "hunter2" })).toBe("submit_ssh_passphrase passphrase=•••");
  });
});

describe("commandRepoPath", () => {
  const paths: Record<string, string> = { r1: "/repos/app" };
  const lookup = (id: string) => paths[id];

  it("scopes by repoId, or by path for repo-path commands", () => {
    expect(commandRepoPath("fetch_remote", { repoId: "r1" }, lookup)).toBe("/repos/app");
    expect(commandRepoPath("open_repository", { path: "/repos/lib" }, lookup)).toBe("/repos/lib");
    expect(commandRepoPath("list_ssh_keys", undefined, lookup)).toBeNull();
  });
});

describe("samePath", () => {
  it("ignores separators, trailing slashes and drive-letter case", () => {
    expect(samePath("/repos/app/", "/repos/app")).toBe(true);
    expect(samePath("c:\\repos\\app", "C:/repos/app/")).toBe(true);
    expect(samePath("/repos/app", "/repos/app-old")).toBe(false);
    expect(samePath(null, "/repos/app")).toBe(false);
  });
});

describe("appendEntry", () => {
  const entry = (id: number, read: boolean): ActivityEntry => ({
    id,
    at: id,
    source: "command",
    text: `e${id}`,
    read,
    repoPath: null,
    status: "ok",
  });

  it("evicts old reads before they can push out actions", () => {
    let entries: ActivityEntry[] = [entry(0, false)];
    for (let i = 1; i <= 10; i++) entries = appendEntry(entries, entry(i, true), { total: 100, reads: 3 });
    expect(entries.map((e) => e.id)).toEqual([0, 8, 9, 10]);
  });

  it("caps the total, oldest first", () => {
    let entries: ActivityEntry[] = [];
    for (let i = 0; i < 6; i++) entries = appendEntry(entries, entry(i, false), { total: 4, reads: 4 });
    expect(entries.map((e) => e.id)).toEqual([2, 3, 4, 5]);
  });
});
