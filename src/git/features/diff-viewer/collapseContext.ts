type Change = { type: "normal" | "insert" | "delete" };
export type HunkLike<C extends Change> = {
  content: string;
  oldStart: number;
  oldLines: number;
  newStart: number;
  newLines: number;
  changes: C[];
};
export type Piece<C extends Change, H extends HunkLike<C>> =
  | { kind: "hunk"; hunk: H }
  | { kind: "gap"; key: string; count: number; hunk: H };

/** Gaps shorter than this stay visible: a "2 unchanged lines" bar saves nothing. */
const MIN_GAP = 4;

/** Splits a full-context hunk into the changed regions (each with `context`
 * lines around it) and the unchanged gaps between them. A gap carries its own
 * hunk so it can be rendered when expanded. */
export function collapseContext<C extends Change, H extends HunkLike<C>>(hunk: H, context: number): Piece<C, H>[] {
  const { changes } = hunk;
  const n = changes.length;
  const keep = new Array<boolean>(n).fill(false);
  changes.forEach((c, i) => {
    if (c.type === "normal") return;
    for (let j = Math.max(0, i - context); j <= Math.min(n - 1, i + context); j++) keep[j] = true;
  });
  if (!keep.some(Boolean)) return [{ kind: "hunk", hunk }];

  // Running line positions, so every slice can state its own start/length.
  const oldAt: number[] = [];
  const newAt: number[] = [];
  let o = hunk.oldStart;
  let nw = hunk.newStart;
  for (const c of changes) {
    oldAt.push(o);
    newAt.push(nw);
    if (c.type !== "insert") o++;
    if (c.type !== "delete") nw++;
  }

  const slices: { start: number; end: number; hidden: boolean }[] = [];
  for (let i = 0; i < n; ) {
    let j = i;
    while (j < n && keep[j] === keep[i]) j++;
    slices.push({ start: i, end: j, hidden: !keep[i] });
    i = j;
  }
  // Short gaps stay visible, and neighbouring visible slices are one hunk.
  const merged: typeof slices = [];
  for (const s of slices) {
    const hidden = s.hidden && s.end - s.start >= MIN_GAP;
    const prev = merged[merged.length - 1];
    if (prev && !prev.hidden && !hidden) prev.end = s.end;
    else merged.push({ ...s, hidden });
  }

  return merged.map(({ start, end, hidden }, i) => {
    const part = changes.slice(start, end);
    const sub = {
      ...hunk,
      content: `${hunk.content}#${i}`,
      oldStart: oldAt[start],
      newStart: newAt[start],
      oldLines: part.filter((c) => c.type !== "insert").length,
      newLines: part.filter((c) => c.type !== "delete").length,
      changes: part,
    } as H;
    return hidden
      ? { kind: "gap" as const, key: sub.content, count: part.length, hunk: sub }
      : { kind: "hunk" as const, hunk: sub };
  });
}
