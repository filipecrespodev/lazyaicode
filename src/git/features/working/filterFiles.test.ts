import { describe, expect, it } from "vitest";
import { filterFiles } from "./filterFiles";

const files = [{ path: "src/App.tsx" }, { path: "README.md" }, { path: "src/lib/app-utils.ts" }];

describe("filterFiles", () => {
  it("matches path substrings case-insensitively", () => {
    expect(filterFiles(files, "APP").map((f) => f.path)).toEqual(["src/App.tsx", "src/lib/app-utils.ts"]);
  });
  it("returns everything for a blank query", () => {
    expect(filterFiles(files, "  ")).toBe(files);
  });
  it("returns nothing when nothing matches", () => {
    expect(filterFiles(files, "zzz")).toEqual([]);
  });
});
