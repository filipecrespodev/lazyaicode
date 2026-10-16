import { intlLocale, t } from "../../../lib/i18n/react";

export type HeatmapDay = {
  /** Local midnight of this day. */
  date: Date;
  count: number;
  /** 0 (no commits) to 4 (busiest), relative to the busiest day shown. */
  level: 0 | 1 | 2 | 3 | 4;
  /** Past `today`: the rest of the current week, drawn as empty space. */
  future: boolean;
};

const WEEKS = 53;

const dayKey = (d: Date) => `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`;

/** GitHub-style contribution grid: `WEEKS` columns of 7 days (Sunday first),
 * the last column holding the week of `today`. Commits are bucketed by the
 * viewer's local calendar day. */
export function buildHeatmap(commitTimes: number[], today: Date = new Date()): HeatmapDay[][] {
  const counts = new Map<string, number>();
  for (const t of commitTimes) {
    const key = dayKey(new Date(t * 1000));
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }

  const end = new Date(today.getFullYear(), today.getMonth(), today.getDate());
  const start = new Date(end);
  start.setDate(end.getDate() - end.getDay() - (WEEKS - 1) * 7);

  const weeks: HeatmapDay[][] = [];
  let max = 0;
  for (let w = 0; w < WEEKS; w++) {
    const week: HeatmapDay[] = [];
    for (let d = 0; d < 7; d++) {
      // setDate on a fresh copy steps calendar days, so DST shifts don't skew it.
      const date = new Date(start);
      date.setDate(start.getDate() + w * 7 + d);
      const future = date > end;
      const count = future ? 0 : (counts.get(dayKey(date)) ?? 0);
      max = Math.max(max, count);
      week.push({ date, count, level: 0, future });
    }
    weeks.push(week);
  }

  for (const week of weeks) {
    for (const day of week) {
      if (day.count > 0) day.level = Math.max(1, Math.ceil((day.count / max) * 4)) as HeatmapDay["level"];
    }
  }
  return weeks;
}

const UNITS: [Intl.RelativeTimeFormatUnit, number][] = [
  ["year", 365 * 24 * 3600],
  ["month", 30 * 24 * 3600],
  ["week", 7 * 24 * 3600],
  ["day", 24 * 3600],
  ["hour", 3600],
  ["minute", 60],
];

const rtfCache = new Map<string, Intl.RelativeTimeFormat>();
function rtf(): Intl.RelativeTimeFormat {
  const locale = intlLocale();
  let f = rtfCache.get(locale);
  if (!f) rtfCache.set(locale, (f = new Intl.RelativeTimeFormat(locale, { numeric: "auto" })));
  return f;
}

/** "3 days ago", "yesterday", "just now"... */
export function relativeTime(date: Date, now: Date = new Date()): string {
  const diffSecs = (date.getTime() - now.getTime()) / 1000;
  for (const [unit, secs] of UNITS) {
    if (Math.abs(diffSecs) >= secs) return rtf().format(Math.round(diffSecs / secs), unit);
  }
  return t("gitStats.justNow");
}
