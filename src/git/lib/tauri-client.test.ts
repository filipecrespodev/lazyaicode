import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { errorMessage, hookFailure, hookOutputTail } from "./tauri-client";

describe("hook failures", () => {
  const error = { kind: "HookFailed", details: { hook: "pre-commit", output: "eslint\nerror: x is unused" } };

  it("shows the hook name with its output", () => {
    expect(errorMessage(error)).toBe("The pre-commit hook failed:\neslint\nerror: x is unused");
    expect(errorMessage({ kind: "HookFailed", details: { hook: "commit-msg", output: "" } })).toBe(
      "The commit-msg hook failed",
    );
    expect(hookFailure(error)).toEqual(error.details);
    expect(hookFailure({ kind: "Internal", details: "x" })).toBeNull();
  });

  it("keeps only the end of long output", () => {
    const output = Array.from({ length: 30 }, (_, i) => `line ${i}`).join("\r\n");
    const tail = hookOutputTail(output, 3);
    expect(tail).toBe("…\nline 27\nline 28\nline 29");
  });
});

describe("signing failures", () => {
  it("says the commit couldn't be signed", () => {
    expect(errorMessage({ kind: "SigningFailed", details: { message: "no key" } })).toBe(
      "Couldn't sign the commit: no key",
    );
  });
});
