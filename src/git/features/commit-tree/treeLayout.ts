import type { TreeNode } from "./treeModel";

export const TREE_PALETTE = [
  "#f2c14e",
  "#d7263d",
  "#9d4edd",
  "#f77f00",
  "#2a9d8f",
  "#3a86ff",
  "#e76f51",
  "#8ac926",
  "#ff006e",
  "#06d6a0",
];

/** TREE_PALETTE's hues deepened for light themes, same order. */
const TREE_PALETTE_LIGHT = [
  "#b7791f",
  "#c81d34",
  "#7b2cbf",
  "#c2410c",
  "#1f766b",
  "#1d4ed8",
  "#b9472c",
  "#4d7c0f",
  "#be185d",
  "#047857",
];

/** The color to paint a layout color with in the current theme mode. */
export function treeColorFor(color: string, mode: "dark" | "light"): string {
  if (mode === "dark") return color;
  const i = TREE_PALETTE.indexOf(color);
  return i >= 0 ? TREE_PALETTE_LIGHT[i] : color;
}

export const COLUMN_WIDTH = 190;
export const ROW_HEIGHT = 30;

export type LaidOutNode = {
  node: TreeNode;
  x: number;
  y: number;
  color: string;
  /** Has children that are currently collapsed away. */
  hasHiddenChildren: boolean;
};

export type LaidOutLink = { from: LaidOutNode; to: LaidOutNode; color: string };

export type TreeLayout = {
  nodes: LaidOutNode[];
  links: LaidOutLink[];
  width: number;
  height: number;
};

/** Per-column extent of a laid-out subtree, relative to its root (index =
 * depth below the root, value = topmost / bottommost y in that column). */
type Contour = { top: number[]; bottom: number[] };

/**
 * Horizontal tree: depth on x, compact on y. Each node sits level with its
 * *main* child — the one holding the most commits — so a long-lived branch
 * reads as a straight line, while side branches fan out alternately above and
 * below it. The main child inherits the parent's color; every side branch
 * starts a new one.
 *
 * Side subtrees are packed Reingold–Tilford style: each is shifted just far
 * enough to clear what's already placed *in the columns it actually spans*
 * (compared through per-column contours). Handing out one global row slot per
 * leaf instead pushed an old commit's side branch above every newer side
 * branch, so on a long main line it ended up hundreds of rows away.
 */
export function layoutTree(root: TreeNode, collapsed: ReadonlySet<string>): TreeLayout {
  const sizeMemo = new Map<string, number>();
  const size = (n: TreeNode): number => {
    let s = sizeMemo.get(n.id);
    if (s === undefined) {
      s = (n.commit ? 1 : 0) + n.children.reduce((sum, c) => sum + c.skipped + size(c), 0);
      sizeMemo.set(n.id, s);
    }
    return s;
  };

  type Placed = LaidOutNode & { offset: number; kids: Placed[] };
  const nodes: Placed[] = [];
  const links: LaidOutLink[] = [];
  let nextColor = 1;
  let maxDepth = 0;

  const place = (n: TreeNode, depth: number, color: string): { laid: Placed; contour: Contour } => {
    maxDepth = Math.max(maxDepth, depth);
    const isCollapsed = collapsed.has(n.id);
    const laid: Placed = {
      node: n,
      x: depth * COLUMN_WIDTH,
      y: 0,
      color,
      hasHiddenChildren: isCollapsed && n.children.length > 0,
      offset: 0,
      kids: [],
    };
    nodes.push(laid);

    const visible = isCollapsed ? [] : n.children;
    if (visible.length === 0) return { laid, contour: { top: [0], bottom: [0] } };

    const main = visible.reduce((best, c) => (c.skipped + size(c) > best.skipped + size(best) ? c : best));
    const sides = visible.filter((c) => c !== main);

    // Main child sits level with this node; its columns start one deeper.
    const placedMain = place(main, depth + 1, color);
    laid.kids.push(placedMain.laid);
    links.push({ from: laid, to: placedMain.laid, color });
    const contour: Contour = { top: [0, ...placedMain.contour.top], bottom: [0, ...placedMain.contour.bottom] };

    sides.forEach((side, i) => {
      const sideColor = TREE_PALETTE[nextColor++ % TREE_PALETTE.length];
      const placed = place(side, depth + 1, sideColor);
      const goesAbove = i % 2 === 0;
      // Closest shift that clears every shared column by a full row.
      let offset = goesAbove ? Infinity : -Infinity;
      placed.contour.top.forEach((_, d) => {
        const col = d + 1;
        if (col >= contour.top.length) return;
        offset = goesAbove
          ? Math.min(offset, contour.top[col] - placed.contour.bottom[d] - ROW_HEIGHT)
          : Math.max(offset, contour.bottom[col] - placed.contour.top[d] + ROW_HEIGHT);
      });
      placed.laid.offset = offset;
      laid.kids.push(placed.laid);
      links.push({ from: laid, to: placed.laid, color: sideColor });
      placed.contour.top.forEach((top, d) => {
        const col = d + 1;
        const bottom = placed.contour.bottom[d];
        if (col < contour.top.length) {
          contour.top[col] = Math.min(contour.top[col], top + offset);
          contour.bottom[col] = Math.max(contour.bottom[col], bottom + offset);
        } else {
          contour.top[col] = top + offset;
          contour.bottom[col] = bottom + offset;
        }
      });
    });
    return { laid, contour };
  };

  const { laid: rootLaid } = place(root, 0, TREE_PALETTE[0]);

  // Offsets are relative to the parent; resolve them top-down (iteratively —
  // a long main line is deep), then shift everything so the top is y = 0.
  const stack: Placed[] = [rootLaid];
  while (stack.length) {
    const p = stack.pop()!;
    for (const k of p.kids) {
      k.y = p.y + k.offset;
      stack.push(k);
    }
  }
  const minY = Math.min(...nodes.map((n) => n.y));
  const maxY = Math.max(...nodes.map((n) => n.y));
  for (const n of nodes) n.y -= minY;

  return {
    // Same objects the links point at (the extra offset/kids fields are harmless).
    nodes,
    links,
    width: maxDepth * COLUMN_WIDTH,
    height: maxY - minY,
  };
}
