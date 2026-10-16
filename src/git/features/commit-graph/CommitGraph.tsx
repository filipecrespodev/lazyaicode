import { useInfiniteQuery, useIsMutating } from "@tanstack/react-query";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Archive, Check, Cloud, GitBranch, GitMerge, Loader2, Monitor, Pencil, Tag } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type MouseEvent } from "react";
import { createPortal } from "react-dom";
import { toast } from "sonner";
import { useContextMenu } from "@/components/ContextMenu";
import { ResizeHandle } from "@/components/ResizeHandle";
import { CreateBranchModal } from "@/features/branches/CreateBranchModal";
import { buildBranchMenuItems, copyNameItem } from "@/features/branches/branchMenu";
import { useRemoteTags } from "@/features/branches/useRemoteTags";
import { useThemeMode } from "@/lib/use-theme";
import { useBranchActions } from "@/features/branches/useBranchActions";
import { CreateTagModal } from "@/features/branches/CreateTagModal";
import { CreateMergeRequestModal } from "@/features/integrations/CreateMergeRequestModal";
import { HOSTING_LABELS } from "@/features/integrations/provider";
import type { MergeRequestInfo, RemoteHosting } from "@/features/integrations/types";
import { useHostingRemote } from "@/features/integrations/useHostingRemote";
import { useOpenMergeRequests } from "@/features/integrations/useOpenMergeRequests";
import { SquashCommitsModal } from "@/features/interactive-rebase/SquashCommitsModal";
import { isSquashable, oldestFirst } from "@/features/interactive-rebase/squash";
import { InteractiveRebaseModal } from "@/features/interactive-rebase/InteractiveRebaseModal";
import { lineReach, svgRowIndices, visibleRange } from "./virtualWindow";
import { useShortcut } from "@/features/shortcuts/useShortcuts";
import { CommitSearchPalette } from "./CommitSearchPalette";
import { useBranchHoverStore } from "./branchHoverStore";
import { reachableFrom } from "./reachable";
import { useRebaseStore } from "@/features/rebase/store";
import type { StashInfo } from "@/features/stash/types";
import { useRevertCommit } from "@/features/history/useRevertCommit";
import { useStashActions } from "@/features/stash/useStashActions";
import { confirm } from "@/lib/confirm-store";
import { formatDateTime } from "@/lib/format-date";
import { gravatarUrl } from "@/lib/gravatar";
import { queryKeys } from "@/lib/query-keys";
import { usePersistedNumber, usePersistedValue } from "@/lib/use-persisted-config";
import { defaultRemote } from "@/features/remote-ops/remotes";
import { getCommitLog } from "./api";
import { useBranchDragStore } from "./branchDragStore";
import { computeLanes, type LaneInput } from "./lanes";
import { groupRefs, type GroupedRef } from "./refGroups";
import type { CommitNode, GraphSelection, RefInfo } from "./types";
import { isGitViewActive, portalRoot } from "@/embed";
import { useT } from "../../../lib/i18n/react";
import {  } from "../../../lib/i18n/react";

type GraphRowEntry =
  | { kind: "commit"; commit: CommitNode }
  | { kind: "wip" }
  | { kind: "stash"; stash: StashInfo };

const PAGE_SIZE = 200;
const OVERSCAN_ROWS = 12;
const ROW_HEIGHT = 28;
const LANE_WIDTH = 16;
const AVATAR_SIZE = 16;
// Lane colors. Lime instead of a plain green, so no lane can be mistaken for
// the checked-out branch's HEAD_COLOR.
const PALETTE = ["#22d3ee", "#a78bfa", "#f472b6", "#fb923c", "#a3e635", "#facc15", "#60a5fa", "#f87171"];
// Same hues, deep enough to read on the pale backgrounds of light themes.
const LIGHT_PALETTE = ["#0891b2", "#7c3aed", "#db2777", "#ea580c", "#65a30d", "#ca8a04", "#2563eb", "#dc2626"];
// Reserved for the checked-out branch's ref pill (and the repo header / branch
// sidebar) — a CSS var so light themes can darken it. Its graph line keeps
// its lane color; it's only drawn slightly thicker.
const HEAD_COLOR = "var(--head-color)";

/** Floating label that follows the cursor while a branch pill is being
 * dragged. Pointer-based drag (see branchDragStore) rather than native HTML5
 * DnD: WebKitGTK (Tauri's Linux webview) renders that API's drag ghost as its
 * own toplevel window, which some window managers list as a phantom taskbar
 * entry — plain mouse events have no such surface to leak. Mounted once at
 * the top of the graph, not per-pill. */
function BranchDragGhost() {
  const draggingBranch = useBranchDragStore((s) => s.draggingBranch);
  const position = useBranchDragStore((s) => s.position);
  const move = useBranchDragStore((s) => s.move);
  const stop = useBranchDragStore((s) => s.stop);

  useEffect(() => {
    if (!draggingBranch) return;
    const onMouseMove = (e: globalThis.MouseEvent) => move(e.clientX, e.clientY);
    const onMouseUp = () => stop();
    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
    // Dragging the pointer over other text (commit messages, other pills)
    // would otherwise start a text selection mid-drag.
    const previousUserSelect = document.body.style.userSelect;
    document.body.style.userSelect = "none";
    return () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
      document.body.style.userSelect = previousUserSelect;
    };
  }, [draggingBranch, move, stop]);

  if (!draggingBranch) return null;

  return createPortal(
    <div
      style={{ position: "fixed", left: position.x + 14, top: position.y + 14, zIndex: 200 }}
      className="pointer-events-none flex items-center gap-1 rounded bg-violet-900 px-1.5 py-0.5 text-xs text-violet-100 shadow-lg"
    >
      <GitBranch size={11} className="shrink-0" />
      {draggingBranch}
    </div>,
    portalRoot(),
  );
}

/** Path from A through corner C to B with the corner rounded (radius capped
 * by both legs), instead of a hard right angle — GitKraken-style bends. */
