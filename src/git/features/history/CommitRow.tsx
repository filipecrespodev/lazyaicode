import type { ReactNode } from "react";
import type { CommitNode } from "@/features/commit-graph/types";
import { formatDateTime } from "@/lib/format-date";
import { gravatarUrl } from "@/lib/gravatar";
import { useT } from "../../../lib/i18n/react";

/** Compact commit line for the history lists (file history, compare). */
export function CommitRow({
  commit,
  selected,
  badge,
  onClick,
  onDoubleClick,
}: {
  commit: CommitNode;
  selected: boolean;
  badge?: ReactNode;
  onClick: () => void;
  onDoubleClick?: () => void;
}) {
  const t = useT();
  return (
    <li
      onClick={onClick}
      onDoubleClick={onDoubleClick}
      title={onDoubleClick ? t("gitHistory.doubleClickSelect") : undefined}
      className={`flex cursor-pointer items-start gap-2 border-b border-neutral-800/60 px-3 py-1.5 ${
        selected ? "bg-neutral-800" : "hover:bg-neutral-900"
      }`}
    >
      <img src={gravatarUrl(commit.authorEmail, 32)} alt="" className="mt-0.5 h-5 w-5 shrink-0 rounded-full" />
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-1.5">
          <span className="min-w-0 flex-1 truncate text-sm text-neutral-200">{commit.summary}</span>
          {badge}
        </div>
        <div className="flex items-center gap-2 text-xs text-neutral-500">
          <span className="min-w-0 truncate">{commit.authorName}</span>
          <span className="shrink-0 tabular-nums">{formatDateTime(commit.authorDate)}</span>
          <span className="ml-auto shrink-0 font-mono">{commit.shortSha}</span>
        </div>
      </div>
    </li>
  );
}
