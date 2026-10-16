import { useInfiniteQuery } from "@tanstack/react-query";
import { History, Loader2, Undo2 } from "lucide-react";
import { memo, useEffect, useMemo, useRef } from "react";
import type { CommitNode } from "@/features/commit-graph/types";
import { formatDate } from "@/lib/format-date";
import { gravatarUrl } from "@/lib/gravatar";
import { queryKeys } from "@/lib/query-keys";
import { guessLanguage, highlightLine } from "@/lib/syntax";
import { errorMessage } from "@/lib/tauri-client";
import { getBlame } from "./api";
import { blameBlocks, nextBlameLine } from "./model";
import { openBlame, openFileHistory } from "./store";
import type { BlameChunk } from "./types";
import { useT } from "../../../lib/i18n/react";

/** Lines blamed per request: the top of a big file shows right away and
 * the rest streams in as it's scrolled to. */
const CHUNK_LINES = 1000;
const LINE_HEIGHT = 18;

/** One loaded chunk, memoized so loading the next one doesn't re-highlight
 * the ones already on screen. */
const BlameChunkView = memo(function BlameChunkView({
  repoId,
  path,
  chunk,
  prevLastSha,
  language,
  onSelectCommit,
}: {
  repoId: string;
  path: string;
  chunk: BlameChunk;
  prevLastSha: string | null;
  language: string | null;
  onSelectCommit: (commit: CommitNode) => void;
}) {
  const t = useT();
  const commits = useMemo(() => new Map(chunk.commits.map((c) => [c.sha, c])), [chunk]);
  const blocks = useMemo(() => blameBlocks(chunk, prevLastSha), [chunk, prevLastSha]);

  return (
    <>
      {blocks.map((block) => {
        const commit = commits.get(block.sha);
        return (
          <div
            key={block.startLine}
            className={`flex ${block.continued ? "" : "border-t border-neutral-800"}`}
            // Off-screen blocks skip layout/paint entirely.
            style={{ contentVisibility: "auto", containIntrinsicSize: `auto ${block.lines.length * LINE_HEIGHT}px` }}
          >
            <div className="w-72 shrink-0 border-r border-neutral-800 bg-neutral-900/40 px-2">
              {commit && !block.continued && (
                <div className="sticky top-0 flex items-center gap-2 py-0.5 text-xs" style={{ minHeight: LINE_HEIGHT }}>
                  <img src={gravatarUrl(commit.authorEmail, 32)} alt="" className="h-4 w-4 shrink-0 rounded-full" />
                  <span className="min-w-0 flex-1 truncate text-neutral-300" title={`${commit.summary}\n\n${commit.authorName} <${commit.authorEmail}>`}>
                    {commit.summary}
                  </span>
                  <span className="shrink-0 tabular-nums text-neutral-500">{formatDate(commit.authorDate)}</span>
                  <button
                    onClick={() => onSelectCommit(commit)}
                    title={t("gitHistory.selectInGraph", { sha: commit.shortSha, author: commit.authorName })}
                    className="shrink-0 font-mono text-blue-500 hover:underline"
                  >
                    {commit.shortSha}
                  </button>
                  {commit.parents[0] && (
                    <button
                      onClick={() => openBlame(repoId, block.origPath ?? path, commit.parents[0])}
                      title={t("gitHistory.blamePrevious")}
                      className="shrink-0 text-neutral-500 hover:text-neutral-200"
                    >
                      <Undo2 size={12} />
                    </button>
                  )}
                </div>
              )}
            </div>
            <div className="min-w-0 flex-1">
              {block.lines.map((line, i) => (
                <div key={i} className="flex font-mono text-xs" style={{ height: LINE_HEIGHT, lineHeight: `${LINE_HEIGHT}px` }}>
                  <span className="w-12 shrink-0 select-none pr-2 text-right text-neutral-600">{block.startLine + i}</span>
                  <span className="whitespace-pre text-neutral-200">{highlightLine(line, language)}</span>
                </div>
              ))}
            </div>
          </div>
        );
      })}
    </>
  );
});

/** Who last changed each line of `path` at `sha`, grouped by hunk. Chunks
 * are cached per (path, commit): reopening a blame is instant. */
export function BlameView({
  repoId,
  path,
  sha,
  onSelectCommit,
}: {
  repoId: string;
  path: string;
  /** Resolved commit (HEAD already resolved). */
  sha: string;
  onSelectCommit: (commit: CommitNode) => void;
}) {
  const t = useT();
  const { data, fetchNextPage, hasNextPage, isFetchingNextPage, isPending, error } = useInfiniteQuery({
    queryKey: queryKeys.blame(repoId, path, sha),
    queryFn: ({ pageParam }) => getBlame(repoId, path, sha, pageParam, CHUNK_LINES),
    initialPageParam: 1,
    getNextPageParam: nextBlameLine,
    staleTime: Infinity,
  });
  const language = useMemo(() => guessLanguage(path), [path]);
  const scrollRef = useRef<HTMLDivElement>(null);
  const sentinelRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const root = scrollRef.current;
    const target = sentinelRef.current;
    if (!root || !target || !hasNextPage) return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0].isIntersecting && !isFetchingNextPage) fetchNextPage();
      },
      { root, rootMargin: "600px" },
    );
    observer.observe(target);
    return () => observer.disconnect();
  }, [hasNextPage, isFetchingNextPage, fetchNextPage]);

  const pages = data?.pages ?? [];

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <div className="flex shrink-0 items-center justify-between border-b border-neutral-800 px-3 py-1 text-xs text-neutral-500">
        <span>
          {pages[0] ? t(pages[0].totalLines === 1 ? "gitHistory.linesOne" : "gitHistory.linesOther", { count: pages[0].totalLines }) : ""}
          {hasNextPage ? ` · ${t("gitHistory.linesLoaded", { count: pages.reduce((n, p) => n + p.lines.length, 0) })}` : ""}
        </span>
        <button
          onClick={() => openFileHistory(repoId, path, sha)}
          className="flex items-center gap-1 rounded bg-neutral-800 px-2 py-0.5 text-neutral-300 hover:bg-neutral-700"
        >
          <History size={12} /> {t("gitHistory.fileHistory")}
        </button>
      </div>
      <div ref={scrollRef} className="min-h-0 flex-1 overflow-auto">
        {isPending && (
          <div className="flex items-center gap-2 p-4 text-sm text-neutral-500">
            <Loader2 size={14} className="animate-spin" /> {t("gitHistory.runningBlame")}
          </div>
        )}
        {error && <p className="p-4 text-sm text-red-400">{errorMessage(error)}</p>}
        {pages[0]?.totalLines === 0 && <p className="p-4 text-sm text-neutral-500">{t("gitHistory.emptyFile")}</p>}
        {pages.map((chunk, i) => {
          const prev = pages[i - 1];
          return (
            <BlameChunkView
              key={chunk.startLine}
              repoId={repoId}
              path={path}
              chunk={chunk}
              prevLastSha={prev?.hunks[prev.hunks.length - 1]?.sha ?? null}
              language={language}
              onSelectCommit={onSelectCommit}
            />
          );
        })}
        {hasNextPage && <div ref={sentinelRef} className="h-px" />}
        {isFetchingNextPage && <p className="py-2 text-center text-xs text-neutral-500">{t("gitHistory.loadingMoreLines")}</p>}
      </div>
    </div>
  );
}