function elbowPath(ax: number, ay: number, cx: number, cy: number, bx: number, by: number): string {
  const r = Math.min(LANE_WIDTH * 0.75, Math.hypot(cx - ax, cy - ay), Math.hypot(bx - cx, by - cy));
  const inX = Math.sign(cx - ax);
  const inY = Math.sign(cy - ay);
  const outX = Math.sign(bx - cx);
  const outY = Math.sign(by - cy);
  return `M ${ax} ${ay} L ${cx - inX * r} ${cy - inY * r} Q ${cx} ${cy} ${cx + outX * r} ${cy + outY * r} L ${bx} ${by}`;
}

function BranchPill({
  group,
  repoId,
  targetSha,
  localBranchNames,
  currentBranch,
  hosting,
  openMr,
  onCreateMr,
  headColor,
  pinnedBranch,
  onTogglePin,
}: {
  group: GroupedRef;
  repoId: string;
  targetSha: string;
  localBranchNames: Set<string>;
  currentBranch: string | null;
  hosting: RemoteHosting | null;
  openMr: MergeRequestInfo | undefined;
  onCreateMr: (source: string, target: string) => void;
  headColor: string;
  pinnedBranch: string | null;
  onTogglePin: (branchName: string) => void;
}) {
  const t = useT();
  const labels = hosting ? HOSTING_LABELS[hosting.provider] : null;
  const branchActions = useBranchActions(repoId);
  const {
    checkout,
    checkoutAtCommit,
    checkoutWithFeedback,
    checkoutAtWithFeedback,
    rebaseOnto,
    syncToRemote,
    syncToRemoteWithFeedback,
    mergeInto,
  } = branchActions;
  const { open, menu } = useContextMenu();
  const anyMutating = useIsMutating() > 0;
  const { tags: remoteTags } = useRemoteTags(repoId, group.isTag);
  const isPending =
    checkout.isPending ||
    checkoutAtCommit.isPending ||
    rebaseOnto.isPending ||
    syncToRemote.isPending ||
    mergeInto.isPending;

  const handleDoubleClick = async () => {
    if (group.isTag || group.isHead || anyMutating) return;
    if (group.hasLocal) {
      checkoutWithFeedback(group.name);
      return;
    }
    // Remote-only ref: if a local branch with this name exists elsewhere, checking
    // it out here means resetting it to this commit — confirm before doing that.
    const existsElsewhere = localBranchNames.has(group.name);
    if (existsElsewhere && group.name === currentBranch) {
      // Already checked out on this exact branch and its remote counterpart
      // has moved — sync in place (autostash, hard reset, re-apply) instead
      // of the generic "move a different local branch here" flow below.
      syncToRemoteWithFeedback(targetSha);
      return;
    }
    if (existsElsewhere) {
      const confirmed = await confirm(
        t("gitGraph.resetBranchConfirm", { name: group.name }),
        { title: t("gitGraph.resetBranchTitle"), kind: "warning" },
      );
      if (!confirmed) return;
    }
    // Tracks the remote this pill came from (origin when it's on several).
    checkoutAtWithFeedback({ name: group.name, targetSha, remoteName: defaultRemote(group.remotes) });
  };

  // Right-click: origin is the checked-out branch. Drop: origin is the dragged pill.
  const buildMenuItems = (origin: string | null, originCheckedOut: boolean, dragMode: boolean) => {
    if (group.isTag) return dragMode ? [] : [copyNameItem("tag", group.name)];
    return buildBranchMenuItems(branchActions, anyMutating, group, origin, originCheckedOut, dragMode, {
      startAt: { repoId, start: targetSha },
      leading: [
        {
          label: pinnedBranch === group.name ? t("gitGraph.unpinLeft") : t("gitGraph.pinLeft"),
          onClick: () => onTogglePin(group.name),
        },
        ...(openMr && labels
          ? [{ label: t("gitGraph.openRequest", { request: labels.request, provider: labels.name }), onClick: () => openUrl(openMr.webUrl) }]
          : []),
      ],
      comparison: [
        ...(labels && origin && group.name !== origin
          ? [{ label: t("gitGraph.createRequest", { request: labels.request, source: origin, target: group.name }), onClick: () => onCreateMr(origin, group.name) }]
          : []),
      ],
    });
  };

  const handleContextMenu = (e: React.MouseEvent) => {
    const items = buildMenuItems(currentBranch, true, false);
    if (items.length === 0) return;
    open(e, items);
  };

  // Pointer-based drag (see branchDragStore for why: not native HTML5 DnD).
  const draggingBranch = useBranchDragStore((s) => s.draggingBranch);
  const startDrag = useBranchDragStore((s) => s.start);
  const stopDrag = useBranchDragStore((s) => s.stop);
  const [isDropTarget, setIsDropTarget] = useState(false);

  // onMouseEnter/onMouseLeave (unlike dragenter/dragleave) don't fire for
  // nested child elements — the browser only fires them when the pointer
  // truly enters/leaves the element's whole subtree — so no flicker guard
  // is needed here.
  useEffect(() => {
    if (!draggingBranch) setIsDropTarget(false);
  }, [draggingBranch]);

  const setHoverTip = useBranchHoverStore((s) => s.set);

  const handleMouseEnter = () => {
    if (!draggingBranch || draggingBranch === group.name) setHoverTip(targetSha);
    if (!draggingBranch || group.isTag || draggingBranch === group.name) return;
    setIsDropTarget(true);
  };

  const handleMouseLeave = () => {
    setIsDropTarget(false);
    setHoverTip(null);
  };

  // A plain click (mousedown immediately followed by mouseup, no real
  // movement) must still reach onDoubleClick/onContextMenu for checkout and
  // the menu — only start showing the drag ghost once the pointer has
  // actually moved a few pixels.
  const handleMouseDown = (e: React.MouseEvent) => {
    if (group.isTag || e.button !== 0) return;
    const startX = e.clientX;
    const startY = e.clientY;
    let started = false;

    const onMove = (ev: globalThis.MouseEvent) => {
      if (started) return;
      if (Math.hypot(ev.clientX - startX, ev.clientY - startY) < 4) return;
      started = true;
      startDrag(group.name, ev.clientX, ev.clientY);
    };
    const onUp = () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  };

  const handleMouseUp = (e: React.MouseEvent) => {
    const dragging = useBranchDragStore.getState().draggingBranch;
    stopDrag();
    if (!dragging || group.isTag || dragging === group.name) return;
    const items = buildMenuItems(dragging, dragging === currentBranch, true);
    if (items.length === 0) return;
    open(e, items);
  };

  return (
    <span
      onDoubleClick={handleDoubleClick}
      onContextMenu={handleContextMenu}
      onMouseDown={handleMouseDown}
      onMouseEnter={handleMouseEnter}
      onMouseLeave={handleMouseLeave}
      onMouseUp={handleMouseUp}
      title={
        group.isTag
          ? group.name
          : t("gitGraph.pillTitle", { name: group.name }) +
            (group.remotes.length ? "\n" + t("gitGraph.pillRemotes", { remotes: group.remotes.join(", ") }) : "")
      }
      className={`inline-flex max-w-full items-center gap-1 truncate rounded px-1.5 py-0.5 text-xs select-none ${
        group.isTag
          ? "bg-amber-900/60 text-amber-200"
          : group.isHead
            ? ""
            : "bg-violet-900/60 text-violet-200"
      } ${!group.isTag && !group.isHead ? "cursor-pointer hover:brightness-125" : ""}`}
      style={{
        ...(group.isHead
          ? {
              // Same hue as the HEAD line in the graph. Text is pulled toward
              // the theme's foreground so it stays readable in light themes.
              color: `color-mix(in srgb, ${headColor} 70%, var(--color-neutral-100))`,
              backgroundColor: `color-mix(in srgb, ${headColor} 22%, transparent)`,
            }
          : undefined),
        // Drop-target highlight: an outline (not border/box-shadow) so it
        // doesn't shift layout, plus a background tint so it reads clearly
        // even over the violet/HEAD pill backgrounds.
        ...(isDropTarget && {
          outline: "2px solid var(--color-blue-400)",
          outlineOffset: "1px",
          backgroundColor: "color-mix(in srgb, var(--color-blue-400) 30%, transparent)",
        }),
      }}
    >
      {isPending ? (
        <Loader2 size={11} className="shrink-0 animate-spin" />
      ) : (
        <>
          {group.isHead && <Check size={11} className="shrink-0" />}
          {group.isTag && <Tag size={11} className="shrink-0" />}
        </>
      )}
      <span className="truncate">{group.name}</span>
      {(group.hasLocal || group.isTag) && <Monitor size={11} className="shrink-0 opacity-70" />}
      {(group.hasRemote || (group.isTag && remoteTags?.includes(group.name))) && (
        <Cloud size={11} className="shrink-0 opacity-70" />
      )}
      {openMr && (
        <span title={t("gitGraph.requestOpen", { request: labels?.request ?? "Merge request" })} className="inline-flex shrink-0">
          <GitMerge size={11} className="opacity-70" />
        </span>
      )}
      {menu}
    </span>
  );
}

