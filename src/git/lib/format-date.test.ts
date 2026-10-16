import { afterEach, describe, expect, it } from "vitest";
import { setLocale } from "../../lib/i18n/index.svelte";
import { formatDate, formatDateTime } from "./format-date";

describe("format-date", () => {
  const d = new Date(2023, 1, 20, 14, 5);
  afterEach(() => setLocale("en"));

  it("follows the interface language", () => {
    expect(formatDate(d)).toBe("02/20/2023");
    setLocale("pt");
    expect(formatDate(d)).toBe("20/02/2023");
    expect(formatDateTime(d)).toBe("20/02/2023 14:05");
  });
  it("treats numbers as unix seconds", () => {
    setLocale("pt");
    expect(formatDate(d.getTime() / 1000)).toBe("20/02/2023");
  });
});
