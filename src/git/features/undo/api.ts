import { call } from "@/lib/tauri-client";

export type UndoStatus = {
  /** Label of the action Undo would revert, e.g. `Commit "fix login"`. */
  undo: string | null;
  /** Label of the action Redo would re-apply. */
  redo: string | null;
};

export const getUndoStatus = (repoId: string) => call<UndoStatus>("get_undo_status", { repoId });

/** Reverts the last local action and returns its label. */
export const undoLastAction = (repoId: string) => call<string>("undo_last_action", { repoId });

/** Re-applies the last undone action and returns its label. */
export const redoLastAction = (repoId: string) => call<string>("redo_last_action", { repoId });
