import { parseDiff } from "react-diff-view";
import { describe, expect, it } from "vitest";
import {
  changeKey,
  changeKeysInOrder,
  changeTexts,
  nextSelection,
  splitIntoChangeBlocks,
  toLineSelection,
} from "./hunkSelection";

const HEADER = ["diff --git a/f.txt b/f.txt", "--- a/f.txt", "+++ b/f.txt"];

/** Full-context patch like the backend sends: old lines 1..20, line 2
 * changed, line 18 removed. */
const PATCH = [
  ...HEADER,
  "@@ -1,20 +1,19 @@",
  " 1",
  "-2",
  "+two",
  ...Array.from({ length: 15 }, (_, i) => ` ${i + 3}`),
  "-18",
  " 19",
  " 20",
  "",
].join("\n");

const hunks = () => parseDiff(PATCH)[0].hunks;

describe("splitIntoChangeBlocks", () => {
  it("splits distant changes into separate blocks covering every line", () => {
    const blocks = splitIntoChangeBlocks(hunks());
    expect(blocks.map((b) => b.changeKeys)).toEqual([["D2", "I2"], ["D18"]]);
    const total = blocks.reduce((n, b) => n + b.hunk.changes.length, 0);
    expect(total).toBe(hunks()[0].changes.length);
    // The second block starts 3 lines before the removal of line 18.
    expect(blocks[1].hunk.oldStart).toBe(15);
    expect(blocks[1].hunk.newStart).toBe(15);
    expect(blocks[1].hunk.changes[0]).toMatchObject({ type: "normal", oldLineNumber: 15 });
    expect(new Set(blocks.map((b) => b.hunk.content)).size).toBe(blocks.length);
  });

  it("keeps changes within twice the context together", () => {
    const blocks = splitIntoChangeBlocks(hunks(), 8);
    expect(blocks.map((b) => b.changeKeys)).toEqual([["D2", "I2", "D18"]]);
  });

  it("puts unchanged lines before the first change in a context-only block", () => {
    const patch = [...HEADER, "@@ -1,6 +1,6 @@", " a", " b", " c", " d", " e", "-f", "+F", ""].join("\n");
    const blocks = splitIntoChangeBlocks(parseDiff(patch)[0].hunks, 3);
    expect(blocks.map((b) => [b.hunk.changes.length, b.changeKeys])).toEqual([
      [2, []],
      [5, ["D6", "I6"]],
    ]);
  });
});

describe("selection", () => {
  it("uses react-diff-view's change keys", () => {
    const [n, d, i] = hunks()[0].changes;
    expect([changeKey(n), changeKey(d), changeKey(i)]).toEqual(["N1", "D2", "I2"]);
  });

  it("toggles on click and selects ranges on shift-click", () => {
    const ordered = changeKeysInOrder(hunks());
    expect(ordered).toEqual(["D2", "I2", "D18"]);
    let sel = nextSelection(new Set(), ordered, "D2", null, false);
    expect([...sel]).toEqual(["D2"]);
    sel = nextSelection(sel, ordered, "D18", "D2", true);
    expect([...sel].sort()).toEqual(["D18", "D2", "I2"]);
    sel = nextSelection(sel, ordered, "I2", "D18", false);
    expect([...sel].sort()).toEqual(["D18", "D2"]);
  });

  it("maps keys to old/new line numbers and their texts for the backend", () => {
    const texts = changeTexts(hunks());
    expect(toLineSelection(["D2", "I2", "D18", "N5"], texts)).toEqual({
      oldLines: [2, 18],
      newLines: [2],
      oldTexts: ["2", "18"],
      newTexts: ["two"],
    });
  });
});
