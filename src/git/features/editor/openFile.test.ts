import { describe, expect, it } from "vitest";
import { pickEditor } from "./openFile";

const installed = [
  { id: "code", name: "VS Code", command: "/usr/bin/code" },
  { id: "zed", name: "Zed", command: "/usr/bin/zed" },
];

describe("pickEditor", () => {
  it("returns the saved editor when installed", () => {
    expect(pickEditor(installed, "zed")?.command).toBe("/usr/bin/zed");
  });
  it("falls back to the system default when unset or uninstalled", () => {
    expect(pickEditor(installed, null)).toBeNull();
    expect(pickEditor(installed, "cursor")).toBeNull();
  });
});
