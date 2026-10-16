import { useQuery } from "@tanstack/react-query";
import type { CommitNode } from "@/features/commit-graph/types";
import { fileHistoryMenuItems } from "@/features/history/store";
import { queryKeys } from "@/lib/query-keys";
import { getRangeDiff } from "./api";
import { ChangedFileList } from "./ChangedFileList";
import type { FileDiff } from "./types";
import { useT } from "../../../lib/i18n/react";

export function MultiCommitDetailsPanel({
  repoId,
  commits,
  fromSha,
  toSha,
  selectedFilePath,
  onOpenFile,
}: {
  repoId: string;
  commits: CommitNode[];
  fromSha: string;
  toSha: string;
  selectedFilePath: string | null;
  onOpenFile: (file: FileDiff) => void;
}) {
  const t = useT();
  const { data, isPending } = useQuery({
    queryKey: queryKeys.rangeDiff(repoId, fromSha, toSha),
    queryFn: () => getRangeDiff(repoId, fromSha, toSha),
  });

  return (
    <div className="flex h-full flex-col overflow-hidden text-sm">
      <div className="max-h-48 shrink-0 overflow-auto border-b border-neutral-800 p-3">
        <p className="mb-2 text-xs uppercase text-neutral-500">{t("gitDiff.commitsSelected", { count: commits.length })}</p>
        <ul className="space-y-1">
          {commits.map((commit) => (
            <li key={commit.sha} className="truncate text-xs text-neutral-300">
              <span className="mr-2 font-mono text-neutral-500">{commit.shortSha}</span>
              {commit.summary}
            </li>
          ))}
        </ul>
      </div>

      <ChangedFileList
        title={(data?.files.length ?? 0) === 1 ? t("gitDiff.rangeFilesOne") : t("gitDiff.rangeFilesOther", { count: data?.files.length ?? 0 })}
        files={data?.files ?? []}
        isPending={isPending}
        selectedFilePath={selectedFilePath}
        onOpenFile={onOpenFile}
        fileMenu={(file) => fileHistoryMenuItems(repoId, file.path, toSha)}
      />
    </div>
  );
}
