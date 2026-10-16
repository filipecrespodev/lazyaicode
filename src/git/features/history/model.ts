import type { BlameChunk, BranchCompare } from "./types";

export type BlameBlock = {
  sha: string;
  startLine: number;
  lines: string[];
  origPath: string | null;
  /** Same commit as the block ending the previous chunk: drawn without a header. */
  continued: boolean;
};

/** Splits a blame chunk's lines into one block per hunk. Lines no hunk
 * covers (shouldn't happen, but blame is best-effort) get a block with an
 * empty sha rather than being dropped. */
export function blameBlocks(chunk: BlameChunk, prevLastSha: string | null): BlameBlock[] {
  const blocks: BlameBlock[] = [];
  const end = chunk.startLine + chunk.lines.length;
  const slice = (from: number, to: number) => chunk.lines.slice(from - chunk.startLine, to - chunk.startLine);
  let line = chunk.startLine;
  for (const hunk of chunk.hunks) {
    const from = Math.max(hunk.startLine, chunk.startLine);
    const to = Math.min(hunk.startLine + hunk.lineCount, end);
    if (to <= from) continue;
    if (from > line) blocks.push({ sha: "", startLine: line, lines: slice(line, from), origPath: null, continued: false });
    blocks.push({ sha: hunk.sha, startLine: from, lines: slice(from, to), origPath: hunk.origPath, continued: false });
    line = to;
  }
  if (line < end) blocks.push({ sha: "", startLine: line, lines: slice(line, end), origPath: null, continued: false });
  if (blocks.length > 0 && prevLastSha !== null && blocks[0].sha !== "" && blocks[0].sha === prevLastSha) {
    blocks[0].continued = true;
  }
  return blocks;
}

/** First line of the next chunk to load, or undefined once the file is covered. */
export function nextBlameLine(chunk: BlameChunk): number | undefined {
  const next = chunk.startLine + chunk.lines.length;
  return chunk.lines.length > 0 && next <= chunk.totalLines ? next : undefined;
}

export type CompareSide = "a" | "b";

/** Aggregate diff for one side of a comparison: what that side changed
 * since the branches diverged (`git diff <other>...<side>`). Unrelated
 * histories have no merge base, so it falls back to the other tip. Null
 * when that side has nothing of its own. */
export function compareRange(cmp: BranchCompare, side: CompareSide): { from: string; to: string } | null {
  const total = side === "a" ? cmp.onlyInATotal : cmp.onlyInBTotal;
  if (total === 0) return null;
  const to = side === "a" ? cmp.aSha : cmp.bSha;
  const other = side === "a" ? cmp.bSha : cmp.aSha;
  return { from: cmp.mergeBase ?? other, to };
}
