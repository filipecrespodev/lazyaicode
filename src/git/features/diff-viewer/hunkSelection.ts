import type { ChangeData, HunkData } from "react-diff-view";

/** Same key react-diff-view gives a change (`getChangeKey`): what its
 * `selectedChanges` matches on. Kept local so this module stays pure. */
export function changeKey(change: ChangeData): string {
  if (change.type === "normal") return `N${change.oldLineNumber}`;
  return `${change.type === "insert" ? "I" : "D"}${change.lineNumber}`;
}

export type ChangeBlock = {
  hunk: HunkData;
  /** Keys of the added/removed lines in this block; empty for context only. */
  changeKeys: string[];
};

/**
 * The working-copy diff is generated with full-file context, so a file is a
 * single hunk. This cuts it back into the hunks `git diff` would show (changes
 * at most `2 * context` unchanged lines apart stay together) without hiding
 * anything: every line lands in exactly one block, in order. Each change
 * block starts `context` lines before its first change; unchanged lines
 * before that go to the previous block, or to a leading context-only block.
 */
export function splitIntoChangeBlocks(hunks: HunkData[], context = 3): ChangeBlock[] {
  const blocks: ChangeBlock[] = [];
  for (const hunk of hunks) {
    const { changes } = hunk;
    const starts: number[] = [];
    let lastChange = -Infinity;
    changes.forEach((c, i) => {
      if (c.type === "normal") return;
      if (i - lastChange - 1 > 2 * context) starts.push(Math.max(0, i - context));
      lastChange = i;
    });
    if (starts.length === 0) {
      blocks.push({ hunk, changeKeys: [] });
      continue;
    }
    if (starts[0] > 0) starts.unshift(0);

    let oldLine = hunk.oldStart;
    let newLine = hunk.newStart;
    starts.forEach((start, b) => {
      const slice = changes.slice(start, starts[b + 1] ?? changes.length);
      const oldStart = oldLine;
      const newStart = newLine;
      const changeKeys: string[] = [];
      for (const c of slice) {
        if (c.type !== "insert") oldLine++;
        if (c.type !== "delete") newLine++;
        if (c.type !== "normal") changeKeys.push(changeKey(c));
      }
      const oldLines = oldLine - oldStart;
      const newLines = newLine - newStart;
      blocks.push({
        hunk: {
          content: `@@ -${oldStart},${oldLines} +${newStart},${newLines} @@`,
          oldStart,
          newStart,
          oldLines,
          newLines,
          changes: slice,
        },
        changeKeys,
      });
    });
  }
  return blocks;
}

/** Keys of every added/removed line, in display order — the range shift-click
 * selection walks. */
export function changeKeysInOrder(hunks: HunkData[]): string[] {
  const keys: string[] = [];
  for (const hunk of hunks) for (const c of hunk.changes) if (c.type !== "normal") keys.push(changeKey(c));
  return keys;
}

/** Click toggles one line; shift-click adds every changed line between the
 * last clicked one (`anchor`) and this one. */
export function nextSelection(
  current: ReadonlySet<string>,
  ordered: string[],
  key: string,
  anchor: string | null,
  shift: boolean,
): Set<string> {
  const next = new Set(current);
  const from = anchor ? ordered.indexOf(anchor) : -1;
  const to = ordered.indexOf(key);
  if (shift && from !== -1 && to !== -1) {
    const [lo, hi] = from < to ? [from, to] : [to, from];
    for (let i = lo; i <= hi; i++) next.add(ordered[i]);
  } else if (next.has(key)) {
    next.delete(key);
  } else {
    next.add(key);
  }
  return next;
}

export type LineSelection = {
  oldLines: number[];
  newLines: number[];
  /** Each line's text as shown, so the backend can refuse numbers that no
   * longer point at it (the file changed since this diff was loaded). */
  oldTexts: string[];
  newTexts: string[];
};

/** Text of every added/removed line by change key. */
export function changeTexts(hunks: HunkData[]): Map<string, string> {
  const texts = new Map<string, string>();
  for (const hunk of hunks) for (const c of hunk.changes) if (c.type !== "normal") texts.set(changeKey(c), c.content);
  return texts;
}

/** Backend shape: removed lines by old-side number, added by new-side. */
export function toLineSelection(keys: Iterable<string>, texts: ReadonlyMap<string, string>): LineSelection {
  const selection: LineSelection = { oldLines: [], newLines: [], oldTexts: [], newTexts: [] };
  for (const key of keys) {
    const n = Number(key.slice(1));
    const text = texts.get(key) ?? "";
    if (key[0] === "D") {
      selection.oldLines.push(n);
      selection.oldTexts.push(text);
    } else if (key[0] === "I") {
      selection.newLines.push(n);
      selection.newTexts.push(text);
    }
  }
  return selection;
}
