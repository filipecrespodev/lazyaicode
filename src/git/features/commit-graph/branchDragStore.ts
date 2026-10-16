import { create } from "zustand";

/** Backs the branch-pill drag-and-drop gesture. Deliberately NOT the native
 * HTML5 drag API — WebKitGTK (Tauri's Linux webview) renders that API's drag
 * ghost as its own toplevel window, which some window managers list as a
 * phantom taskbar entry. Plain mouse events sidestep that entirely. */
type BranchDragStore = {
  draggingBranch: string | null;
  position: { x: number; y: number };
  start: (name: string, x: number, y: number) => void;
  move: (x: number, y: number) => void;
  stop: () => void;
};

export const useBranchDragStore = create<BranchDragStore>((set) => ({
  draggingBranch: null,
  position: { x: 0, y: 0 },
  start: (name, x, y) => set({ draggingBranch: name, position: { x, y } }),
  move: (x, y) => set({ position: { x, y } }),
  stop: () => set({ draggingBranch: null }),
}));
