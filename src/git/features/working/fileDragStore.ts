import { create } from "zustand";
import type { WorkingSide } from "./types";

/** Backs dragging a file between the staged and unstaged lists. Mouse events,
 * not native HTML5 DnD (see commit-graph/branchDragStore for why). */
type FileDragStore = {
  drag: { path: string; from: WorkingSide } | null;
  /** List under the pointer while dragging. */
  hoverSide: WorkingSide | null;
  position: { x: number; y: number };
  start: (path: string, from: WorkingSide, x: number, y: number) => void;
  move: (x: number, y: number) => void;
  setHover: (side: WorkingSide | null) => void;
  stop: () => void;
};

export const useFileDragStore = create<FileDragStore>((set) => ({
  drag: null,
  hoverSide: null,
  position: { x: 0, y: 0 },
  start: (path, from, x, y) => set({ drag: { path, from }, position: { x, y }, hoverSide: null }),
  move: (x, y) => set({ position: { x, y } }),
  setHover: (hoverSide) => set({ hoverSide }),
  stop: () => set({ drag: null, hoverSide: null }),
}));

/** What dropping a file dragged from `from` onto `over` does. */
export function dropAction(from: WorkingSide, over: WorkingSide | null): "stage" | "unstage" | null {
  if (!over || over === from) return null;
  return over === "staged" ? "stage" : "unstage";
}
