import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Loader2 } from "lucide-react";
import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { EventMap, HunkData } from "react-diff-view";
import { toast } from "sonner";
import {
  changeKey,
  changeKeysInOrder,
  changeTexts,
  nextSelection,
  splitIntoChangeBlocks,
  toLineSelection,
  type LineSelection,
} from "@/features/diff-viewer/hunkSelection";
import { confirm } from "@/lib/confirm-store";
import { queryKeys } from "@/lib/query-keys";
import { errorMessage } from "@/lib/tauri-client";
import { applyLines, type LineAction } from "./api";
import type { WorkingSide } from "./types";
import { t, useT } from "../../../lib/i18n/react";

const EMPTY: ReadonlySet<string> = new Set();

/**
 * Hunk and line staging for a working-copy file open in the diff viewer.
 * Selection lives here as a Set; the diff rows only see whether *they* are
 * selected (react-diff-view memoizes each row), and the click handlers are
 * stable, so toggling a line re-renders that line, not the whole file.
 */
export function useLineStaging(
  repoId: string,
  path: string,
  patch: string,
  side: WorkingSide | null,
  hunks: HunkData[] | null,
) {
  const queryClient = useQueryClient();
  const [selection, setSelection] = useState<ReadonlySet<string>>(EMPTY);
  const anchorRef = useRef<string | null>(null);

  const blocks = useMemo(() => (side && hunks ? splitIntoChangeBlocks(hunks) : null), [side, hunks]);
  const displayHunks = useMemo(() => blocks?.map((b) => b.hunk) ?? null, [blocks]);
  const ordered = useMemo(() => (hunks ? changeKeysInOrder(hunks) : []), [hunks]);
  const orderedRef = useRef(ordered);
  orderedRef.current = ordered;
  const texts = useMemo(() => (hunks ? changeTexts(hunks) : new Map<string, string>()), [hunks]);
  const textsRef = useRef(texts);
  textsRef.current = texts;

  // Line numbers refer to one specific diff: start over whenever it changes.
  useEffect(() => {
    setSelection(EMPTY);
    anchorRef.current = null;
  }, [path, patch, side]);

  const onLineClick = useCallback<NonNullable<EventMap["onClick"]>>(({ change }, e) => {
    if (!change || change.type === "normal") return;
    // Dragging across lines to copy text shouldn't toggle them.
    if (window.getSelection()?.toString()) return;
    const key = changeKey(change);
    const shift = e.shiftKey;
    setSelection((prev) => nextSelection(prev, orderedRef.current, key, anchorRef.current, shift));
    anchorRef.current = key;
  }, []);
  const lineEvents = useMemo<EventMap | undefined>(
    () => (side ? { onClick: onLineClick } : undefined),
    [side, onLineClick],
  );
  const selectedChanges = useMemo(() => [...selection], [selection]);

  const mutation = useMutation({
    mutationFn: ({ action, selection }: { action: LineAction; selection: LineSelection }) =>
      applyLines(repoId, path, action, selection),
    onSuccess: () => {
      setSelection(EMPTY);
      anchorRef.current = null;
    },
    onError: (e) => toast.error(errorMessage(e)),
    // Also on failure: the usual one is a file that changed on disk, and the
    // refreshed diff is what the user needs to pick again from.
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.workingStatus(repoId) });
      queryClient.invalidateQueries({ queryKey: ["workdirDiff", repoId] });
      queryClient.invalidateQueries({ queryKey: queryKeys.repoStatus(repoId) });
    },
  });
  const { mutate } = mutation;

  const run = useCallback(
    async (action: LineAction, keys: string[]) => {
      if (keys.length === 0) return;
      // Captured now, from the diff the user picked from: it may refresh
      // while the confirm dialog is open.
      const selection = toLineSelection(keys, textsRef.current);
      if (action === "discard") {
        const confirmed = await confirm(
          t(keys.length === 1 ? "gitWorking.discardLinesConfirmOne" : "gitWorking.discardLinesConfirmOther", {
            count: keys.length,
            path,
          }),
          { title: t("gitWorking.discardLines"), kind: "warning" },
        );
        if (!confirmed) return;
      }
      mutate({ action, selection });
    },
    [mutate, path],
  );

  return {
    blocks,
    displayHunks,
    lineEvents,
    selectedChanges,
    run,
    clear: () => setSelection(EMPTY),
    pending: mutation.isPending,
  };
}

const btn =
  "flex items-center gap-1 rounded px-1.5 py-0.5 text-[11px] disabled:opacity-50 bg-neutral-800 hover:bg-neutral-700";

/** Actions shown above each hunk of a working-copy diff. */
export const HunkActions = memo(function HunkActions({
  side,
  label,
  changeKeys,
  disabled,
  onRun,
}: {
  side: WorkingSide;
  label: string;
  changeKeys: string[];
  disabled: boolean;
  onRun: (action: LineAction, keys: string[]) => void;
}) {
  const t = useT();
  return (
    <div className="flex items-center justify-between gap-2 px-2 py-1 font-mono text-[11px] text-neutral-500">
      <span className="truncate">{label}</span>
      <span className="flex shrink-0 items-center gap-1 font-sans">
        {side === "unstaged" ? (
          <>
            <button disabled={disabled} onClick={() => onRun("discard", changeKeys)} className={`${btn} text-red-300`}>
              {t("gitWorking.discardHunk")}
            </button>
            <button disabled={disabled} onClick={() => onRun("stage", changeKeys)} className={`${btn} text-emerald-300`}>
              {t("gitWorking.stageHunk")}
            </button>
          </>
        ) : (
          <button disabled={disabled} onClick={() => onRun("unstage", changeKeys)} className={btn}>
            {t("gitWorking.unstageHunk")}
          </button>
        )}
      </span>
    </div>
  );
});

/** Bar above the diff: hint while nothing is selected, line actions after. */
export function LineSelectionBar({
  side,
  selected,
  pending,
  onRun,
  onClear,
}: {
  side: WorkingSide;
  selected: string[];
  pending: boolean;
  onRun: (action: LineAction, keys: string[]) => void;
  onClear: () => void;
}) {
  const t = useT();
  const n = selected.length;
  return (
    <div className="flex shrink-0 items-center justify-between gap-2 border-b border-neutral-800 bg-neutral-950 px-3 py-1 text-xs text-neutral-400">
      <span className="flex items-center gap-1.5">
        {pending && <Loader2 size={11} className="animate-spin" />}
        {n === 0
          ? t("gitWorking.lineSelectHint")
          : t(n === 1 ? "gitWorking.linesSelectedOne" : "gitWorking.linesSelectedOther", { count: n })}
      </span>
      {n > 0 && (
        <span className="flex items-center gap-1">
          <button onClick={onClear} className={btn}>
            {t("gitWorking.clear")}
          </button>
          {side === "unstaged" ? (
            <>
              <button disabled={pending} onClick={() => onRun("discard", selected)} className={`${btn} text-red-300`}>
                {t("gitWorking.discardLines")}
              </button>
              <button disabled={pending} onClick={() => onRun("stage", selected)} className={`${btn} text-emerald-300`}>
                {t("gitWorking.stageLines")}
              </button>
            </>
          ) : (
            <button disabled={pending} onClick={() => onRun("unstage", selected)} className={btn}>
              {t("gitWorking.unstageLines")}
            </button>
          )}
        </span>
      )}
    </div>
  );
}
