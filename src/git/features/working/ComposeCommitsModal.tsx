import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Loader2 } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { Modal } from "@/components/Modal";
import { applyComposedCommits, composeCommits, type ComposedCommit } from "@/features/ai/api";
import { useAiSettings } from "@/features/ai/useAiSettings";
import { queryKeys } from "@/lib/query-keys";
import { errorMessage } from "@/lib/tauri-client";
import { toastCommitError } from "./commit-error";
import { t as tr, useT } from "../../../lib/i18n/react";

/** "src/a.ts#0", "src/a.ts#2" → [["src/a.ts", "(2 hunks)"]]; whole files get no label. */
function fileSummary(changes: string[]): [string, string][] {
  const byPath = new Map<string, number | null>();
  for (const id of changes) {
    const m = id.match(/^(.*)#\d+$/);
    if (!m) byPath.set(id, null);
    else byPath.set(m[1], (byPath.get(m[1]) ?? 0) + 1);
  }
  return [...byPath].map(([p, n]) => [p, n === null ? "" : tr(n === 1 ? "gitWorking.hunksOne" : "gitWorking.hunksOther", { count: n })]);
}

/** Asks the AI to split the staged changes (down to individual hunks, so one
 * file can feed several commits) into atomic commits, asking clarifying
 * questions when unsure. Shows the plan, then the backend creates the commits
 * straight from the index, guaranteeing they add up to exactly what's staged. */
export function ComposeCommitsModal({
  repoId,
  fileCount,
  author,
  noVerify = false,
  onApplied,
  onClose,
}: {
  repoId: string;
  fileCount: number;
  author: { name: string; email: string } | null;
  /** Skip pre-commit / commit-msg hooks (the staging panel's "Skip hooks"). */
  noVerify?: boolean;
  /** After the commits were created (before closing). */
  onApplied?: () => void;
  onClose: () => void;
}) {
  const t = useT();
  const queryClient = useQueryClient();
  const { settings } = useAiSettings();
  const [questions, setQuestions] = useState<string[]>([]);
  const [answers, setAnswers] = useState("");
  const [plan, setPlan] = useState<ComposedCommit[] | null>(null);
  const [treeId, setTreeId] = useState("");

  const compose = useMutation({
    mutationFn: (answersText: string) =>
      composeCommits({
        model: settings.commitMessageModel,
        repoId,
        customInstructions: settings.composeCustomInstructions,
        answers: answersText,
      }),
    onSuccess: (r) => {
      setTreeId(r.treeId);
      if (r.commits.length > 0) setPlan(r.commits);
      else setQuestions(r.questions);
    },
    onError: (e) => toast.error(errorMessage(e)),
  });

  // Kick off once on open.
  useEffect(() => {
    compose.mutate("");
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const refresh = () => {
    queryClient.invalidateQueries({ queryKey: queryKeys.workingStatus(repoId) });
    queryClient.invalidateQueries({ queryKey: ["workdirDiff", repoId] });
    queryClient.invalidateQueries({ queryKey: queryKeys.repoStatus(repoId) });
    queryClient.invalidateQueries({ queryKey: queryKeys.commitLog(repoId) });
    queryClient.invalidateQueries({ queryKey: queryKeys.headCommit(repoId) });
  };

  const apply = useMutation({
    mutationFn: (commits: ComposedCommit[]) =>
      applyComposedCommits(repoId, treeId, commits, author?.name ?? null, author?.email ?? null, noVerify),
    onSuccess: (_d, commits) => {
      toast.success(t(commits.length === 1 ? "gitWorking.commitsCreatedOne" : "gitWorking.commitsCreatedOther", { count: commits.length }));
      onApplied?.();
      onClose();
    },
    // All-or-nothing: on error no commit was created and the staging is untouched.
    onError: toastCommitError,
    onSettled: refresh,
  });

  const busy = compose.isPending || apply.isPending;

  return (
    <Modal onClose={busy ? () => {} : onClose} align="top" width="w-[640px]">
      <div className="flex max-h-[70vh] flex-col text-sm">
        <div className="shrink-0 border-b border-neutral-800 px-3 py-2 font-semibold text-neutral-200">
          {t("gitWorking.compose")}
        </div>
        <div className="min-h-0 flex-1 space-y-3 overflow-y-auto p-4">
          {compose.isPending && (
            <p className="flex items-center gap-2 text-neutral-400">
              <Loader2 size={14} className="animate-spin" /> {t(fileCount === 1 ? "gitWorking.analyzingOne" : "gitWorking.analyzingOther", { count: fileCount })}
            </p>
          )}
          {!compose.isPending && !plan && questions.length > 0 && (
            <>
              <p className="text-xs text-neutral-500">{t("gitWorking.aiQuestions")}</p>
              <ul className="list-disc space-y-1 pl-5 text-neutral-200">
                {questions.map((q) => (
                  <li key={q}>{q}</li>
                ))}
              </ul>
              <textarea
                autoFocus
                value={answers}
                onChange={(e) => setAnswers(e.currentTarget.value)}
                rows={4}
                placeholder={t("gitWorking.answersPlaceholder")}
                className="w-full rounded bg-neutral-800 px-2 py-1 text-sm text-neutral-100"
              />
            </>
          )}
          {!compose.isPending && !plan && questions.length === 0 && compose.isError && (
            <p className="text-neutral-400">{t("gitWorking.composeFailed")}</p>
          )}
          {plan?.map((c, i) => (
            <div key={i} className="rounded border border-neutral-800 p-2">
              <div className="font-mono text-neutral-100">
                {i + 1}. {c.summary}
              </div>
              {c.description && (
                <pre className="mt-1 whitespace-pre-wrap font-sans text-xs text-neutral-400">{c.description}</pre>
              )}
              <ul className="mt-1.5 space-y-0.5 text-xs text-neutral-500">
                {fileSummary(c.changes).map(([path, label]) => (
                  <li key={path} className="truncate font-mono">
                    {path} <span className="text-neutral-600">{label}</span>
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </div>
        <div className="flex shrink-0 justify-end gap-2 border-t border-neutral-800 p-3">
          <button
            onClick={onClose}
            disabled={busy}
            className="rounded border border-neutral-700 px-3 py-1.5 text-neutral-300 hover:bg-neutral-900 disabled:opacity-50"
          >
            {t("gitCommon.cancel")}
          </button>
          {plan ? (
            <>
              <button
                onClick={() => {
                  setPlan(null);
                  compose.mutate(answers);
                }}
                disabled={busy}
                className="rounded border border-neutral-700 px-3 py-1.5 text-neutral-300 hover:bg-neutral-900 disabled:opacity-50"
              >
                {t("gitWorking.regenerate")}
              </button>
              <button
                onClick={() => apply.mutate(plan)}
                disabled={busy}
                className="flex items-center gap-1.5 rounded bg-emerald-700 px-3 py-1.5 text-emerald-50 hover:bg-emerald-800 disabled:opacity-50"
              >
                {apply.isPending && <Loader2 size={13} className="animate-spin" />}
                {t(plan.length === 1 ? "gitWorking.createCommitsOne" : "gitWorking.createCommitsOther", { count: plan.length })}
              </button>
            </>
          ) : (
            questions.length > 0 && (
              <button
                onClick={() => {
                  setQuestions([]);
                  compose.mutate(answers);
                }}
                disabled={busy || !answers.trim()}
                className="rounded bg-emerald-700 px-3 py-1.5 text-emerald-50 hover:bg-emerald-800 disabled:opacity-50"
              >
                {t("gitWorking.sendAnswers")}
              </button>
            )
          )}
        </div>
      </div>
    </Modal>
  );
}
