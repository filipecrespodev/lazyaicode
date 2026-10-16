/** Rows [start, end) worth rendering for a viewport, with `overscan` rows of slack on each side. */
export function visibleRange(scrollTop: number, viewportHeight: number, total: number, rowHeight: number, overscan: number) {
  const first = Math.floor(Math.max(0, scrollTop) / rowHeight);
  const last = Math.ceil((Math.max(0, scrollTop) + viewportHeight) / rowHeight);
  return { start: Math.max(0, first - overscan), end: Math.min(total, last + overscan) };
}

/** For each row, the furthest row its lines reach down to (itself when it has none). */
export function lineReach(rows: { parentConnections: { toRowIndex: number }[] }[]): number[] {
  return rows.map((r, i) => r.parentConnections.reduce((max, c) => Math.max(max, c.toRowIndex), i));
}

/** Rows to draw in the SVG: those in [start, end), plus earlier rows whose lines run down into the window. */
export function svgRowIndices(reach: number[], start: number, end: number): number[] {
  const out: number[] = [];
  for (let i = 0; i < start; i++) if (reach[i] >= start) out.push(i);
  for (let i = start; i < end; i++) out.push(i);
  return out;
}
