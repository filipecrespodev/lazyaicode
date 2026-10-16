import { useQuery } from "@tanstack/react-query";
import { ArrowLeft } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { Decoration, Diff, getChangeKey, Hunk, markEdits, parseDiff, tokenize } from "react-diff-view";
import type { RenderToken } from "react-diff-view";
import "react-diff-view/style/index.css";
import type { WorkingSide } from "@/features/working/types";
import { HunkActions, LineSelectionBar, useLineStaging } from "@/features/working/useLineStaging";
import { guessLanguage, highlightLine } from "@/lib/syntax";
import { usePersistedValue } from "@/lib/use-persisted-config";
import { getFileContent } from "./api";
import { collapseContext, type Piece } from "./collapseContext";
import { DiffMinimap } from "./DiffMinimap";
import { getImageMime } from "./imageMime";
import type { FileDiff } from "./types";
import { useT } from "../../../lib/i18n/react";
import {  } from "../../../lib/i18n/react";

type Hunk_ = ReturnType<typeof parseDiff>[number]["hunks"][number];
const CONTEXT_LINES = 3;

export function FileDiffFullView({
  repoId,
  sha,
  file,
  workingSide = null,
  onBack,
  actions,
}: {
  repoId: string;
  sha: string | null;
  file: FileDiff;
  /** Set for a working-copy file: enables hunk/line staging for that side. */
  workingSide?: WorkingSide | null;
  onBack: () => void;
  /** Extra header buttons, left of Back. */
  actions?: ReactNode;
}) {
  const t = useT();
  const [viewType, setViewType] = usePersistedValue<"unified" | "split">("diff.viewType", "unified");
  // "Changes only" folds long unchanged runs into expandable bars.
  const [changesOnly, setChangesOnly] = usePersistedValue("diff.changesOnly", false);
  const [expandedGaps, setExpandedGaps] = useState<Set<string>>(new Set());
  const imageMime = file.isBinary ? getImageMime(file.path) : null;
  const scrollRef = useRef<HTMLDivElement>(null);

  // Memoized on the patch text: parsing a big file on every render would also
  // hand the diff new hunk objects each time and re-render every row.
  const parsed = useMemo(() => {
    if (file.isBinary) return null;
    try {
      return parseDiff(file.patch)[0] ?? null;
    } catch {
      return null;
    }
  }, [file.isBinary, file.patch]);

  const language = file.isBinary ? null : guessLanguage(file.path);
  const tokens = useMemo(() => {
    if (!parsed) return undefined;
    try {
      // react-diff-view's own `highlight: true` option is incompatible with
      // installed refractor versions >= 4 (it still expects refractor@3's
      // token shape and throws on the current hast-based one) — syntax
      // coloring is applied ourselves below, via `renderToken`, instead.
      // markEdits still works fine here: it operates on raw text, not tokens.
      return tokenize(parsed.hunks, {
        highlight: false,
        enhancers: [markEdits(parsed.hunks, { type: "block" })],
      });
    } catch {
      return undefined;
    }
  }, [parsed]);

  // Re-applies our own syntax highlighter on top of react-diff-view's plain
  // (unhighlighted) tokens, per token — "text" tokens are a line's untouched
  // portion, "edit" tokens are the specific range markEdits found different
  // from the other side. Highlighting each independently can occasionally
  // split a token (e.g. a string literal) at an edit boundary — an accepted
  // tradeoff for combining word-level diffing with syntax color.
  const renderToken = useCallback<RenderToken>((token, renderDefault, index) => {
    if (!language) return renderDefault(token, index);
    if (token.type === "text" && typeof token.value === "string") {
      return <span key={index}>{highlightLine(token.value, language)}</span>;
    }
    if (token.type === "edit") {
      const text = ((token.children ?? []) as Array<{ value?: string }>)
        .map((c) => c.value ?? "")
        .join("");
      return (
        <span key={index} className="diff-code-edit">
          {highlightLine(text, language)}
        </span>
      );
    }
    return renderDefault(token, index);
  }, [language]);

  // Renders one hunk, folded into pieces when "changes only" is on.
  const renderHunk = (hunk: Hunk_) => {
    if (!changesOnly) return [<Hunk key={hunk.content} hunk={hunk} />];
    return collapseContext(hunk, CONTEXT_LINES).map((piece: Piece<Hunk_["changes"][number], Hunk_>) =>
      piece.kind === "gap" && !expandedGaps.has(piece.key) ? (
        <Decoration key={piece.key}>
          <button
            onClick={() => setExpandedGaps((prev) => new Set(prev).add(piece.key))}
            className="w-full bg-neutral-900 py-0.5 text-center text-[11px] text-neutral-500 hover:bg-neutral-800 hover:text-neutral-300"
          >
            ⋯ {t(piece.count === 1 ? "gitDiff.unchangedLinesOne" : "gitDiff.unchangedLinesOther", { count: piece.count })}
          </button>
        </Decoration>
      ) : (
        <Hunk key={piece.hunk.content} hunk={piece.hunk} />
      ),
    );
  };

  const staging = useLineStaging(
    repoId,
    file.path,
    file.patch,
    workingSide && parsed ? workingSide : null,
    parsed?.hunks ?? null,
  );

  const { data: imageBase64, isPending: imagePending } = useQuery({
    queryKey: ["fileContent", repoId, sha, file.path],
    queryFn: () => getFileContent(repoId, sha, file.path),
    enabled: !!imageMime,
  });

  // Jumps to the first changed line whenever a different file is opened, or
  // the same file's content changes under it (e.g. after a stash pop/apply) —
  // deferred a tick so the hunks (and their anchor ids) have painted first.
  // Not when the change is the user staging hunks/lines of the open
  // working-copy file: they keep their place while working down the file.
  useEffect(() => setExpandedGaps(new Set()), [file.path, file.patch]);
  const jumpedFor = useRef<string | null>(null);
  useEffect(() => {
    if (!parsed) return;
    const target = `${workingSide}:${file.path}`;
    const sameWorkingFile = workingSide !== null && jumpedFor.current === target;
    jumpedFor.current = target;
    if (sameWorkingFile) return;
    const firstChange = parsed.hunks.flatMap((h) => h.changes).find((c) => c.type !== "normal");
    if (!firstChange) return;
    const id = getChangeKey(firstChange);
    const timer = setTimeout(() => {
      document.getElementById(id)?.scrollIntoView({ block: "center" });
    }, 0);
    return () => clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [file.path, file.patch]);

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <div className="flex shrink-0 items-center justify-between border-b border-neutral-800 bg-neutral-900 px-3 py-2 text-sm text-neutral-200">
        <span className="min-w-0 truncate">
          {file.oldPath && file.oldPath !== file.path ? `${file.oldPath} → ${file.path}` : file.path}
        </span>
        <div className="flex shrink-0 items-center gap-1">
          {actions}
          {!imageMime && !file.isBinary && (
            <button
              onClick={() => {
                setChangesOnly((v) => !v);
                setExpandedGaps(new Set());
              }}
              title={t("gitDiff.changesOnlyTitle")}
              className={`flex shrink-0 items-center gap-1 rounded px-2 py-1 text-xs ${
                changesOnly ? "bg-neutral-700 text-neutral-100" : "bg-neutral-800 hover:bg-neutral-700"
              }`}
            >
              {changesOnly ? t("gitDiff.wholeFile") : t("gitDiff.changesOnly")}
            </button>
          )}
          <button
            onClick={onBack}
            className="flex shrink-0 items-center gap-1 rounded bg-neutral-800 px-2 py-1 text-xs hover:bg-neutral-700"
          >
            <ArrowLeft size={13} /> {t("gitDiff.back")}
          </button>
        </div>
      </div>

      {workingSide && parsed && (
        <LineSelectionBar
          side={workingSide}
          selected={staging.selectedChanges}
          pending={staging.pending}
          onRun={staging.run}
          onClear={staging.clear}
        />
      )}

      <div className="flex min-h-0 flex-1 overflow-hidden">
        <div ref={scrollRef} className="min-w-0 flex-1 overflow-auto text-sm">
          {imageMime ? (
            imagePending ? (
              <p className="p-3 text-xs text-neutral-500">{t("gitDiff.loadingImage")}</p>
            ) : imageBase64 ? (
              <div className="flex justify-center p-4">
                <img
                  src={`data:${imageMime};base64,${imageBase64}`}
                  alt={file.path}
                  className="max-w-full rounded border border-neutral-800"
                />
              </div>
            ) : (
              <p className="p-3 text-xs text-neutral-500">{t("gitDiff.imageLoadFailed", { path: file.path })}</p>
            )
          ) : file.isBinary ? (
            <p className="p-3 text-xs text-neutral-500">{t("gitDiff.binary", { path: file.path })}</p>
          ) : parsed ? (
            <Diff
              viewType={viewType}
              diffType={parsed.type}
              hunks={staging.displayHunks ?? parsed.hunks}
              tokens={tokens}
              renderToken={renderToken}
              generateAnchorID={getChangeKey}
              className={staging.blocks ? "diff-line-select" : undefined}
              selectedChanges={staging.selectedChanges}
              gutterEvents={staging.lineEvents}
              codeEvents={staging.lineEvents}
            >
              {(hunks) =>
                staging.blocks && workingSide
                  ? staging.blocks.flatMap(({ hunk, changeKeys }) => [
                      ...(changeKeys.length > 0
                        ? [
                            <Decoration key={`actions${hunk.content}`}>
                              <HunkActions
                                side={workingSide}
                                label={hunk.content}
                                changeKeys={changeKeys}
                                disabled={staging.pending}
                                onRun={staging.run}
                              />
                            </Decoration>,
                          ]
                        : []),
                      ...renderHunk(hunk),
                    ])
                  : hunks.flatMap(renderHunk)
              }
            </Diff>
          ) : (
            <pre className="whitespace-pre-wrap break-all p-3 font-mono text-xs text-neutral-300">
              {file.patch || t("gitDiff.noChanges")}
            </pre>
          )}
        </div>
        {parsed && !imageMime && <DiffMinimap hunks={parsed.hunks} scrollRef={scrollRef} />}
      </div>

      {!imageMime && (
        <button
          onClick={() => setViewType((v) => (v === "unified" ? "split" : "unified"))}
          className="shrink-0 border-t border-neutral-800 py-2 text-sm text-neutral-400 hover:bg-neutral-900"
        >
          {viewType === "unified" ? t("gitDiff.viewSplit") : t("gitDiff.viewUnified")}
        </button>
      )}
    </div>
  );
}
