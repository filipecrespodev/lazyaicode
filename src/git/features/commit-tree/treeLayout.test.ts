import { describe, expect, it } from "vitest";
import { layoutTree, TREE_PALETTE } from "./treeLayout";
import type { TreeNode } from "./treeModel";

function node(id: string, children: TreeNode[] = [], skipped = 0): TreeNode {
  return { id, commit: null, refNames: [], isHead: false, skipped, children };
}

const byId = (layout: ReturnType<typeof layoutTree>) => new Map(layout.nodes.map((n) => [n.node.id, n]));

describe("layoutTree", () => {
  it("places depth on x and keeps a straight line on one y", () => {
    const layout = layoutTree(node("a", [node("b", [node("c")])]), new Set());
    const pos = byId(layout);
    expect(pos.get("a")!.x).toBeLessThan(pos.get("b")!.x);
    expect(pos.get("b")!.x).toBeLessThan(pos.get("c")!.x);
    expect(new Set(layout.nodes.map((n) => n.y)).size).toBe(1);
  });

  it("keeps the parent level with its biggest child and fans side branches around it", () => {
    // "main" carries 5 skipped commits, so it continues the line; the two
    // small branches go one above and one below.
    const root = node("root", [node("side1"), node("main", [], 5), node("side2")]);
    const pos = byId(layoutTree(root, new Set()));
    expect(pos.get("root")!.y).toBe(pos.get("main")!.y);
    const sides = [pos.get("side1")!.y, pos.get("side2")!.y].sort((a, b) => a - b);
    expect(sides[0]).toBeLessThan(pos.get("main")!.y);
    expect(sides[1]).toBeGreaterThan(pos.get("main")!.y);
  });

  it("passes the color down the main line and gives side branches their own", () => {
    const root = node("root", [node("main", [], 5), node("side")]);
    const pos = byId(layoutTree(root, new Set()));
    expect(pos.get("root")!.color).toBe(TREE_PALETTE[0]);
    expect(pos.get("main")!.color).toBe(TREE_PALETTE[0]);
    expect(pos.get("side")!.color).not.toBe(TREE_PALETTE[0]);
  });

  it("hides the children of collapsed nodes and flags them", () => {
    const layout = layoutTree(node("a", [node("b", [node("c")])]), new Set(["b"]));
    const pos = byId(layout);
    expect(pos.has("c")).toBe(false);
    expect(pos.get("b")!.hasHiddenChildren).toBe(true);
    expect(layout.links.map((l) => `${l.from.node.id}->${l.to.node.id}`)).toEqual(["a->b"]);
  });

  it("does not overlap leaves", () => {
    const root = node("r", [node("x", [node("x1"), node("x2")]), node("y", [node("y1"), node("y2")], 9)]);
    const layout = layoutTree(root, new Set());
    const leafYs = layout.nodes.filter((n) => n.node.children.length === 0).map((n) => n.y);
    expect(new Set(leafYs).size).toBe(leafYs.length);
  });

  it("keeps an old side branch next to its parent on a long main line", () => {
    // A 12-deep main line where every main node also has a side branch: the
    // side branch of the oldest node must stay a row or two away, not be
    // pushed up past every newer side branch.
    let main = node("m12", [], 0);
    for (let i = 11; i >= 0; i--) main = node(`m${i}`, [node(`side${i}`), { ...main, skipped: 20 }]);
    const pos = byId(layoutTree(main, new Set()));
    expect(Math.abs(pos.get("side0")!.y - pos.get("m0")!.y)).toBeLessThanOrEqual(2 * 30);
    expect(Math.abs(pos.get("side11")!.y - pos.get("m11")!.y)).toBeLessThanOrEqual(2 * 30);
  });

  it("never stacks two nodes of the same column on one row", () => {
    let main = node("m8");
    for (let i = 7; i >= 0; i--) {
      main = node(`m${i}`, [node(`a${i}`, [node(`a${i}x`), node(`a${i}y`)]), { ...main, skipped: 20 }, node(`b${i}`)]);
    }
    const layout = layoutTree(main, new Set());
    const seen = new Set<string>();
    for (const n of layout.nodes) {
      const key = `${n.x}:${n.y}`;
      expect(seen.has(key)).toBe(false);
      seen.add(key);
    }
  });
});

