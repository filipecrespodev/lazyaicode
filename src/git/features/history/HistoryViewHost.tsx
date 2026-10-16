import { portalRoot } from "@/embed";
import { useQuery } from "@tanstack/react-query";
import { ArrowLeft, X } from "lucide-react";
import { useCallback, useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import { getHeadCommit } from "@/features/commit-graph/api";
import type { CommitNode } from "@/features/commit-graph/types";
import { queryKeys } from "@/lib/query-keys";
import { BlameView } from "./BlameView";
import { CompareView } from "./CompareView";
import { FileHistoryView } from "./FileHistoryView";
import { useHistoryViewStore, type HistoryView } from "./store";
import { t, useT } from "../../../lib/i18n/react";

function title(view: HistoryView): { main: string; sub: string } {
  switch (view.kind) {
    case "fileHistory":
      return { main: t("gitHistory.fileHistory"), sub: view.path };
    case "blame":
      return { main: t("gitHistory.blame"), sub: view.path };
    case "compare":
      return { main: t("gitHistory.compare"), sub: `${view.a} ↔ ${view.b}` };
  }
}

/** Shows the history view on top of the stack (file history, blame,
 * compare) for this repo's tab, full screen like the reflog. Esc / Back
 * returns to the previous view. Picking a commit in any of them closes the
 * stack and selects that commit in the graph. */
export function HistoryViewHost({
  repoId,
  active,
  onSelectCommit,
}: {
  repoId: string;
  active: boolean;
  onSelectCommit: (commit: CommitNode) => void;
}) {
  const t = useT();
  const view = useHistoryViewStore((s) => s.stack[s.stack.length - 1] ?? null);
  const depth = useHistoryViewStore((s) => s.stack.length);
  const back = useHistoryViewStore((s) => s.back);
  const close = useHistoryViewStore((s) => s.close);
  const shown = active && view !== null && view.repoId === repoId ? view : null;

  // A stable callback, so memoized blame chunks don't re-render with every parent render.
  const selectRef = useRef(onSelectCommit);
  useEffect(() => {
    selectRef.current = onSelectCommit;
  });
  const selectCommit = useCallback((commit: CommitNode) => {
    useHistoryViewStore.getState().close();
    selectRef.current(commit);
  }, []);

  const needsHead = shown !== null && shown.kind !== "compare" && shown.sha === null;
  const { data: head, isPending: headPending } = useQuery({
    queryKey: queryKeys.headCommit(repoId),
    queryFn: () => getHeadCommit(repoId),
    enabled: needsHead,
  });

  useEffect(() => {
    if (!shown) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !e.defaultPrevented) back();
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [shown, back]);

  if (!shown) return null;
  const { main, sub } = title(shown);
  const sha = shown.kind === "compare" ? null : (shown.sha ?? head?.sha ?? null);

  let body: React.ReactNode;
  if (shown.kind === "compare") {
    body = <CompareView key={`${shown.a}\0${shown.b}`} repoId={repoId} a={shown.a} b={shown.b} onSelectCommit={selectCommit} />;
  } else if (!sha) {
    body = <p className="p-4 text-sm text-neutral-500">{needsHead && headPending ? t("gitHistory.resolvingHead") : t("gitHistory.noCommits")}</p>;
  } else if (shown.kind === "fileHistory") {
    body = <FileHistoryView key={`${shown.path}\0${sha}`} repoId={repoId} path={shown.path} startSha={sha} onSelectCommit={selectCommit} />;
  } else {
    body = <BlameView key={`${shown.path}\0${sha}`} repoId={repoId} path={shown.path} sha={sha} onSelectCommit={selectCommit} />;
  }

  return createPortal(
    <div className="fixed inset-0 z-[150] flex flex-col bg-neutral-950 text-neutral-200">
      <div className="flex shrink-0 items-center justify-between gap-3 border-b border-neutral-800 px-3 py-2">
        <div className="flex min-w-0 items-center gap-3">
          {depth > 1 && (
            <button
              title={t("gitHistory.back")}
              onClick={back}
              className="rounded p-1 text-neutral-400 hover:bg-neutral-800 hover:text-neutral-100"
            >
              <ArrowLeft size={15} />
            </button>
          )}
          <span className="shrink-0 text-sm font-semibold">{main}</span>
          <span className="truncate font-mono text-xs text-neutral-400" title={sub}>
            {sub}
          </span>
          {sha && <span className="shrink-0 font-mono text-xs text-neutral-500">@ {sha.slice(0, 7)}</span>}
        </div>
        <button
          title={t("gitHistory.close")}
          onClick={close}
          className="rounded p-1 text-neutral-400 hover:bg-neutral-800 hover:text-neutral-100"
        >
          <X size={15} />
        </button>
      </div>
      {body}
    </div>,
    portalRoot(),
  );
}