function BranchCell({
  groups,
  repoId,
  targetSha,
  localBranchNames,
  currentBranch,
  width,
  hosting,
  openMrByBranch,
  onCreateMr,
  headColor,
  pinnedBranch,
  onTogglePin,
  laneColor,
}: {
  groups: GroupedRef[];
  repoId: string;
  targetSha: string;
  localBranchNames: Set<string>;
  currentBranch: string | null;
  width: number;
  hosting: RemoteHosting | null;
  openMrByBranch: Map<string, MergeRequestInfo>;
  onCreateMr: (source: string, target: string) => void;
  headColor: string;
  pinnedBranch: string | null;
  onTogglePin: (branchName: string) => void;
  laneColor: string;
}) {
  if (groups.length === 0) return <span className="shrink-0" style={{ width }} />;

  const primary = groups.find((g) => g.isHead) ?? groups[0];
  const extra = groups.filter((g) => g !== primary);

  return (
    <span className="group/branchcell relative flex shrink-0 items-center px-1" style={{ width }}>
      <span className="flex min-w-0 items-center gap-1 overflow-hidden">
        <BranchPill
          group={primary}
          repoId={repoId}
          targetSha={targetSha}
          localBranchNames={localBranchNames}
          currentBranch={currentBranch}
          hosting={hosting}
          openMr={openMrByBranch.get(primary.name)}
          onCreateMr={onCreateMr}
          headColor={headColor}
          pinnedBranch={pinnedBranch}
          onTogglePin={onTogglePin}
        />
        {extra.length > 0 && (
          <span className="shrink-0 rounded bg-neutral-700 px-1 text-[10px] text-neutral-300">
            +{extra.length}
          </span>
        )}
      </span>
      {/* First half of the faint badge → commit lead; the SVG draws the rest
          from x = -4, i.e. from where this ends (the cell's right padding). */}
      <span className="ml-1 min-w-1 flex-1 border-t opacity-50" style={{ borderColor: laneColor }} />
      {extra.length > 0 && (
        <span className="absolute left-1 top-0.5 z-10 hidden w-max min-w-[calc(100%-0.5rem)] flex-col items-start gap-1 rounded border border-neutral-700 bg-neutral-900 p-1 shadow-xl group-hover/branchcell:flex">
          <BranchPill
            group={primary}
            repoId={repoId}
            targetSha={targetSha}
            localBranchNames={localBranchNames}
            currentBranch={currentBranch}
            hosting={hosting}
            openMr={openMrByBranch.get(primary.name)}
            onCreateMr={onCreateMr}
            headColor={headColor}
            pinnedBranch={pinnedBranch}
            onTogglePin={onTogglePin}
          />
          {extra.map((g) => (
            <BranchPill
              key={g.name + g.isTag}
              group={g}
              repoId={repoId}
              targetSha={targetSha}
              localBranchNames={localBranchNames}
              currentBranch={currentBranch}
              hosting={hosting}
              openMr={openMrByBranch.get(g.name)}
              onCreateMr={onCreateMr}
              headColor={headColor}
              pinnedBranch={pinnedBranch}
              onTogglePin={onTogglePin}
            />
          ))}
        </span>
      )}
    </span>
  );
}

