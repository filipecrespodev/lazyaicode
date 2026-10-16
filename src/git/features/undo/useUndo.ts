import { useIsMutating, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { t } from "../../../lib/i18n/react";
import { queryKeys } from "@/lib/query-keys";
import { errorMessage } from "@/lib/tauri-client";
import { getUndoStatus, redoLastAction, undoLastAction } from "./api";
import { isGitViewActive } from "@/embed";
import { useShortcut } from "@/features/shortcuts/useShortcuts";

/** Typing in a field (or the terminal, which is an xterm textarea) keeps the
 * native Ctrl+Z instead of undoing a git action. */
function isTextInput(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName);
}

/** Undo/redo of the last local git action (see `git/undo.rs`), with
 * Ctrl+Z / Ctrl+Shift+Z (or Ctrl+Y) while `active`. */
export function useUndo(repoId: string, active: boolean) {
  const queryClient = useQueryClient();
  const anyMutating = useIsMutating() > 0;

  const { data: status } = useQuery({
    queryKey: queryKeys.undoStatus(repoId),
    queryFn: () => getUndoStatus(repoId),
    // Mutations refresh this right away (see query-client.ts); the interval
    // covers actions that call the backend directly, e.g. conflict resolution.
    // Background tabs don't need it until they're shown again.
    refetchInterval: active ? 5_000 : false,
  });

  // Anything about the repo may have changed: refresh every query of it.
  const refreshRepo = () =>
    queryClient.invalidateQueries({ predicate: (q) => q.queryKey.includes(repoId) });

  const undo = useMutation({
    mutationFn: () => undoLastAction(repoId),
    onSuccess: (label) => toast.success(t("gitUndo.undone", { label })),
    onError: (e) => toast.error(errorMessage(e)),
    onSettled: refreshRepo,
  });
  const redo = useMutation({
    mutationFn: () => redoLastAction(repoId),
    onSuccess: (label) => toast.success(t("gitUndo.redone", { label })),
    onError: (e) => toast.error(errorMessage(e)),
    onSettled: refreshRepo,
  });

  const canUndo = !!status?.undo && !anyMutating;
  const canRedo = !!status?.redo && !anyMutating;

  // Rebindable in Settings → Shortcuts (defaults Ctrl+Z / Ctrl+Shift+Z or Ctrl+Y).
  const onKey = (run: () => void) => (e: KeyboardEvent) => {
    if (!isGitViewActive() || isTextInput(e.target)) return;
    e.preventDefault();
    run();
  };
  useShortcut("undo", onKey(() => canUndo && undo.mutate()), active);
  useShortcut("redo", onKey(() => canRedo && redo.mutate()), active);

  return { status, canUndo, canRedo, undo: () => undo.mutate(), redo: () => redo.mutate() };
}
