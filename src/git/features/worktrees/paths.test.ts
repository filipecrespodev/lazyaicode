import { describe, expect, it } from "vitest";
import { suggestWorktreePath } from "./paths";

describe("suggestWorktreePath", () => {
  it("puts the worktree next to the repo", () => {
    expect(suggestWorktreePath("/home/me/dev/app", "feature/login")).toBe("/home/me/dev/app-feature-login");
    expect(suggestWorktreePath("/home/me/dev/app/", "fix")).toBe("/home/me/dev/app-fix");
  });

  it("keeps Windows separators", () => {
    expect(suggestWorktreePath("C:\\dev\\app", "hot fix")).toBe("C:\\dev\\app-hot-fix");
  });

  it("falls back when the branch is empty", () => {
    expect(suggestWorktreePath("/r/app", "  ")).toBe("/r/app-worktree");
  });
});
