import { describe, expect, it } from "vitest";
import { buildHeatmap, relativeTime } from "./activity";

const at = (y: number, m: number, d: number, h = 12) => new Date(y, m - 1, d, h).getTime() / 1000;

describe("buildHeatmap", () => {
  // Tuesday 13 Oct 2026.
  const today = new Date(2026, 9, 13, 15);

  it("lays out 53 Sunday-first weeks ending with the current week", () => {
    const weeks = buildHeatmap([], today);
    expect(weeks).toHaveLength(53);
    expect(weeks.every((w) => w.length === 7)).toBe(true);
    const last = weeks[52];
    expect(last[0].date.getDay()).toBe(0);
    expect(last[2].date.getDate()).toBe(13);
    expect(last[2].future).toBe(false);
    expect(last[3].future).toBe(true);
  });

  it("buckets commits per local day and scales levels to the busiest day", () => {
    const times = [at(2026, 10, 13, 9), at(2026, 10, 13, 18), at(2026, 10, 12), ...Array(6).fill(at(2026, 10, 5))];
    const days = buildHeatmap(times, today).flat();
    // The grid spans a year, so it holds Oct 2025 too: match the full date.
    const find = (d: number) =>
      days.find((x) => x.date.getFullYear() === 2026 && x.date.getMonth() === 9 && x.date.getDate() === d)!;
    expect(find(13).count).toBe(2);
    expect(find(5).count).toBe(6);
    expect(find(5).level).toBe(4);
    expect(find(12).level).toBe(1);
    expect(find(11).level).toBe(0);
  });

  it("ignores commits older than the grid", () => {
    const days = buildHeatmap([at(2024, 1, 1)], today).flat();
    expect(days.every((d) => d.count === 0)).toBe(true);
  });
});

describe("relativeTime", () => {
  const now = new Date(2026, 9, 13, 12);
  it("picks the largest fitting unit", () => {
    expect(relativeTime(new Date(2026, 9, 10, 12), now)).toBe("3 days ago");
    expect(relativeTime(new Date(2026, 9, 13, 9), now)).toBe("3 hours ago");
    expect(relativeTime(new Date(2026, 9, 12, 12), now)).toBe("yesterday");
    expect(relativeTime(new Date(2026, 9, 13, 11, 59, 50), now)).toBe("just now");
  });
});
