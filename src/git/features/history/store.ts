import { create } from "zustand";
import { t } from "../../../lib/i18n/react";

/** `sha: null` means "at HEAD", resolved by the view when it loads. */
export type HistoryView =
  | { kind: "fileHistory"; repoId: string; path: string; sha: string | null }
  | { kind: "blame"; repoId: string; path: string; sha: string | null }
  | { kind: "compare"; repoId: string; a: string; b: string };

type HistoryViewStore = {
  /** Views opened from one another (history → blame) stack, so Back returns. */
  stack: HistoryView[];
  open: (view: HistoryView) => void;
  back: () => void;
  close: () => void;
};

/** Full-screen history views, opened from anywhere (file lists, diff viewer,
 * branch menus) without threading callbacks through every panel. */
export const useHistoryViewStore = create<HistoryViewStore>((set) => ({
  stack: [],
  // A view of another repo starts a fresh stack: Back never crosses tabs.
  open: (view) =>
    set((s) => ({ stack: s.stack[s.stack.length - 1]?.repoId === view.repoId ? [...s.stack, view] : [view] })),
  back: () => set((s) => ({ stack: s.stack.slice(0, -1) })),
  close: () => set({ stack: [] }),
}));

export const openFileHistory = (repoId: string, path: string, sha: string | null) =>
  useHistoryViewStore.getState().open({ kind: "fileHistory", repoId, path, sha });

export const openBlame = (repoId: string, path: string, sha: string | null) =>
  useHistoryViewStore.getState().open({ kind: "blame", repoId, path, sha });

export const openCompare = (repoId: string, a: string, b: string) =>
  useHistoryViewStore.getState().open({ kind: "compare", repoId, a, b });

/** Context-menu items for a file shown at `sha` (null: working copy / HEAD). */
export const fileHistoryMenuItems = (repoId: string, path: string, sha: string | null) => [
  { label: t("gitHistory.fileHistory"), onClick: () => openFileHistory(repoId, path, sha) },
  { label: sha ? t("gitHistory.blameAtCommit") : t("gitHistory.blame"), onClick: () => openBlame(repoId, path, sha) },
];
