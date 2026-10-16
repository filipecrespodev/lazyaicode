import { describe, expect, it } from "vitest";
import type { CommitNode } from "@/features/commit-graph/types";
import { actionForKey, initialRows, isUnchanged, moveRow, toItems, validateRows, type TodoRow } from "./todo";
import {  } from "../../../lib/i18n/react";

const commit = (sha: string, summary = `msg ${sha}`, body = ""): CommitNode => ({
  sha,
  shortSha: sha.slice(0, 7),
  summary,
  body,
  authorName: "a",
  authorEmail: "a@x",
  authorDate: 0,
  committerDate: 0,
  parents: [],
  isMerge: false,
});

const commits = [commit("a1"), commit("b2", "second", "details"), commit("c3")];
const withActions = (...actions: TodoRow["action"][]) =>
  initialRows(commits).map((r, i) => ({ ...r, action: actions[i] ?? r.action }));

describe("interactive rebase todo", () => {
  it("starts as all picks with the full original message", () => {
    const rows = initialRows(commits);
    expect(rows.map((r) => r.action)).toEqual(["pick", "pick", "pick"]);
    expect(rows[1].message).toBe("second\n\ndetails");
    expect(isUnchanged(rows, commits)).toBe(true);
  });

  it("moves rows", () => {
    expect(moveRow(["a", "b", "c"], 2, 0)).toEqual(["c", "a", "b"]);
    expect(moveRow(["a", "b", "c"], 0, 2)).toEqual(["b", "c", "a"]);
    const same = ["a"];
    expect(moveRow(same, 0, 5)).toBe(same);
  });

  it("validates the first kept commit and empty rewords", () => {
    expect(validateRows(withActions("pick", "squash", "fixup"))).toBeNull();
    expect(validateRows(withActions("squash"))).toMatch(/first commit/);
    expect(validateRows(withActions("drop", "fixup"))).toMatch(/first commit/);
    expect(validateRows(withActions("drop", "drop", "drop"))).toMatch(/drop/);
    const emptyReword = withActions("reword");
    emptyReword[0].message = "  ";
    expect(validateRows(emptyReword)).toMatch(/a1/);
  });

  it("detects changes and builds the request", () => {
    const reordered = moveRow(initialRows(commits), 2, 0);
    expect(isUnchanged(reordered, commits)).toBe(false);
    const reword = withActions("reword", "drop");
    expect(isUnchanged(withActions("reword"), commits)).toBe(true);
    reword[0].message = "new";
    expect(toItems(reword)).toEqual([
      { sha: "a1", action: "reword", message: "new" },
      { sha: "b2", action: "drop", message: null },
      { sha: "c3", action: "pick", message: null },
    ]);
    // left as it was: no amend
    expect(toItems(withActions("pick", "reword"))[1]).toEqual({ sha: "b2", action: "pick", message: null });
  });

  it("maps shortcut keys", () => {
    expect(actionForKey("S")).toBe("squash");
    expect(actionForKey("x")).toBeNull();
  });
});
