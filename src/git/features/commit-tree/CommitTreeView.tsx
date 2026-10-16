import { useQuery } from "@tanstack/react-query";
import { Crosshair, Loader2, Maximize, Minus, Plus, X } from "lucide-react";
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { getCommitLog } from "@/features/commit-graph/api";
import type { CommitNode } from "@/features/commit-graph/types";
import { formatDateTime } from "@/lib/format-date";
import { useThemeMode } from "@/lib/use-theme";
import { COLUMN_WIDTH, layoutTree, treeColorFor, type LaidOutNode, type TreeLayout } from "./treeLayout";
import { buildCommitTree } from "./treeModel";
import { overlayClass, overlayOrInline } from "@/embed";
import { t as translate, useT } from "../../../lib/i18n/react";

// How much history the overview loads in one go (the graph pages by 200).
const TREE_COMMIT_LIMIT = 3000;
const MIN_ZOOM = 0.04;
const MAX_ZOOM = 4;
const NODE_RADIUS = 5;
const FONT = "system-ui, sans-serif";

type View = { k: number; x: number; y: number };

const clampZoom = (k: number) => Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, k));
const clamp01 = (v: number) => Math.min(1, Math.max(0, v));
// Labels fade out when zoomed out; ref labels (branches/tags) hold on longer.
const labelOpacity = (k: number) => clamp01((k - 0.45) / 0.35);
const refLabelOpacity = (k: number) => clamp01((k - 0.15) / 0.25);

function nodeLabel(laid: LaidOutNode): string {
  const { node } = laid;
  if (!node.commit) return "…";
  // Several refs: first one plus a count (the tooltip lists them all).
  if (node.refNames.length > 1) return `${node.refNames[0]} +${node.refNames.length - 1}`;
  if (node.refNames.length === 1) return node.refNames[0];
  return node.children.length === 0 ? node.commit.summary : node.commit.shortSha;
}

// A label may use its own column and no more, so it never runs into the next
// column's nodes. Tip labels start right of the dot; others are centered under it.
const TIP_LABEL_WIDTH = COLUMN_WIDTH - NODE_RADIUS - 18;
const CENTERED_LABEL_WIDTH = COLUMN_WIDTH - 24;

const fitCache = new Map<string, string>();
/** `text` shortened with "…" to fit `maxWidth` in the context's current font. */
function fitText(ctx: CanvasRenderingContext2D, text: string, maxWidth: number): string {
  const key = `${ctx.font}|${maxWidth}|${text}`;
  const cached = fitCache.get(key);
  if (cached !== undefined) return cached;
  let fitted = text;
  if (ctx.measureText(text).width > maxWidth) {
    // Longest prefix that fits together with the ellipsis (binary search).
    let lo = 0;
    let hi = text.length;
    while (lo < hi) {
      const mid = Math.ceil((lo + hi) / 2);
      if (ctx.measureText(`${text.slice(0, mid)}…`).width <= maxWidth) lo = mid;
      else hi = mid - 1;
    }
    fitted = `${text.slice(0, lo).trimEnd()}…`;
  }
  if (fitCache.size > 5000) fitCache.clear();
  fitCache.set(key, fitted);
  return fitted;
}

function nodeTitle(laid: LaidOutNode): string[] {
  const { node } = laid;
  if (!node.commit) return [translate("gitTree.olderHistory")];
  const c = node.commit;
  return [
    node.refNames.length > 0 ? node.refNames.join(", ") : null,
    `${c.shortSha} ${c.summary}`,
    `${c.authorName} · ${formatDateTime(c.authorDate)}`,
    node.skipped > 0 ? node.skipped === 1 ? translate("gitTree.skippedOne") : translate("gitTree.skippedOther", { count: node.skipped }) : null,
  ].filter((l): l is string => l !== null);
}

/**
 * Zoomable, pannable overview of the whole commit tree (horizontal, d3
 * "collapsible tree" style), drawn on a canvas. Navigation follows Obsidian's
 * graph view: the wheel zooms smoothly around the cursor, dragging pans with
 * inertia, hovering a node highlights its neighborhood, and labels fade out as
 * you zoom out. The camera lives in refs and every frame is a plain redraw
 * (culled to the viewport); React only renders the chrome and the tooltip.
 */
