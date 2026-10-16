import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { signatureLabel } from "./signature";
import {  } from "../../../lib/i18n/react";

describe("signatureLabel", () => {
  it("names the signature kind and hides unsigned commits", () => {
    expect(signatureLabel({ format: "ssh" })).toBe("Signed (SSH)");
    expect(signatureLabel({ format: "gpg" })).toBe("Signed (GPG)");
    expect(signatureLabel({ format: "unknown" })).toBe("Signed");
    expect(signatureLabel(null)).toBeNull();
    expect(signatureLabel(undefined)).toBeNull();
  });
});
