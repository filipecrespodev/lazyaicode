import { useQuery } from "@tanstack/react-query";
import { getCommitDiff } from "@/features/diff-viewer/api";
import { ChangedFileList } from "@/features/diff-viewer/ChangedFileList";
import type { FileDiff } from "@/features/diff-viewer/types";
import { queryKeys } from "@/lib/query-keys";
import type { StashInfo } from "./types";
import { useT } from "../../../lib/i18n/react";

export function StashDetailsPanel({
  repoId,
  stash,
  selectedFilePath,
  onOpenFile,
}: {
  repoId: string;
  stash: StashInfo;
  selectedFilePath: string | null;
  onOpenFile: (file: FileDiff) => void;
}) {
  const t = useT();
  const { data, isPending } = useQuery({
    queryKey: queryKeys.commitDiff(repoId, stash.sha),
    queryFn: () => getCommitDiff(repoId, stash.sha),
  });

  return (
    <div className="flex h-full flex-col overflow-hidden text-sm">
      <div className="shrink-0 border-b border-neutral-800 p-3">
        <p className="mb-2 font-mono text-xs text-neutral-500">stash: {stash.sha.slice(0, 7)}</p>
        <p className="whitespace-pre-wrap font-medium text-neutral-100">{stash.message}</p>
      </div>

      <ChangedFileList
        title={t((data?.files.length ?? 0) === 1 ? "gitStash.changedFilesOne" : "gitStash.changedFilesOther", { count: data?.files.length ?? 0 })}
        files={data?.files ?? []}
        isPending={isPending}
        selectedFilePath={selectedFilePath}
        onOpenFile={onOpenFile}
      />
    </div>
  );
}
