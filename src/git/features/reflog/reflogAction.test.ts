import { describe, expect, it } from "vitest";
import { parseReflogMessage } from "./reflogAction";

describe("parseReflogMessage", () => {
  it("splits git's `<action>: <detail>` messages", () => {
    expect(parseReflogMessage("commit (amend): fix: typo")).toEqual({
      kind: "commit",
      label: "commit (amend)",
      detail: "fix: typo",
    });
    expect(parseReflogMessage("checkout: moving from main to dev").kind).toBe("checkout");
    expect(parseReflogMessage("rebase (finish): returning to refs/heads/x").kind).toBe("rebase");
    expect(parseReflogMessage("merge origin/main: Fast-forward").kind).toBe("merge");
    expect(parseReflogMessage("pull: Fast-forward").kind).toBe("merge");
    expect(parseReflogMessage("cherry-pick: add X").kind).toBe("cherry-pick");
    expect(parseReflogMessage("branch: Created from HEAD").kind).toBe("branch");
  });

  it("keeps unknown or unstructured messages as detail", () => {
    expect(parseReflogMessage("something custom")).toEqual({ kind: "other", label: "", detail: "something custom" });
    expect(parseReflogMessage("stash: WIP").kind).toBe("other");
  });
});
