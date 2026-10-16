import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowDown, ArrowUp, GripVertical, Loader2 } from "lucide-react";
import { memo, useCallback, useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import { Modal } from "@/components/Modal";
import { Select } from "@/components/Select";
import { useRebaseStore } from "@/features/rebase/store";
import { queryKeys } from "@/lib/query-keys";
import { errorMessage } from "@/lib/tauri-client";
import { getRebasePlan, startInteractiveRebase } from "./api";
import {
  TODO_ACTIONS,
  actionForKey,
  initialRows,
  isUnchanged,
  moveRow,
  toItems,
  validateRows,
  type TodoAction,
  type TodoRow,
} from "./todo";
import { useT } from "../../../lib/i18n/react";

const ACTION_OPTIONS = TODO_ACTIONS.map((a) => ({ value: a, label: a }));

const ACTION_TONE: Record<TodoAction, string> = {
  pick: "",
  reword: "border-l-sky-500",
  squash: "border-l-violet-500",
  fixup: "border-l-violet-500",
  drop: "border-l-red-600 opacity-50",
};

/** `git rebase -i` from `fromSha` (included) up to HEAD: reorder by dragging,
 * pick an action per commit, then run. Stops land in the rebase conflict panel. */
export function InteractiveRebaseModal({
  repoId,
  fromSha,
  onClose,
}: {
  repoId: string;
  fromSha: string;
  onClose: () => void;
}) {
  const t = useT();
  const queryClient = useQueryClient();
  const setRebaseStatus = useRebaseStore((s) => s.setStatus);
  const { data: plan, error, isPending } = useQuery({
    queryKey: ["rebasePlan", repoId, fromSha],
    queryFn: () => getRebasePlan(repoId, fromSha),
    // HEAD may move between openings: never reuse an old plan (gcTime 0).
    // But never refetch while open either — that would reset the user's edits
    // (the backend refuses a list that no longer matches the branch).
    staleTime: Infinity,
    gcTime: 0,
    refetchOnWindowFocus: false,
    refetchOnReconnect: false,
  });
  const [rows, setRows] = useState<TodoRow[]>([]);
  // Ref, not state: rows' drop handler stays stable so memoized rows don't re-render.
  const dragFrom = useRef<number | null>(null);
  const [dragging, setDragging] = useState(false);
  const [running, setRunning] = useState(false);

  useEffect(() => {
    if (plan) setRows(initialRows(plan.commits));
  }, [plan]);

  const setAction = useCallback(
    (index: number, action: TodoAction) =>
      setRows((rs) => rs.map((r, i) => (i === index ? { ...r, action } : r))),
    [],
  );
  const setMessage = useCallback(
    (index: number, message: string) =>
      setRows((rs) => rs.map((r, i) => (i === index ? { ...r, message } : r))),
    [],
  );
  const move = useCallback((from: number, to: number) => setRows((rs) => moveRow(rs, from, to)), []);
  const dragStart = useCallback((index: number | null) => {
    dragFrom.current = index;
    setDragging(index !== null);
  }, []);
  const dropAt = useCallback(
    (to: number) => {
      if (dragFrom.current !== null) move(dragFrom.current, to);
      dragStart(null);
    },
    [move, dragStart],
  );

  const invalid = validateRows(rows);
  const unchanged = plan ? isUnchanged(rows, plan.commits) : true;

  const run = async () => {
    if (!plan || invalid) return;
    setRunning(true);
    try {
      const status = await startInteractiveRebase(repoId, plan.upstream, toItems(rows));
      for (const key of [
        queryKeys.commitLog(repoId),
        queryKeys.headCommit(repoId),
        queryKeys.branches(repoId),
        queryKeys.repoStatus(repoId),
        queryKeys.workingStatus(repoId),
        // --autostash parks uncommitted changes in a stash while it runs
        queryKeys.stashes(repoId),
        queryKeys.undoStatus(repoId),
      ]) {
        queryClient.invalidateQueries({ queryKey: key });
      }
      if (status.state === "Complete") {
        toast.success(t("gitInteractiveRebase.done"));
      } else {
        setRebaseStatus(status);
        toast.warning(
          status.conflicts.length > 0
            ? t("gitInteractiveRebase.conflicts")
            : t("gitInteractiveRebase.stopped"),
        );
      }
      onClose();
    } catch (e) {
      toast.error(errorMessage(e));
    } finally {
      setRunning(false);
    }
  };

  return (
    <Modal onClose={onClose} align="top" width="w-[760px]" dismissible={!running}>
      <div className="flex max-h-[75vh] flex-col text-sm">
        <div className="shrink-0 border-b border-neutral-800 px-3 py-2">
          <span className="font-semibold text-neutral-200">{t("gitInteractiveRebase.title")}</span>
          <p className="mt-0.5 text-xs text-neutral-500">
            {t("gitInteractiveRebase.hint")}
          </p>
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto">
          {isPending ? (
            <div className="flex items-center gap-2 p-4 text-neutral-400">
              <Loader2 size={14} className="animate-spin" /> {t("gitInteractiveRebase.loadingCommits")}
            </div>
          ) : error ? (
            <p className="p-4 text-red-400">{errorMessage(error)}</p>
          ) : (
            <ol>
              {rows.map((row, i) => (
                <Row
                  key={row.commit.sha}
                  row={row}
                  index={i}
                  last={i === rows.length - 1}
                  dragging={dragging}
                  onAction={setAction}
                  onMessage={setMessage}
                  onMove={move}
                  onDragStart={dragStart}
                  onDropAt={dropAt}
                />
              ))}
            </ol>
          )}
        </div>

        <div className="flex shrink-0 items-center justify-between gap-3 border-t border-neutral-800 px-3 py-2">
          <span className="min-w-0 truncate text-xs text-amber-400" title={invalid ?? undefined}>
            {rows.length > 0 && invalid}
          </span>
          <div className="flex shrink-0 gap-2">
            <button
              onClick={onClose}
              disabled={running}
              className="rounded border border-neutral-700 px-3 py-1.5 text-sm text-neutral-300 hover:bg-neutral-900 disabled:opacity-50"
            >
              {t("gitInteractiveRebase.cancel")}
            </button>
            <button
              onClick={run}
              disabled={running || !plan || !!invalid || unchanged}
              title={unchanged ? t("gitInteractiveRebase.unchanged") : undefined}
              className="flex items-center gap-1.5 rounded bg-emerald-700 px-3 py-1.5 text-sm text-emerald-50 hover:bg-emerald-800 disabled:opacity-50"
            >
              {running && <Loader2 size={13} className="animate-spin" />} {t("gitInteractiveRebase.start")}
            </button>
          </div>
        </div>
      </div>
    </Modal>
  );
}

/** Memoized: editing one row's message doesn't re-render the others. */
const Row = memo(function Row({
  row,
  index,
  last,
  dragging,
  onAction,
  onMessage,
  onMove,
  onDragStart,
  onDropAt,
}: {
  row: TodoRow;
  index: number;
  last: boolean;
  dragging: boolean;
  onAction: (index: number, action: TodoAction) => void;
  onMessage: (index: number, message: string) => void;
  onMove: (from: number, to: number) => void;
  onDragStart: (index: number | null) => void;
  onDropAt: (index: number) => void;
}) {
  const t = useT();
  const [over, setOver] = useState(false);
  // Only the grip starts a drag, so text in the reword box stays selectable.
  const [armed, setArmed] = useState(false);
  const { commit, action } = row;

  return (
    <li
      tabIndex={0}
      draggable={armed}
      onDragStart={(e) => {
        e.dataTransfer.effectAllowed = "move";
        onDragStart(index);
      }}
      onDragEnd={() => {
        setArmed(false);
        onDragStart(null);
      }}
      onDragOver={(e) => {
        if (!dragging) return;
        e.preventDefault();
        setOver(true);
      }}
      onDragLeave={() => setOver(false)}
      onDrop={(e) => {
        e.preventDefault();
        setOver(false);
        onDropAt(index);
      }}
      onKeyDown={(e) => {
        if (e.target !== e.currentTarget || e.ctrlKey || e.metaKey || e.altKey) return;
        const next = actionForKey(e.key);
        if (next) {
          e.preventDefault();
          onAction(index, next);
        }
      }}
      className={`border-b border-l-2 border-b-neutral-800 border-l-transparent px-2 py-1.5 outline-none focus:bg-neutral-800/60 ${
        ACTION_TONE[action]
      } ${over ? "bg-emerald-950/50" : ""}`}
    >
      <div className="flex items-center gap-2">
        <span
          onMouseDown={() => setArmed(true)}
          onMouseUp={() => setArmed(false)}
          className="shrink-0 cursor-grab text-neutral-600 hover:text-neutral-400"
        >
          <GripVertical size={14} />
        </span>
        <div className="w-24 shrink-0">
          <Select value={action} options={ACTION_OPTIONS} onChange={(v) => onAction(index, v as TodoAction)} />
        </div>
        <span className="shrink-0 font-mono text-xs text-neutral-500">{commit.shortSha}</span>
        <span className={`min-w-0 flex-1 truncate ${action === "drop" ? "line-through" : ""}`} title={commit.summary}>
          {commit.summary}
        </span>
        <span className="hidden shrink-0 truncate text-xs text-neutral-500 sm:block">{commit.authorName}</span>
        <button
          onClick={() => onMove(index, index - 1)}
          disabled={index === 0}
          title={t("gitInteractiveRebase.moveUp")}
          className="shrink-0 rounded p-0.5 text-neutral-400 hover:bg-neutral-700 disabled:opacity-30"
        >
          <ArrowUp size={13} />
        </button>
        <button
          onClick={() => onMove(index, index + 1)}
          disabled={last}
          title={t("gitInteractiveRebase.moveDown")}
          className="shrink-0 rounded p-0.5 text-neutral-400 hover:bg-neutral-700 disabled:opacity-30"
        >
          <ArrowDown size={13} />
        </button>
      </div>
      {action === "reword" && (
        <textarea
          autoFocus
          value={row.message}
          onChange={(e) => onMessage(index, e.currentTarget.value)}
          rows={Math.min(8, Math.max(2, row.message.split("\n").length))}
          className="mt-1.5 ml-[124px] block w-[calc(100%-124px)] resize-y rounded bg-neutral-800 px-2 py-1 font-mono text-xs text-neutral-100"
        />
      )}
    </li>
  );
});
