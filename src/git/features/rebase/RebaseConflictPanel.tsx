import { AlertTriangle, FileWarning } from "lucide-react";
import { useRef, type ReactNode } from "react";
import { useRebaseStore } from "./store";
import { useResolveConflict } from "./useResolveConflict";
import { useT } from "../../../lib/i18n/react";

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div>
      <div className="bg-neutral-950 px-3 py-1.5 text-xs uppercase text-neutral-500">{title}</div>
      <ul>{children}</ul>
    </div>
  );
}

export function RebaseConflictPanel({
  repoId,
  onOpenConflict,
  openConflictPath,
}: {
  repoId: string;
  onOpenConflict: (path: string) => void;
  openConflictPath: string | null;
}) {
  const t = useT();
  const status = useRebaseStore((s) => s.status);
  const { resolve, continueRebase, abort } = useResolveConflict(repoId);
  // Paths ever seen conflicted this rebase session, minus those still
  // conflicted, gives us "resolved" without the backend tracking history —
  // this naturally resets since the panel only mounts while conflicts exist.
  const seenPaths = useRef<Set<string>>(new Set());

  if (!status) return null;

  status.conflicts.forEach((c) => seenPaths.current.add(c.path));
  const conflictPaths = new Set(status.conflicts.map((c) => c.path));
  const resolvedPaths = [...seenPaths.current].filter((p) => !conflictPaths.has(p));
  const allResolved = status.conflicts.length === 0;

  return (
    <div className="flex h-full flex-col overflow-hidden text-sm">
      <div className="shrink-0 border-b border-amber-900 bg-amber-950/40 px-3 py-2 text-amber-100">
        <div className="flex items-center gap-1.5">
          <AlertTriangle size={13} className="shrink-0" />
          <span>
            {status.totalSteps > 0
              ? t("gitRebase.rebaseInProgress", { current: status.currentStep, total: status.totalSteps })
              : t("gitRebase.mergeInProgress")}
          </span>
        </div>
      </div>

      <div className="flex-1 overflow-auto">
        <Section title={t("gitRebase.conflictedFiles", { count: status.conflicts.length })}>
          {status.conflicts.map((c) => (
            <li
              key={c.path}
              onClick={() => !c.isBinary && onOpenConflict(c.path)}
              className={`flex items-center justify-between gap-2 px-3 py-1.5 ${c.isBinary ? "" : "cursor-pointer"} ${
                openConflictPath === c.path ? "bg-neutral-800" : "hover:bg-neutral-900"
              }`}
            >
              <span className="flex min-w-0 items-center gap-1.5 truncate">
                <FileWarning size={12} className="shrink-0 text-amber-500" />
                <span className="truncate font-mono text-xs">{c.path}</span>
              </span>
              {c.isBinary && (
                <div className="flex shrink-0 gap-1">
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      resolve(c.path, { kind: "UseOurs" });
                    }}
                    className="rounded bg-neutral-800 px-1.5 py-0.5 text-[10px] hover:bg-neutral-700"
                  >
                    {t("gitRebase.keepOurs")}
                  </button>
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      resolve(c.path, { kind: "UseTheirs" });
                    }}
                    className="rounded bg-neutral-800 px-1.5 py-0.5 text-[10px] hover:bg-neutral-700"
                  >
                    {t("gitRebase.keepTheirs")}
                  </button>
                </div>
              )}
            </li>
          ))}
        </Section>

        {resolvedPaths.length > 0 && (
          <Section title={t("gitRebase.resolvedFiles", { count: resolvedPaths.length })}>
            {resolvedPaths.map((p) => (
              <li key={p} className="truncate px-3 py-1.5 font-mono text-xs text-neutral-500">
                {p}
              </li>
            ))}
          </Section>
        )}
      </div>

      <div className="flex shrink-0 gap-2 border-t border-neutral-800 p-2">
        <button
          onClick={continueRebase}
          disabled={!allResolved}
          className="flex-1 rounded bg-amber-800 px-2 py-1.5 text-sm hover:bg-amber-700 disabled:opacity-40"
        >
          {t("gitRebase.continue")}
        </button>
        <button
          onClick={abort}
          className="rounded bg-red-900 px-3 py-1.5 text-sm text-red-100 hover:bg-red-800"
        >
          {t("gitRebase.abort")}
        </button>
      </div>
    </div>
  );
}