export function CommitTreeView({
  repoId,
  onClose,
  onOpenCommit,
}: {
  repoId: string;
  onClose?: () => void;
  onOpenCommit: (commit: CommitNode) => void;
}) {
  const t = useT();
  const { data, isPending } = useQuery({
    queryKey: ["commitTree", repoId],
    queryFn: () => getCommitLog(repoId, 0, TREE_COMMIT_LIMIT),
  });
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const [hovered, setHovered] = useState<LaidOutNode | null>(null);
  const [pointer, setPointer] = useState({ x: 0, y: 0 });

  const tree = useMemo(() => (data ? buildCommitTree(data.commits, data.refs) : null), [data]);
  const layout = useMemo(() => (tree ? layoutTree(tree, collapsed) : null), [tree, collapsed]);

  // Neighborhood of the hovered node: itself, its parent and its children.
  const highlight = useMemo(() => {
    if (!hovered || !layout) return null;
    const id = hovered.node.id;
    const ids = new Set([id]);
    for (const l of layout.links) {
      if (l.from.node.id === id) ids.add(l.to.node.id);
      if (l.to.node.id === id) ids.add(l.from.node.id);
    }
    return ids;
  }, [hovered, layout]);

  const containerRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const view = useRef<View>({ k: 1, x: 0, y: 0 });
  const target = useRef<View>({ k: 1, x: 0, y: 0 });
  const velocity = useRef({ x: 0, y: 0 });
  const frame = useRef<number | null>(null);

  // What draw() reads, kept in refs so the animation loop never goes stale.
  const scene = useRef<{ layout: TreeLayout | null; hovered: LaidOutNode | null; highlight: Set<string> | null }>({
    layout: null,
    hovered: null,
    highlight: null,
  });
  scene.current = { layout, hovered, highlight };

  // ---- drawing --------------------------------------------------------------
  const draw = useCallback(() => {
    const canvas = canvasRef.current;
    const ctx = canvas?.getContext("2d");
    const { layout, hovered, highlight } = scene.current;
    if (!canvas || !ctx) return;

    const dpr = window.devicePixelRatio || 1;
    const css = getComputedStyle(document.documentElement);
    const bg = css.getPropertyValue("--color-neutral-950").trim() || "#0a0a0a";
    const refText = css.getPropertyValue("--color-neutral-200").trim() || "#e5e5e5";
    const plainText = css.getPropertyValue("--color-neutral-400").trim() || "#a3a3a3";
    const mode = document.documentElement.dataset.mode === "light" ? "light" : "dark";
    const paint = (color: string) => treeColorFor(color, mode);

    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.fillStyle = bg;
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    if (!layout) return;

    const { k, x, y } = view.current;
    ctx.setTransform(dpr * k, 0, 0, dpr * k, dpr * x, dpr * y);

    // Visible world rect (+ margin for labels) — everything else is skipped.
    const margin = 260;
    const left = -x / k - margin;
    const top = -y / k - margin;
    const right = (canvas.width / dpr - x) / k + margin;
    const bottom = (canvas.height / dpr - y) / k + margin;
    const inView = (px: number, py: number) => px >= left && px <= right && py >= top && py <= bottom;

    const lo = labelOpacity(k);
    const refLo = refLabelOpacity(k);
    const dimmed = (id: string) => highlight !== null && !highlight.has(id);

    // Links
    ctx.lineWidth = Math.max(1.75, 1 / k);
    for (const l of layout.links) {
      const minX = l.from.x;
      const maxX = l.to.x;
      const minY = Math.min(l.from.y, l.to.y);
      const maxY = Math.max(l.from.y, l.to.y);
      if (maxX < left || minX > right || maxY < top || minY > bottom) continue;
      const xm = (l.from.x + l.to.x) / 2;
      ctx.globalAlpha = highlight ? (highlight.has(l.from.node.id) && highlight.has(l.to.node.id) ? 1 : 0.08) : 0.75;
      ctx.strokeStyle = paint(l.color);
      ctx.beginPath();
      ctx.moveTo(l.from.x, l.from.y);
      ctx.bezierCurveTo(xm, l.from.y, xm, l.to.y, l.to.x, l.to.y);
      ctx.stroke();
      if (l.to.node.skipped > 0 && lo > 0) {
        ctx.globalAlpha *= lo;
        ctx.fillStyle = paint(l.color);
        ctx.font = `9px ${FONT}`;
        ctx.textAlign = "center";
        ctx.textBaseline = "alphabetic";
        ctx.fillText(String(l.to.node.skipped), xm, (l.from.y + l.to.y) / 2 - 4);
      }
    }

    // Nodes + labels
    for (const laid of layout.nodes) {
      if (!inView(laid.x, laid.y)) continue;
      const { node } = laid;
      const isHovered = hovered?.node.id === node.id;
      const alpha = dimmed(node.id) ? 0.15 : 1;
      const hasVisibleChildren = node.children.length > 0 && !laid.hasHiddenChildren;

      ctx.globalAlpha = alpha;
      if (node.isHead) {
        ctx.globalAlpha = alpha * 0.6;
        ctx.strokeStyle = paint(laid.color);
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.arc(laid.x, laid.y, NODE_RADIUS + 4, 0, Math.PI * 2);
        ctx.stroke();
        ctx.globalAlpha = alpha;
      }
      ctx.beginPath();
      ctx.arc(laid.x, laid.y, isHovered ? NODE_RADIUS + 1.5 : NODE_RADIUS, 0, Math.PI * 2);
      // Filled = folded (or a tip), hollow = expanded with children.
      ctx.fillStyle = hasVisibleChildren ? bg : paint(laid.color);
      ctx.fill();
      ctx.lineWidth = 2;
      ctx.strokeStyle = paint(laid.color);
      ctx.stroke();

      const isRefNode = node.refNames.length > 0;
      const textAlpha = isHovered ? 1 : isRefNode ? refLo : lo;
      if (textAlpha <= 0) continue;
      ctx.globalAlpha = alpha * textAlpha;
      ctx.font = `${node.isHead ? 700 : isRefNode ? 600 : 400} ${isRefNode ? 11 : 10}px ${FONT}`;
      const label = fitText(ctx, nodeLabel(laid), hasVisibleChildren ? CENTERED_LABEL_WIDTH : TIP_LABEL_WIDTH);
      const tx = hasVisibleChildren ? laid.x : laid.x + NODE_RADIUS + 5;
      const ty = hasVisibleChildren ? laid.y + NODE_RADIUS + 13 : laid.y;
      ctx.textAlign = hasVisibleChildren ? "center" : "left";
      ctx.textBaseline = hasVisibleChildren ? "alphabetic" : "middle";
      // Background-colored outline keeps labels readable over crossing lines.
      ctx.lineWidth = 3;
      ctx.strokeStyle = bg;
      ctx.strokeText(label, tx, ty);
      ctx.fillStyle = isRefNode ? refText : plainText;
      ctx.fillText(label, tx, ty);
    }
    ctx.globalAlpha = 1;
  }, []);

  // Redraw when the content (data, folding, hover) or the theme changes.
  const themeMode = useThemeMode();
  useEffect(() => {
    draw();
  }, [layout, hovered, highlight, draw, themeMode]);

  // Keep the canvas backing store matched to its CSS size × devicePixelRatio.
  useEffect(() => {
    const container = containerRef.current;
    const canvas = canvasRef.current;
    if (!container || !canvas) return;
    const resize = () => {
      const dpr = window.devicePixelRatio || 1;
      const rect = container.getBoundingClientRect();
      canvas.width = Math.round(rect.width * dpr);
      canvas.height = Math.round(rect.height * dpr);
      canvas.style.width = `${rect.width}px`;
      canvas.style.height = `${rect.height}px`;
      draw();
    };
    resize();
    const observer = new ResizeObserver(resize);
    observer.observe(container);
    return () => observer.disconnect();
  }, [draw]);

  // ---- camera ---------------------------------------------------------------
  // One rAF loop eases the view toward the zoom target and plays out pan inertia.
  const animate = useCallback(() => {
    if (frame.current !== null) return;
    const step = () => {
      const v = view.current;
      const t = target.current;
      const vel = velocity.current;
      t.x += vel.x;
      t.y += vel.y;
      vel.x *= 0.9;
      vel.y *= 0.9;
      v.k += (t.k - v.k) * 0.22;
      v.x += (t.x - v.x) * 0.22;
      v.y += (t.y - v.y) * 0.22;
      const settled =
        Math.abs(t.k - v.k) < 1e-4 &&
        Math.abs(t.x - v.x) < 0.1 &&
        Math.abs(t.y - v.y) < 0.1 &&
        Math.hypot(vel.x, vel.y) < 0.05;
      if (settled) Object.assign(v, t);
      draw();
      frame.current = settled ? null : requestAnimationFrame(step);
    };
    frame.current = requestAnimationFrame(step);
  }, [draw]);

  useEffect(
    () => () => {
      if (frame.current !== null) cancelAnimationFrame(frame.current);
    },
    [],
  );

  /** Zoom the target by `factor`, keeping the screen point (px, py) fixed. */
  const zoomAt = useCallback(
    (factor: number, px: number, py: number) => {
      const t = target.current;
      const k = clampZoom(t.k * factor);
      const wx = (px - t.x) / t.k;
      const wy = (py - t.y) / t.k;
      target.current = { k, x: px - wx * k, y: py - wy * k };
      animate();
    },
    [animate],
  );

  const viewport = () => {
    const rect = containerRef.current?.getBoundingClientRect();
    return rect ? { w: rect.width, h: rect.height } : { w: 0, h: 0 };
  };

  const fitAll = useCallback(() => {
    if (!layout) return;
    const { w, h } = viewport();
    const pad = 80;
    const k = clampZoom(
      Math.min((w - pad * 2) / Math.max(layout.width, 1), (h - pad * 2) / Math.max(layout.height, 1), 1.2),
    );
    target.current = { k, x: (w - layout.width * k) / 2, y: (h - layout.height * k) / 2 };
    velocity.current = { x: 0, y: 0 };
    animate();
  }, [layout, animate]);

  const focusHead = useCallback(
    (instant = false) => {
      if (!layout) return;
      const head = layout.nodes.find((n) => n.node.isHead) ?? layout.nodes[layout.nodes.length - 1];
      const { w, h } = viewport();
      const k = 0.9;
      target.current = { k, x: w / 2 - head.x * k, y: h / 2 - head.y * k };
      velocity.current = { x: 0, y: 0 };
      if (instant) {
        view.current = { ...target.current };
        draw();
      } else {
        animate();
      }
    },
    [layout, animate, draw],
  );

  // First paint: start on the checked-out commit, like opening a note's local graph.
  const placedInitially = useRef(false);
  useLayoutEffect(() => {
    if (!layout || placedInitially.current) return;
    placedInitially.current = true;
    focusHead(true);
  }, [layout, focusHead]);

  // Wheel must be a non-passive listener to stop the page from scrolling.
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      const rect = el.getBoundingClientRect();
      const delta = e.deltaMode === 1 ? e.deltaY * 16 : e.deltaY;
      zoomAt(Math.exp(-delta * 0.0018), e.clientX - rect.left, e.clientY - rect.top);
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, [zoomAt]);

  /** Node under a screen point (container coords), if any. */
  const hitTest = (sx: number, sy: number): LaidOutNode | null => {
    if (!layout) return null;
    const { k, x, y } = view.current;
    const wx = (sx - x) / k;
    const wy = (sy - y) / k;
    // Generous target: at least ~8 screen px however far out you zoom.
    const reach = Math.max(NODE_RADIUS + 3, 8 / k);
    let best: LaidOutNode | null = null;
    let bestDist = reach * reach;
    for (const n of layout.nodes) {
      const d = (n.x - wx) ** 2 + (n.y - wy) ** 2;
      if (d <= bestDist) {
        best = n;
        bestDist = d;
      }
    }
    return best;
  };

  const localPoint = (e: { clientX: number; clientY: number }) => {
    const rect = containerRef.current!.getBoundingClientRect();
    return { x: e.clientX - rect.left, y: e.clientY - rect.top };
  };

  // Drag to pan (background or nodes); a release with speed keeps gliding.
  const drag = useRef<{ x: number; y: number; t: number; moved: boolean } | null>(null);
  const onPointerDown = (e: React.PointerEvent) => {
    if (e.button !== 0) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = { x: e.clientX, y: e.clientY, t: performance.now(), moved: false };
    velocity.current = { x: 0, y: 0 };
  };
  const onPointerMove = (e: React.PointerEvent) => {
    const d = drag.current;
    if (!d) {
      const p = localPoint(e);
      const hit = hitTest(p.x, p.y);
      setPointer(p);
      if (hit?.node.id !== hovered?.node.id) setHovered(hit);
      return;
    }
    const dx = e.clientX - d.x;
    const dy = e.clientY - d.y;
    if (!d.moved) {
      if (Math.hypot(dx, dy) < 3) return;
      d.moved = true;
      setHovered(null);
    }
    const now = performance.now();
    const dt = Math.max(1, now - d.t);
    // Pan moves both the shown view and the target, so it tracks the cursor 1:1.
    view.current.x += dx;
    view.current.y += dy;
    target.current.x += dx;
    target.current.y += dy;
    velocity.current = { x: (dx / dt) * 16, y: (dy / dt) * 16 };
    d.x = e.clientX;
    d.y = e.clientY;
    d.t = now;
    draw();
  };
  const onPointerUp = (e: React.PointerEvent) => {
    const d = drag.current;
    drag.current = null;
    e.currentTarget.releasePointerCapture(e.pointerId);
    if (d?.moved) {
      // Only glide if the pointer was still moving right before release.
      if (performance.now() - d.t < 80) animate();
      else velocity.current = { x: 0, y: 0 };
      return;
    }
    // A click (no drag): fold/unfold the node under the pointer.
    const p = localPoint(e);
    const hit = hitTest(p.x, p.y);
    if (hit && hit.node.children.length > 0) toggle(hit.node.id);
  };
  const onDoubleClick = (e: React.MouseEvent) => {
    const p = localPoint(e);
    const hit = hitTest(p.x, p.y);
    if (hit?.node.commit) onOpenCommit(hit.node.commit);
  };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose?.();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [onClose]);

  const toggle = (id: string) =>
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });

  const zoomButton = (factor: number) => {
    const { w, h } = viewport();
    zoomAt(factor, w / 2, h / 2);
  };

  const toolButton = "rounded p-1.5 text-neutral-400 hover:bg-neutral-800 hover:text-neutral-100";

  return overlayOrInline(
    <div className={`${overlayClass(!!onClose)} flex flex-col bg-neutral-950 text-neutral-200`}>
      <div className="flex shrink-0 items-center justify-between border-b border-neutral-800 px-3 py-2">
        <div className="flex items-baseline gap-3">
          <span className="text-sm font-semibold">{t("gitTree.title")}</span>
          {data && (
            <span className="text-xs text-neutral-500">
              {data.nextSkip !== null
                ? t("gitTree.commitCountLimited", { count: data.commits.length, limit: TREE_COMMIT_LIMIT })
                : t("gitTree.commitCount", { count: data.commits.length })}{" "}
              ·{" "}
              {(layout?.nodes.length ?? 0) === 1
                ? t("gitTree.nodesOne")
                : t("gitTree.nodesOther", { count: layout?.nodes.length ?? 0 })}{" "}
              · {t("gitTree.hint")}
            </span>
          )}
        </div>
        <div className="flex items-center gap-0.5">
          <button title={t("gitTree.zoomIn")} onClick={() => zoomButton(1.4)} className={toolButton}>
            <Plus size={15} />
          </button>
          <button title={t("gitTree.zoomOut")} onClick={() => zoomButton(1 / 1.4)} className={toolButton}>
            <Minus size={15} />
          </button>
          <button title={t("gitTree.fitAll")} onClick={fitAll} className={toolButton}>
            <Maximize size={15} />
          </button>
          <button title={t("gitTree.focusHead")} onClick={() => focusHead()} className={toolButton}>
            <Crosshair size={15} />
          </button>
          {onClose && (
            <>
              <span className="mx-1 h-4 w-px bg-neutral-800" />
              <button title={t("gitTree.close")} onClick={onClose} className={toolButton}>
                <X size={15} />
              </button>
            </>
          )}
        </div>
      </div>

      <div
        ref={containerRef}
        className={`relative min-h-0 flex-1 touch-none overflow-hidden ${
          hovered?.node.children.length ? "cursor-pointer" : "cursor-grab active:cursor-grabbing"
        }`}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerLeave={() => setHovered(null)}
        onDoubleClick={onDoubleClick}
      >
        <canvas ref={canvasRef} className="absolute inset-0" />
        {isPending && (
          <div className="absolute inset-0 flex items-center justify-center gap-2 text-sm text-neutral-500">
            <Loader2 size={16} className="animate-spin" /> {t("gitTree.loading")}
          </div>
        )}
        {hovered && (
          <div
            className="pointer-events-none absolute z-10 max-w-sm rounded border border-neutral-700 bg-neutral-900 px-2 py-1.5 text-xs shadow-xl"
            style={{ left: pointer.x + 14, top: pointer.y + 14 }}
          >
            {nodeTitle(hovered).map((line, i) => (
              <div key={i} className={i === 0 ? "font-semibold text-neutral-100" : "text-neutral-400"}>
                {line}
              </div>
            ))}
          </div>
        )}
      </div>
    </div>,
    !!onClose,
  );
}
