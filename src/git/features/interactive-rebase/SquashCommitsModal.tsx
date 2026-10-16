import { useQueryClient } from "@tanstack/react-query";
import { Loader2 } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { Modal } from "@/components/Modal";
import type { CommitNode } from "@/features/commit-graph/types";
import { useRebaseStore } from "@/features/rebase/store";
import { queryKeys } from "@/lib/query-keys";
import { errorMessage } from "@/lib/tauri-client";
import { getRebasePlan, startInteractiveRebase } from "./api";
import { combinedMessage, squashItems } from "./squash";
import { useT } from "../../../lib/i18n/react";

/** Squashes `commits` (oldest first, adjacent, linear) into one with an editable message. */
export function SquashCommitsModal({
  repoId,
  commits,
  onClose,
}: {
  repoId: string;
  commits: CommitNode[];
  onClose: () => void;
}) {
  const t = useT();
  const queryClient = useQueryClient();
  const setRebaseStatus = useRebaseStore((s) => s.setStatus);
  const [message, setMessage] = useState(() => combinedMessage(commits));
  const [running, setRunning] = useState(false);

  const run = async () => {
    setRunning(true);
    try {
      const plan = await getRebasePlan(repoId, commits[0].sha);
      const status = await startInteractiveRebase(
        repoId,
        plan.upstream,
        squashItems(plan.commits, new Set(commits.map((c) => c.sha)), message.trim()),
      );
      for (const key of [
        queryKeys.commitLog(repoId),
        queryKeys.headCommit(repoId),
        queryKeys.branches(repoId),
        queryKeys.repoStatus(repoId),
        queryKeys.workingStatus(repoId),
        queryKeys.stashes(repoId),
        queryKeys.undoStatus(repoId),
      ]) {
        queryClient.invalidateQueries({ queryKey: key });
      }
      if (status.state === "Complete") {
        toast.success(t("gitInteractiveRebase.squashDone", { count: commits.length }));
      } else {
        setRebaseStatus(status);
        toast.warning(
          status.conflicts.length > 0
            ? t("gitInteractiveRebase.squashConflicts")
            : t("gitInteractiveRebase.squashStopped"),
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
    <Modal onClose={onClose} align="top" width="w-[620px]" dismissible={!running}>
      <div className="flex flex-col text-sm">
        <div className="border-b border-neutral-800 px-3 py-2">
          <span className="font-semibold text-neutral-200">{t("gitInteractiveRebase.squashTitle", { count: commits.length })}</span>
          <p className="mt-0.5 text-xs text-neutral-500">
            {t("gitInteractiveRebase.squashHint")}
          </p>
        </div>
        <textarea
          value={message}
          onChange={(e) => setMessage(e.currentTarget.value)}
          rows={10}
          autoFocus
          className="m-3 resize-none rounded bg-neutral-950 px-2 py-1.5 font-mono text-xs text-neutral-100 outline-none"
        />
        <div className="flex justify-end gap-2 border-t border-neutral-800 px-3 py-2">
          <button
            onClick={onClose}
            disabled={running}
            className="rounded border border-neutral-700 px-3 py-1.5 text-neutral-300 hover:bg-neutral-900 disabled:opacity-50"
          >
            {t("gitInteractiveRebase.cancel")}
          </button>
          <button
            onClick={run}
            disabled={running || !message.trim()}
            className="flex items-center gap-1.5 rounded bg-emerald-700 px-3 py-1.5 text-emerald-50 hover:bg-emerald-800 disabled:opacity-50"
          >
            {running && <Loader2 size={13} className="animate-spin" />} {t("gitInteractiveRebase.squash")}
          </button>
        </div>
      </div>
    </Modal>
  );
}
