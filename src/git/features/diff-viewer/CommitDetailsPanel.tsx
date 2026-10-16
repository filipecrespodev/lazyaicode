import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ShieldCheck } from "lucide-react";
import { useEffect, useState } from "react";
import type { CommitNode } from "@/features/commit-graph/types";
import { fileHistoryMenuItems } from "@/features/history/store";
import { amendCommitMessage } from "@/features/working/api";
import { toastCommitError } from "@/features/working/commit-error";
import { formatDateTime } from "@/lib/format-date";
import { gravatarUrl } from "@/lib/gravatar";
import { queryKeys } from "@/lib/query-keys";
import { getCommitDiff } from "./api";
import { ChangedFileList } from "./ChangedFileList";
import { getCommitSignature, signatureLabel } from "./signature";
import type { FileDiff } from "./types";
import { useT } from "../../../lib/i18n/react";
import {  } from "../../../lib/i18n/react";

export function CommitDetailsPanel({
  repoId,
  commit,
  selectedFilePath,
  onOpenFile,
  onAmended,
}: {
  repoId: string;
  commit: CommitNode | null;
  selectedFilePath: string | null;
  onOpenFile: (file: FileDiff) => void;
  onAmended: (commit: CommitNode) => void;
}) {
  const t = useT();
  const queryClient = useQueryClient();
  const { data, isPending } = useQuery({
    queryKey: queryKeys.commitDiff(repoId, commit?.sha ?? ""),
    queryFn: () => getCommitDiff(repoId, commit!.sha),
    enabled: !!commit,
  });
  // Only for the selected commit, and a commit's signature never changes.
  const { data: signature } = useQuery({
    queryKey: queryKeys.commitSignature(repoId, commit?.sha ?? ""),
    queryFn: () => getCommitSignature(repoId, commit!.sha),
    enabled: !!commit,
    staleTime: Infinity,
  });
  const signed = signatureLabel(signature);

  const [editingSummary, setEditingSummary] = useState(false);
  const [editingBody, setEditingBody] = useState(false);
  const [summaryDraft, setSummaryDraft] = useState("");
  const [bodyDraft, setBodyDraft] = useState("");

  useEffect(() => {
    setEditingSummary(false);
    setEditingBody(false);
  }, [commit?.sha]);

  const amendMut = useMutation({
    mutationFn: (args: { summary: string; body: string }) =>
      amendCommitMessage(repoId, commit!.sha, args.summary, args.body),
    onSuccess: (updated) => {
      queryClient.invalidateQueries({ queryKey: queryKeys.commitLog(repoId) });
      queryClient.invalidateQueries({ queryKey: queryKeys.headCommit(repoId) });
      onAmended(updated);
    },
    onError: toastCommitError,
  });

  const saveSummary = () => {
    setEditingSummary(false);
    const next = summaryDraft.trim();
    if (!commit || !next || next === commit.summary) return;
    amendMut.mutate({ summary: next, body: commit.body });
  };

  const saveBody = () => {
    setEditingBody(false);
    if (!commit || bodyDraft === commit.body) return;
    amendMut.mutate({ summary: commit.summary, body: bodyDraft });
  };

  if (!commit) {
    return <p className="p-4 text-sm text-neutral-500">{t("gitDiff.selectCommit")}</p>;
  }

  return (
    <div className="flex h-full flex-col overflow-hidden text-sm">
      <div className="shrink-0 border-b border-neutral-800 p-3">
        <p className="mb-2 flex items-center gap-2 font-mono text-xs text-neutral-500">
          commit: {commit.shortSha}
          {signed && (
            <span
              title={t("gitDiff.signedTitle")}
              className="flex items-center gap-1 rounded bg-emerald-900/40 px-1.5 py-0.5 font-sans text-[10px] text-emerald-300"
            >
              <ShieldCheck size={10} /> {signed}
            </span>
          )}
        </p>
        {editingSummary ? (
          <input
            autoFocus
            value={summaryDraft}
            onChange={(e) => setSummaryDraft(e.currentTarget.value)}
            onFocus={(e) => e.currentTarget.select()}
            onBlur={saveSummary}
            onKeyDown={(e) => {
              if (e.key === "Enter") e.currentTarget.blur();
              if (e.key === "Escape") setEditingSummary(false);
            }}
            className="mb-1 w-full rounded bg-neutral-900 px-1.5 py-0.5 font-medium text-neutral-100"
          />
        ) : (
          <p
            onClick={() => {
              setSummaryDraft(commit.summary);
              setEditingSummary(true);
            }}
            className="mb-1 cursor-pointer whitespace-pre-wrap rounded font-medium text-neutral-100 hover:bg-neutral-900"
          >
            {commit.summary}
          </p>
        )}
        {editingBody ? (
          <textarea
            autoFocus
            rows={3}
            value={bodyDraft}
            onChange={(e) => setBodyDraft(e.currentTarget.value)}
            onBlur={saveBody}
            onKeyDown={(e) => {
              if (e.key === "Escape") setEditingBody(false);
            }}
            className="mb-3 w-full resize-none rounded bg-neutral-900 px-1.5 py-0.5 text-xs text-neutral-400"
          />
        ) : commit.body ? (
          <p
            onClick={() => {
              setBodyDraft(commit.body);
              setEditingBody(true);
            }}
            className="mb-3 cursor-pointer whitespace-pre-wrap rounded text-xs text-neutral-400 hover:bg-neutral-900"
          >
            {commit.body}
          </p>
        ) : (
          <p
            onClick={() => {
              setBodyDraft("");
              setEditingBody(true);
            }}
            className="mb-3 cursor-pointer rounded text-xs italic text-neutral-600 hover:bg-neutral-900"
          >
            {t("gitDiff.addDescription")}
          </p>
        )}

        <div className="flex items-center gap-2">
          <img
            src={gravatarUrl(commit.authorEmail, 32)}
            alt={commit.authorName}
            className="h-8 w-8 shrink-0 rounded-full"
          />
          <div className="min-w-0">
            <p className="truncate text-neutral-200">{commit.authorName}</p>
            <p className="truncate text-xs text-neutral-500">{commit.authorEmail}</p>
            <p className="text-xs text-neutral-500">
              {t("gitDiff.createdAt", { date: formatDateTime(commit.authorDate) })}
            </p>
          </div>
        </div>
        {commit.committerDate !== commit.authorDate && (
          <p className="mt-1 pl-10 text-xs text-neutral-500">
            {t("gitDiff.committedAt", { date: formatDateTime(commit.committerDate) })}
          </p>
        )}
      </div>

      <ChangedFileList
        title={(data?.files.length ?? 0) === 1 ? t("gitDiff.filesChangedOne") : t("gitDiff.filesChangedOther", { count: data?.files.length ?? 0 })}
        files={data?.files ?? []}
        isPending={isPending}
        selectedFilePath={selectedFilePath}
        onOpenFile={onOpenFile}
        fileMenu={(file) => fileHistoryMenuItems(repoId, file.path, file.status === "Deleted" ? (commit.parents[0] ?? null) : commit.sha)}
      />
    </div>
  );
}
