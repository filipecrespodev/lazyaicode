import { describe, expect, it } from "vitest";
import { escapeIgnorePath, ignoreOptions } from "./ignorePatterns";

describe("ignoreOptions", () => {
  it("offers the file, its extension and every folder above it", () => {
    expect(ignoreOptions("src/app/debug.log").map((o) => o.pattern)).toEqual([
      "/src/app/debug.log",
      "*.log",
      "/src/app/",
      "/src/",
    ]);
  });

  it("has no extension option for dotfiles or extensionless names", () => {
    expect(ignoreOptions(".env").map((o) => o.pattern)).toEqual(["/.env"]);
    expect(ignoreOptions("Makefile").map((o) => o.pattern)).toEqual(["/Makefile"]);
  });

  it("treats untracked folder entries as folders", () => {
    expect(ignoreOptions("out/build/").map((o) => o.pattern)).toEqual(["/out/build/", "/out/"]);
  });

  it("gives every option a distinct label", () => {
    const labels = ignoreOptions("a/b/c.txt").map((o) => o.label);
    expect(new Set(labels).size).toBe(labels.length);
  });
});

describe("escapeIgnorePath", () => {
  it("escapes glob characters and a trailing space", () => {
    expect(escapeIgnorePath("a[1]*?.txt")).toBe("a\\[1\\]\\*\\?.txt");
    expect(escapeIgnorePath("name ")).toBe("name\\ ");
  });
});
