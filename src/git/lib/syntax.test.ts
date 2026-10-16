import { describe, expect, it } from "vitest";
import { guessLanguage } from "./syntax";

describe("guessLanguage", () => {
  it("resolves every mapped language to a registered grammar", () => {
    for (const path of ["a.ts", "a.tsx", "a.rs", "a.py", "a.cpp", "a.html", "a.md", "a.sh", "a.toml", "Makefile"]) {
      expect(guessLanguage(path), path).not.toBeNull();
    }
  });
  it("returns null for unknown extensions", () => {
    expect(guessLanguage("a.unknownext")).toBeNull();
    expect(guessLanguage("LICENSE")).toBeNull();
  });
});
