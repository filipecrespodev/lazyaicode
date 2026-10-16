import { describe, expect, it } from "vitest";
import { INHERIT, keyOptions, normalizeSshKeySettings, resolveKey, setOverride, type SshKeySettings } from "./settings";

const settings: SshKeySettings = {
  defaultKey: "/home/me/.ssh/default",
  overrides: { "/ws": "/home/me/.ssh/work", "/ws/app": "/home/me/.ssh/app" },
};

describe("resolveKey", () => {
  it("prefers the repo's key, then its workspace's, then the default", () => {
    expect(resolveKey(settings, "/ws/app")).toBe("/home/me/.ssh/app");
    expect(resolveKey(settings, "/ws/lib")).toBe("/home/me/.ssh/work");
    expect(resolveKey(settings, "/other")).toBe("/home/me/.ssh/default");
  });

  it("matches whole directories, trailing slashes and Windows separators", () => {
    expect(resolveKey(settings, "/ws/app-old")).toBe("/home/me/.ssh/work");
    expect(resolveKey(settings, "/wsx")).toBe("/home/me/.ssh/default");
    expect(resolveKey(settings, "/ws/app/")).toBe("/home/me/.ssh/app");
    const win = { defaultKey: null, overrides: { "C:\\ws": "k" } };
    expect(resolveKey(win, "C:/ws/repo")).toBe("k");
  });
});

describe("setOverride", () => {
  it("sets and clears without touching the input", () => {
    const set = setOverride(settings, "/x", "/k");
    expect(set.overrides["/x"]).toBe("/k");
    expect(settings.overrides["/x"]).toBeUndefined();
    expect(setOverride(set, "/x", null).overrides).toEqual(settings.overrides);
  });
});

describe("normalizeSshKeySettings", () => {
  it("fills in what older or missing config lacks", () => {
    expect(normalizeSshKeySettings(null)).toEqual({ defaultKey: null, overrides: {} });
    expect(normalizeSshKeySettings({ defaultKey: "/k" })).toEqual({ defaultKey: "/k", overrides: {} });
  });
});

describe("keyOptions", () => {
  const keys = [
    { path: "/s/a", name: "a", encrypted: false },
    { path: "/s/b", name: "b", encrypted: true },
  ];

  it("lists the inherit choice first and flags keys needing a passphrase", () => {
    expect(keyOptions(keys, null, "Inherit")).toEqual([
      { value: INHERIT, label: "Inherit" },
      { value: "/s/a", label: "a" },
      { value: "/s/b", label: "b (with passphrase)" },
    ]);
  });

  it("keeps a configured key that disappeared from disk", () => {
    const options = keyOptions(keys, "/s/gone", "Inherit");
    expect(options[options.length - 1]).toEqual({ value: "/s/gone", label: "/s/gone (not found)" });
  });
});