// ponytail: renders the whole loaded page directly (no @tanstack/react-virtual),
// fine up to a few thousand rows; add row virtualization if that stops being true.
export function CommitGraph({
  repoId,
  selection,
  currentBranch,
  hasLocalChanges,
  onSelectionChange,
}: {
  repoId: string;
  selection: GraphSelection;
  currentBranch: string | null;
  hasLocalChanges: boolean;
  onSelectionChange: (next: GraphSelection) => void;
}) {
  const palette = useThemeMode() === "light" ? LIGHT_PALETTE : PALETTE;
  const t = useT();
  const { data, fetchNextPage, hasNextPage, isFetchingNextPage, isPending } = useInfiniteQuery({
    queryKey: queryKeys.commitLog(repoId),
    queryFn: ({ pageParam }) => getCommitLog(repoId, pageParam, PAGE_SIZE),
    initialPageParam: 0,
    getNextPageParam: (lastPage) => lastPage.nextSkip ?? undefined,
  });
  const {
    localBranchNames,
    checkoutDetachedWithFeedback,
    cherryPickWithFeedback,
    cherryPickManyWithFeedback,
    resetToWithFeedback,
  } = useBranchActions(repoId);
  const { stashes, pop: stashPop, apply: stashApply, drop: stashDrop } = useStashActions(repoId);
  const { revertWithFeedback } = useRevertCommit(repoId);
  const { open: openCommitMenu, menu: commitMenu } = useContextMenu();
  const anyMutating = useIsMutating() > 0;

  const hosting = useHostingRemote(repoId);
  const openMrByBranch = useOpenMergeRequests(repoId, hosting);
  const [createMr, setCreateMr] = useState<{ source: string; target: string } | null>(null);
  const [createBranchAt, setCreateBranchAt] = useState<string | null>(null);
  const [createTagAt, setCreateTagAt] = useState<string | null>(null);
  const [rebaseFrom, setRebaseFrom] = useState<string | null>(null);
  const [searchOpen, setSearchOpen] = useState(false);
  const [squashCommits, setSquashCommits] = useState<CommitNode[] | null>(null);
  const rebaseInProgress = useRebaseStore((s) => s.status !== null);

  const [branchColWidth, setBranchColWidth] = usePersistedNumber("graph.branchColWidth", 200);
  const [dateColWidth, setDateColWidth] = usePersistedNumber("graph.dateColWidth", 144);

  const commits = useMemo(() => data?.pages.flatMap((p) => p.commits) ?? [], [data]);
  const commitIndexBySha = useMemo(
    () => new Map(commits.map((c, i) => [c.sha, i] as const)),
    [commits],
  );
  const lastClickedShaRef = useRef<string | null>(null);
  const hoverTip = useBranchHoverStore((s) => s.tipSha);
  const reachable = useMemo(() => (hoverTip ? reachableFrom(commits, hoverTip) : null), [commits, hoverTip]);
  const dim = (sha: string) => (reachable && !reachable.has(sha) ? 0.2 : undefined);

  const selectedShas = useMemo(() => {
    if (selection?.kind === "commit") return new Set([selection.commit.sha]);
    if (selection?.kind === "commits") return new Set(selection.commits.map((c) => c.sha));
    return new Set<string>();
  }, [selection]);

  const handleCommitClick = (e: MouseEvent, commit: CommitNode) => {
    const index = commitIndexBySha.get(commit.sha);
    if (index === undefined) return;

    if (e.shiftKey && lastClickedShaRef.current) {
      const lastIndex = commitIndexBySha.get(lastClickedShaRef.current);
      if (lastIndex !== undefined) {
        const [lo, hi] = [lastIndex, index].sort((a, b) => a - b);
        const range = commits.slice(lo, hi + 1);
        onSelectionChange({ kind: "commits", commits: range, fromSha: commits[hi].sha, toSha: commits[lo].sha });
        return;
      }
    }

    if (e.ctrlKey || e.metaKey) {
      const current =
        selection?.kind === "commits" ? selection.commits : selection?.kind === "commit" ? [selection.commit] : [];
      const exists = current.some((c) => c.sha === commit.sha);
      const next = exists ? current.filter((c) => c.sha !== commit.sha) : [...current, commit];
      lastClickedShaRef.current = commit.sha;
      if (next.length === 0) return onSelectionChange(null);
      if (next.length === 1) return onSelectionChange({ kind: "commit", commit: next[0] });
      const indices = next.map((c) => commitIndexBySha.get(c.sha) ?? 0);
      const minIdx = Math.min(...indices);
      const maxIdx = Math.max(...indices);
      return onSelectionChange({
        kind: "commits",
        commits: next,
        fromSha: commits[maxIdx].sha,
        toSha: commits[minIdx].sha,
      });
    }

    lastClickedShaRef.current = commit.sha;
    const isSameSingle = selection?.kind === "commit" && selection.commit.sha === commit.sha;
    onSelectionChange(isSameSingle ? null : { kind: "commit", commit });
  };

  const handleCommitContextMenu = (e: React.MouseEvent, commit: CommitNode) => {
    const multiSelected =
      selection?.kind === "commits" && selection.commits.some((c) => c.sha === commit.sha);

    if (multiSelected && selection?.kind === "commits") {
      const shas = selection.commits
        .map((c) => c.sha)
        .sort((a, b) => (commitIndexBySha.get(b) ?? 0) - (commitIndexBySha.get(a) ?? 0));
      openCommitMenu(e, [
        {
          label: t("gitGraph.cherryPickMany", { count: shas.length }),
          onClick: () => cherryPickManyWithFeedback(shas),
          disabled: anyMutating,
        },
        ...(isSquashable(selection.commits, commits)
          ? [
              {
                label: t("gitGraph.squashMany", { count: shas.length }),
                onClick: () => setSquashCommits(oldestFirst(selection.commits, commits)),
                disabled: anyMutating || rebaseInProgress,
              },
            ]
          : []),
      ]);
      return;
    }

    openCommitMenu(e, [
      {
        label: t("gitGraph.copySha"),
        onClick: () => {
          writeText(commit.sha);
          toast.success(t("gitGraph.shaCopied"));
        },
      },
      {
        label: t("gitGraph.createBranchHere"),
        onClick: () => setCreateBranchAt(commit.sha),
        disabled: anyMutating,
      },
      {
        label: t("gitGraph.createTagHere"),
        onClick: () => setCreateTagAt(commit.sha),
        disabled: anyMutating,
      },
      {
        label: t("gitGraph.checkoutCommit"),
        onClick: () => checkoutDetachedWithFeedback(commit.sha),
        disabled: anyMutating,
      },
      {
        label: t("gitGraph.cherryPick"),
        onClick: async () => {
          const createCommit = await confirm(
            t("gitGraph.cherryPickConfirm"),
            { title: t("gitGraph.cherryPick"), kind: "info" },
          );
          cherryPickWithFeedback({ sha: commit.sha, createCommit });
        },
        disabled: anyMutating,
      },
      // A merge is reverted relative to one of its parents (`git revert -m`):
      // the submenu asks which side to keep.
      commit.isMerge
        ? {
            label: t("gitGraph.revertCommit"),
            submenu: commit.parents.map((parent, i) => {
              const summary = commits[commitIndexBySha.get(parent) ?? -1]?.summary;
              return {
                label: `${t("gitGraph.keepParent", { n: i + 1, sha: parent.slice(0, 7) })}${summary ? ` ${summary.slice(0, 40)}` : ""}`,
                onClick: () => revertWithFeedback({ sha: commit.sha, mainline: i + 1 }),
                disabled: anyMutating,
              };
            }),
          }
        : {
            label: t("gitGraph.revertCommit"),
            onClick: () => revertWithFeedback({ sha: commit.sha, mainline: null }),
            disabled: anyMutating,
          },
      {
        // This commit and everything after it on the checked-out branch.
        label: t("gitGraph.interactiveRebaseFromHere"),
        onClick: () => setRebaseFrom(commit.sha),
        disabled: anyMutating || rebaseInProgress || commit.isMerge,
      },
      ...(currentBranch
        ? [
            {
              label: t("gitGraph.resetTo", { branch: currentBranch }),
              submenu: [
                {
                  label: t("gitGraph.resetSoft"),
                  onClick: () => resetToWithFeedback({ sha: commit.sha, hard: false }),
                  disabled: anyMutating || hasLocalChanges,
                },
                {
                  label: t("gitGraph.resetHard"),
                  onClick: () => resetToWithFeedback({ sha: commit.sha, hard: true }),
                  disabled: anyMutating || hasLocalChanges,
                },
              ],
            },
          ]
        : []),
    ]);
  };

  const handleStashContextMenu = (e: React.MouseEvent, stash: StashInfo) => {
    openCommitMenu(e, [
      {
        label: t("gitGraph.stashApply"),
        onClick: () => stashApply.mutate(stash.index),
        disabled: anyMutating,
      },
      {
        label: t("gitGraph.stashPop"),
        onClick: () => stashPop.mutate(stash.index),
        disabled: anyMutating,
      },
      {
        label: t("gitGraph.stashDelete"),
        onClick: async () => {
          const confirmed = await confirm(t("gitGraph.stashDeleteConfirm", { message: stash.message }), {
            title: t("gitGraph.stashDeleteTitle"),
            kind: "warning",
          });
          if (confirmed) stashDrop.mutate(stash.index);
        },
        disabled: anyMutating,
      },
    ]);
  };

  const refs = data?.pages[0]?.refs ?? [];
  const headSha = refs.find((r) => r.isHead)?.targetSha;
  // ponytail: WIP only anchors to an attached HEAD (via the isHead ref); a
  // detached HEAD has no such ref, so the pseudo-row is skipped in that case.
  const showWip = hasLocalChanges && !!headSha;

  const entries = useMemo<GraphRowEntry[]>(() => {
    // Stashes are placed by when they were made: before the first commit
    // that isn't newer than them (so a fresh stash rises to the top) — but
    // never below their base commit, since a child row must sit above its
    // parent. Stashes come newest-first (stash@{0}), which keeps that order
    // among stashes landing on the same spot.
    // ponytail: stashes whose base commit isn't in the currently loaded page
    // simply don't show up in the graph yet — the branches sidebar still
    // lists every stash regardless, so nothing is actually hidden.
    const commitIndex = new Map(commits.map((c, i) => [c.sha, i]));
    const stashesAt = new Map<number, StashInfo[]>();
    for (const stash of stashes ?? []) {
      const baseIndex = commitIndex.get(stash.baseSha);
      if (baseIndex === undefined) continue;
      const byDate = commits.findIndex((c) => c.committerDate <= stash.date);
      const at = Math.min(byDate === -1 ? baseIndex : byDate, baseIndex);
      stashesAt.set(at, [...(stashesAt.get(at) ?? []), stash]);
    }
    const commitEntries: GraphRowEntry[] = commits.flatMap((commit, i) => [
      ...(stashesAt.get(i) ?? []).map((stash): GraphRowEntry => ({ kind: "stash", stash })),
      { kind: "commit", commit } as GraphRowEntry,
    ]);
    return showWip ? [{ kind: "wip" }, ...commitEntries] : commitEntries;
  }, [commits, showWip, stashes]);

  const laneInputs = useMemo<LaneInput[]>(
    () =>
      entries.map((entry) => {
        if (entry.kind === "wip") return { sha: "wip", parents: headSha ? [headSha] : [] };
        if (entry.kind === "stash") return { sha: entry.stash.sha, parents: [entry.stash.baseSha] };
        return { sha: entry.commit.sha, parents: entry.commit.parents };
      }),
    [entries, headSha],
  );
  // A branch pinned from its pill's context menu gets lane 0 (leftmost) for
  // its whole first-parent line; remembered per repo. Resolved by name so it
  // follows the branch as it moves — local branch first, else its remote.
  const [pinnedBranch, setPinnedBranch] = usePersistedValue<string | null>(`graph.pinnedBranch.${repoId}`, null);
  const togglePin = (name: string) => setPinnedBranch(pinnedBranch === name ? null : name);
  const pinnedLine = useMemo(() => {
    const line = new Set<string>();
    if (!pinnedBranch) return line;
    const local = refs.find((r) => r.kind === "LocalBranch" && r.name === pinnedBranch);
    const remote = refs.find(
      (r) => r.kind === "RemoteBranch" && r.name.slice(r.name.indexOf("/") + 1) === pinnedBranch,
    );
    const tip = (local ?? remote)?.targetSha;
    if (!tip) return line;
    const firstParent = new Map(laneInputs.map((l) => [l.sha, l.parents[0]]));
    // The WIP row sits on top of HEAD, so it joins the pinned line too.
    let cur: string | undefined = showWip && tip === headSha ? "wip" : tip;
    while (cur && firstParent.has(cur) && !line.has(cur)) {
      line.add(cur);
      cur = firstParent.get(cur);
    }
    return line;
  }, [pinnedBranch, refs, laneInputs, showWip, headSha]);
  const rows = useMemo(() => computeLanes(laneInputs, pinnedLine), [laneInputs, pinnedLine]);
  // The checked-out branch's "line": HEAD (or the WIP row above it) followed
  // down its first-parent chain, like `git log --first-parent`. Merged-in
  // side branches aren't part of it.
  const headColor = HEAD_COLOR;
  const { headLine, firstParent } = useMemo(() => {
    const firstParent = new Map(laneInputs.map((l) => [l.sha, l.parents[0]]));
    const headLine = new Set<string>();
    let cur = showWip ? "wip" : headSha;
    while (cur && firstParent.has(cur) && !headLine.has(cur)) {
      headLine.add(cur);
      cur = firstParent.get(cur);
    }
    return { headLine, firstParent };
  }, [laneInputs, showWip, headSha]);
  const refsBySha = useMemo(() => {
    const map = new Map<string, RefInfo[]>();
    for (const ref of refs) {
      const list = map.get(ref.targetSha) ?? [];
      list.push(ref);
      map.set(ref.targetSha, list);
    }
    return map;
  }, [refs]);

  const laneCount = rows.reduce((max, r) => Math.max(max, r.lane + 1), 1);
  const svgWidth = laneCount * LANE_WIDTH;

  const scrollRef = useRef<HTMLDivElement>(null);
  const sentinelRef = useRef<HTMLDivElement>(null);

  // Only the rows in (or near) the viewport are rendered: every commit used to
  // be a DOM row plus SVG nodes, which made scrolling and hover fades crawl
  // on long histories.
  const [viewport, setViewport] = useState({ top: 0, height: 800 });
  useEffect(() => {
    const root = scrollRef.current;
    if (!root) return;
    let frame = 0;
    const read = () => {
      frame = 0;
      setViewport((v) =>
        v.top === root.scrollTop && v.height === root.clientHeight
          ? v
          : { top: root.scrollTop, height: root.clientHeight },
      );
    };
    const schedule = () => {
      if (!frame) frame = requestAnimationFrame(read);
    };
    read();
    root.addEventListener("scroll", schedule, { passive: true });
    const observer = new ResizeObserver(schedule);
    observer.observe(root);
    return () => {
      root.removeEventListener("scroll", schedule);
      observer.disconnect();
      cancelAnimationFrame(frame);
    };
  }, [isPending]);
  const { start: winStart, end: winEnd } = visibleRange(viewport.top, viewport.height, rows.length, ROW_HEIGHT, OVERSCAN_ROWS);
  const reach = useMemo(() => lineReach(rows), [rows]);
  const svgIndices = useMemo(() => svgRowIndices(reach, winStart, winEnd), [reach, winStart, winEnd]);

  // A commit selected from elsewhere (blame, file history, compare) may be
  // off screen: bring it into view. Rows already visible don't move.
  const selectedSha = selection?.kind === "commit" ? selection.commit.sha : null;
  const selectedIndex = useMemo(
    () => (selectedSha ? entries.findIndex((e) => e.kind === "commit" && e.commit.sha === selectedSha) : -1),
    [entries, selectedSha],
  );
  useEffect(() => {
    const root = scrollRef.current;
    if (!root || !selectedSha) return;
    if (selectedIndex === -1) return;
    const top = selectedIndex * ROW_HEIGHT;
    if (top < root.scrollTop || top + ROW_HEIGHT > root.scrollTop + root.clientHeight) {
      root.scrollTop = top - root.clientHeight / 2;
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedSha, selectedIndex !== -1]);

  // A search result older than the loaded pages: keep loading until it appears.
  useEffect(() => {
    if (selectedSha && selectedIndex === -1 && hasNextPage && !isFetchingNextPage) fetchNextPage();
  }, [selectedSha, selectedIndex, hasNextPage, isFetchingNextPage, fetchNextPage]);

  // Searches commits (only the visible graph: tabs stay mounted).
  useShortcut("searchCommits", (e) => {
    if (!isGitViewActive() || !scrollRef.current?.offsetParent) return;
    e.preventDefault();
    setSearchOpen(true);
  });

  useEffect(() => {
    const root = scrollRef.current;
    const target = sentinelRef.current;
    if (!root || !target || !hasNextPage) return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0].isIntersecting) fetchNextPage();
      },
      { root, rootMargin: "300px" },
    );
    observer.observe(target);
    return () => observer.disconnect();
  }, [hasNextPage, fetchNextPage]);

  if (isPending) return <p className="p-4 text-sm text-neutral-400">{t("gitGraph.loadingCommits")}</p>;

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <div className="flex shrink-0 items-stretch border-b border-neutral-800 text-[11px] uppercase text-neutral-500">
        <span className="px-2 py-1" style={{ width: branchColWidth }}>
          {t("gitGraph.colBranchTag")}
        </span>
        <ResizeHandle onDrag={(dx) => setBranchColWidth((w) => Math.max(80, w + dx))} />
        <span className="py-1" style={{ width: svgWidth }} />
        <span className="flex-1 py-1">{t("gitGraph.colMessage")}</span>
        <ResizeHandle onDrag={(dx) => setDateColWidth((w) => Math.max(80, w - dx))} />
        <span className="py-1 text-right" style={{ width: dateColWidth }}>
          {t("gitGraph.colDate")}
        </span>
        <span className="w-16 shrink-0 px-2 py-1 text-right">SHA</span>
      </div>
      <div ref={scrollRef} className="flex-1 overflow-auto">
        <div className="relative" style={{ height: rows.length * ROW_HEIGHT }}>
          <svg
            width={svgWidth}
            height={rows.length * ROW_HEIGHT}
            className="absolute top-0"
            style={{ left: branchColWidth, overflow: "visible" }}
          >
            {svgIndices.map((i) => {
              const row = rows[i];
              const y = i * ROW_HEIGHT + ROW_HEIGHT / 2;
              const x = row.lane * LANE_WIDTH + LANE_WIDTH / 2;
              const entry = entries[i];
              const nodeColor = palette[row.color];
              return (
                <g key={row.sha} opacity={entry.kind === "commit" ? dim(row.sha) : undefined}>
                  {row.parentConnections
                    .filter((c) => c.toRowIndex !== -1)
                    .map((c, ci) => {
                      const x2 = c.toLane * LANE_WIDTH + LANE_WIDTH / 2;
                      const y2 = c.toRowIndex * ROW_HEIGHT + ROW_HEIGHT / 2;
                      const sameLane = c.toLane === row.lane;
                      const onHeadLine =
                        headLine.has(row.sha) && rows[c.toRowIndex].sha === firstParent.get(row.sha);
                      // Hollow nodes (WIP, stashes) have no opaque fill to hide
                      // the line under — start it at the circle's edge instead
                      // of its center so it doesn't cut through the middle.
                      const isHollow = entry.kind === "wip" || entry.kind === "stash";

                      let d: string;
                      let color = palette[row.color];
                      if (sameLane) {
                        const y1 = isHollow ? y + Math.sign(y2 - y || 1) * (AVATAR_SIZE / 2) : y;
                        d = `M ${x} ${y1} L ${x2} ${y2}`;
                      } else if (c.kind === "merge") {
                        // Merges bend at the merge commit's own row, colored
                        // like the branch being merged in.
                        const x1 = isHollow ? x + Math.sign(x2 - x || 1) * (AVATAR_SIZE / 2) : x;
                        d = elbowPath(x1, y, x2, y, x2, y2);
                        color = palette[c.toLane % palette.length];
                      } else {
                        // Direct branch points flow with time: exit the parent
                        // (older, below) sideways at its own row height, then
                        // run straight up into the child's lane, arriving at
                        // the child (newer, above) from underneath.
                        const yEnd = isHollow ? y + Math.sign(y2 - y || 1) * (AVATAR_SIZE / 2) : y;
                        d = elbowPath(x2, y2, x, y2, x, yEnd);
                      }

                      return (
                        <path
                          key={ci}
                          d={d}
                          fill="none"
                          style={{ stroke: color }}
                          strokeWidth={onHeadLine ? 2.25 : 2}
                        />
                      );
                    })}
                  {/* Second half of the faint lead from the ref badge to its
                      commit (the branch cell draws the first). It starts a few
                      px left of the SVG to bridge the cell's right padding. */}
                  {entry.kind === "commit" && refsBySha.has(row.sha) && (
                    <line
                      x1={-4}
                      y1={y}
                      x2={x - AVATAR_SIZE / 2 - 1.5}
                      y2={y}
                      style={{ stroke: nodeColor }}
                      strokeWidth={1}
                      strokeOpacity={0.5}
                    />
                  )}
                  {entry.kind === "wip" ? (
                    <circle
                      cx={x}
                      cy={y}
                      r={AVATAR_SIZE / 2}
                      fill="none"
                      style={{ stroke: nodeColor }}
                      strokeWidth={2}
                      strokeDasharray="3,2"
                    />
                  ) : entry.kind === "stash" ? (
                    <>
                      <circle
                        cx={x}
                        cy={y}
                        r={AVATAR_SIZE / 2}
                        fill="none"
                        stroke={palette[row.color]}
                        strokeWidth={2}
                        strokeDasharray="3,2"
                      />
                      <Archive
                        x={x - AVATAR_SIZE / 2 + 2}
                        y={y - AVATAR_SIZE / 2 + 2}
                        width={AVATAR_SIZE - 4}
                        height={AVATAR_SIZE - 4}
                        color={palette[row.color]}
                        strokeWidth={2}
                      />
                    </>
                  ) : (
                    <>
                      {/* Soft disc in the line's color behind the avatar — shows
                          as a faint ring around it, and through avatars with
                          transparent pixels. */}
                      <circle
                        cx={x}
                        cy={y}
                        r={AVATAR_SIZE / 2 + 4.5}
                        style={{ fill: nodeColor }}
                        fillOpacity={0.18}
                      />
                      <circle
                        cx={x}
                        cy={y}
                        r={AVATAR_SIZE / 2 + 1.5}
                        fill="none"
                        style={{ stroke: nodeColor }}
                        strokeWidth={2}
                      />
                      {/* Filling a circle with a pattern instead of clip-path on
                          the <image>: WebKitGTK doesn't honor
                          clipPathUnits="objectBoundingBox" on <image> elements,
                          which rendered the avatar as an unclipped square. */}
                      <defs>
                        <pattern id={`gk-avatar-${row.sha}`} patternUnits="objectBoundingBox" width={1} height={1}>
                          <image
                            href={gravatarUrl(entry.commit.authorEmail, 32)}
                            x={0}
                            y={0}
                            width={AVATAR_SIZE}
                            height={AVATAR_SIZE}
                            preserveAspectRatio="xMidYMid slice"
                          />
                        </pattern>
                      </defs>
                      <circle cx={x} cy={y} r={AVATAR_SIZE / 2} fill={`url(#gk-avatar-${row.sha})`} />
                    </>
                  )}
                  {entry.kind !== "wip" && (
                    <circle
                      cx={x}
                      cy={y}
                      r={AVATAR_SIZE / 2 + 2}
                      fill="transparent"
                      onContextMenu={(e) =>
                        entry.kind === "stash"
                          ? handleStashContextMenu(e, entry.stash)
                          : handleCommitContextMenu(e, entry.commit)
                      }
                    />
                  )}
                </g>
              );
            })}
          </svg>

          <ul
            className="select-none"
            style={{ paddingTop: winStart * ROW_HEIGHT, paddingBottom: (rows.length - winEnd) * ROW_HEIGHT }}
            onMouseDown={(e) => e.shiftKey && e.preventDefault()}
          >
            {entries.slice(winStart, winEnd).map((entry, k) => {
              const i = winStart + k;
              if (entry.kind === "wip") {
                return (
                  <li
                    key="wip"
                    onClick={() => onSelectionChange(selection?.kind === "wip" ? null : { kind: "wip" })}
                    className={`flex cursor-pointer items-center gap-2 text-sm text-neutral-200 ${
                      selection?.kind === "wip" ? "bg-neutral-800" : "hover:bg-neutral-900"
                    }`}
                    style={{ height: ROW_HEIGHT }}
                  >
                    <span className="shrink-0" style={{ width: branchColWidth }} />
                    <span className="shrink-0" style={{ width: svgWidth }} />
                    <span className="flex min-w-0 flex-1 items-center gap-1.5 truncate italic text-neutral-400">
                      <Pencil size={12} className="shrink-0" /> {t("gitGraph.uncommitted")}
                    </span>
                    <span className="shrink-0" style={{ width: dateColWidth }} />
                    <span className="w-16 shrink-0 px-2" />
                  </li>
                );
              }

              if (entry.kind === "stash") {
                const stash = entry.stash;
                const isSelected = selection?.kind === "stash" && selection.stash.sha === stash.sha;
                return (
                  <li
                    key={stash.sha}
                    onClick={() =>
                      onSelectionChange(isSelected ? null : { kind: "stash", stash })
                    }
                    onContextMenu={(e) => handleStashContextMenu(e, stash)}
                    className={`flex cursor-pointer items-center gap-2 text-sm text-neutral-200 ${
                      isSelected ? "bg-neutral-800" : "hover:bg-neutral-900"
                    }`}
                    style={{ height: ROW_HEIGHT }}
                  >
                    <span className="shrink-0" style={{ width: branchColWidth }} />
                    <span className="shrink-0" style={{ width: svgWidth }} />
                    <span className="flex min-w-0 flex-1 items-center gap-1.5 truncate italic text-neutral-400">
                      <Archive size={12} className="shrink-0" /> {stash.message}
                    </span>
                    <span className="shrink-0 text-right text-xs text-neutral-500" style={{ width: dateColWidth }}>
                      {formatDateTime(stash.date)}
                    </span>
                    <span className="w-16 shrink-0 px-2 text-right font-mono text-xs text-neutral-500">
                      {stash.sha.slice(0, 7)}
                    </span>
                  </li>
                );
              }

              const commit = entry.commit;
              const groups = groupRefs(refsBySha.get(commit.sha) ?? []);
              const laneColor = palette[rows[i].color];
              const bandStart = branchColWidth + rows[i].lane * LANE_WIDTH + LANE_WIDTH / 2;
              return (
                <li
                  key={commit.sha}
                  onClick={(e) => handleCommitClick(e, commit)}
                  onContextMenu={(e) => handleCommitContextMenu(e, commit)}
                  className={`flex cursor-pointer items-center gap-2 text-sm text-neutral-200 ${
                    selectedShas.has(commit.sha) ? "bg-neutral-800" : "hover:bg-neutral-900"
                  }`}
                  style={{
                    height: ROW_HEIGHT,
                    opacity: dim(commit.sha),
                    // Lane-colored band from the node to the row's right edge.
                    backgroundImage: `linear-gradient(to right, transparent ${bandStart}px, color-mix(in srgb, ${laneColor} 12%, transparent) ${bandStart}px)`,
                  }}
                >
                  <BranchCell
                    groups={groups}
                    repoId={repoId}
                    targetSha={commit.sha}
                    localBranchNames={localBranchNames}
                    currentBranch={currentBranch}
                    width={branchColWidth}
                    hosting={hosting}
                    openMrByBranch={openMrByBranch}
                    onCreateMr={(source, target) => setCreateMr({ source, target })}
                    headColor={headColor}
                    pinnedBranch={pinnedBranch}
                    onTogglePin={togglePin}
                    laneColor={laneColor}
                  />
                  <span className="shrink-0" style={{ width: svgWidth }} />
                  <span className="min-w-0 flex-1 truncate">{commit.summary}</span>
                  <span
                    className="shrink-0 text-right text-xs text-neutral-500"
                    style={{ width: dateColWidth }}
                  >
                    {formatDateTime(commit.authorDate)}
                  </span>
                  <span className="w-16 shrink-0 px-2 text-right font-mono text-xs text-neutral-500">
                    {commit.shortSha}
                  </span>
                </li>
              );
            })}
          </ul>
        </div>
        {hasNextPage && <div ref={sentinelRef} className="h-px" />}
        {isFetchingNextPage && (
          <p className="py-2 text-center text-xs text-neutral-500">{t("gitGraph.loadingMore")}</p>
        )}
      </div>
      {commitMenu}
      <BranchDragGhost />
      {createBranchAt && (
        <CreateBranchModal repoId={repoId} startSha={createBranchAt} onClose={() => setCreateBranchAt(null)} />
      )}
      {createTagAt && (
        <CreateTagModal repoId={repoId} startSha={createTagAt} onClose={() => setCreateTagAt(null)} />
      )}
      {searchOpen && (
        <CommitSearchPalette
          repoId={repoId}
          onClose={() => setSearchOpen(false)}
          onPick={(commit) => onSelectionChange({ kind: "commit", commit })}
        />
      )}
      {squashCommits && (
        <SquashCommitsModal repoId={repoId} commits={squashCommits} onClose={() => setSquashCommits(null)} />
      )}
      {rebaseFrom && (
        <InteractiveRebaseModal repoId={repoId} fromSha={rebaseFrom} onClose={() => setRebaseFrom(null)} />
      )}
      {createMr && hosting && (
        <CreateMergeRequestModal
          repoId={repoId}
          hosting={hosting}
          sourceBranch={createMr.source}
          initialTargetBranch={createMr.target}
          onClose={() => setCreateMr(null)}
        />
      )}
    </div>
  );
}
