import { Check, ChevronRight, CornerDownRight, Eye, EyeOff, Loader2, Trash2, X } from "lucide-react";
import { memo, useLayoutEffect, useMemo, useRef, useState } from "react";
import { useEmbed } from "@/embed";
import { basename } from "@/lib/path";
import { type ActivityEntry, samePath } from "./format";
import { useActivityStore } from "./store";
import { useT } from "../../../lib/i18n/react";
import { shortcutLabel } from "../../../lib/shortcuts/shortcuts";
import { useShortcutOverrides } from "@/features/shortcuts/useShortcuts";

type Scope = "app" | "repo";

const pad = (n: number, len = 2) => String(n).padStart(len, "0");
function formatTime(at: number) {
  const d = new Date(at);
  return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}.${pad(d.getMilliseconds(), 3)}`;
}
const formatDuration = (ms: number) => (ms < 1000 ? `${Math.round(ms)} ms` : `${(ms / 1000).toFixed(1)} s`);

function StatusIcon({ entry }: { entry: ActivityEntry }) {
  if (entry.status === "running") return <Loader2 size={12} className="animate-spin text-blue-400" />;
  if (entry.status === "error") return <X size={12} className="text-red-400" />;
  if (entry.source === "backend") return <CornerDownRight size={12} className="text-neutral-500" />;
  return <Check size={12} className="text-emerald-500" />;
}

/** Memoizado: as entradas mantêm a identidade até mudarem, então uma linha
 * nova no log (várias por segundo durante o polling) renderiza uma linha, e
 * não as centenas já exibidas. */
const Row = memo(function Row({ entry }: { entry: ActivityEntry }) {
  const [expanded, setExpanded] = useState(false);
  const hasDetails = entry.args !== undefined || entry.error !== undefined;
  return (
    <li className={`border-b border-neutral-900 ${entry.source === "backend" ? "bg-neutral-950" : ""}`}>
      <button
        onClick={() => hasDetails && setExpanded((x) => !x)}
        className={`flex w-full items-start gap-2 px-2 py-1 text-left ${hasDetails ? "hover:bg-neutral-900" : "cursor-default"}`}
      >
        <span className="shrink-0 text-neutral-600">{formatTime(entry.at)}</span>
        <span className="mt-0.5 shrink-0">
          <StatusIcon entry={entry} />
        </span>
        <span
          className={`min-w-0 flex-1 whitespace-pre-wrap break-all ${
            entry.status === "error" ? "text-red-300" : entry.source === "backend" ? "text-neutral-400" : "text-neutral-200"
          }`}
        >
          {entry.text}
          {entry.error && !expanded && <span className="text-red-400"> — {entry.error}</span>}
        </span>
        {entry.durationMs !== undefined && (
          <span className="shrink-0 text-neutral-600">{formatDuration(entry.durationMs)}</span>
        )}
        {hasDetails && (
          <ChevronRight size={12} className={`mt-0.5 shrink-0 text-neutral-600 ${expanded ? "rotate-90" : ""}`} />
        )}
      </button>
      {expanded && (
        <div className="space-y-1 px-2 pb-2 pl-24">
          {entry.error && <p className="whitespace-pre-wrap break-all text-red-300">{entry.error}</p>}
          {entry.args !== undefined && (
            <pre className="overflow-x-auto rounded bg-neutral-900 p-2 text-neutral-400">
              {JSON.stringify(entry.args, null, 2)}
            </pre>
          )}
        </div>
      )}
    </li>
  );
});

const iconButton = "rounded p-1 text-neutral-400 hover:bg-neutral-800 hover:text-neutral-100";

/** The log itself: application vs current-repository tabs, filter, and the
 * filter. Rendered in a modal. */
export function ActivityLogView({ onClose }: { onClose: () => void }) {
  const t = useT();
  const { overrides } = useShortcutOverrides();
  const entries = useActivityStore((s) => s.entries);
  const clear = useActivityStore((s) => s.clear);
  const repoPath = useEmbed((s) => s.repoPath);
  const [scope, setScope] = useState<Scope>(repoPath ? "repo" : "app");
  const [showReads, setShowReads] = useState(false);
  const [filter, setFilter] = useState("");

  const inScope = (e: ActivityEntry) => (scope === "app" ? e.repoPath === null : samePath(e.repoPath, repoPath));
  const needle = filter.trim().toLowerCase();
  const shown = useMemo(
    () =>
      entries.filter(
        (e) => inScope(e) && (showReads || !e.read) && (!needle || e.text.toLowerCase().includes(needle)),
      ),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [entries, scope, repoPath, showReads, needle],
  );
  const counts = useMemo(
    () => ({
      app: entries.filter((e) => e.repoPath === null && !e.read).length,
      repo: entries.filter((e) => samePath(e.repoPath, repoPath) && !e.read).length,
    }),
    [entries, repoPath],
  );

  // Follow new entries while scrolled to the bottom; stay put once the user scrolls up.
  const listRef = useRef<HTMLDivElement>(null);
  const atBottom = useRef(true);
  useLayoutEffect(() => {
    const el = listRef.current;
    if (el && atBottom.current) el.scrollTop = el.scrollHeight;
  }, [shown]);

  const tab = (id: Scope, label: string, count: number, disabled = false) => (
    <button
      onClick={() => setScope(id)}
      disabled={disabled}
      className={`min-w-0 truncate rounded px-2 py-1 ${
        scope === id ? "bg-neutral-800 text-neutral-100" : "text-neutral-400 hover:bg-neutral-900"
      } disabled:opacity-40`}
    >
      {label} <span className="text-neutral-500">{count}</span>
    </button>
  );

  return (
    <div className="flex h-full min-h-0 flex-col bg-neutral-950 text-xs">
      <div className="flex shrink-0 items-center gap-1 border-b border-neutral-800 px-2 py-1.5">
        <span className="mr-auto truncate text-sm font-semibold text-neutral-200">{t("gitActivity.title")}</span>
        <button
          onClick={() => setShowReads((x) => !x)}
          title={showReads ? t("gitActivity.hideReads") : t("gitActivity.showReads")}
          className={`${iconButton} ${showReads ? "text-blue-400" : ""}`}
        >
          {showReads ? <Eye size={14} /> : <EyeOff size={14} />}
        </button>
        <button onClick={() => clear((e) => !inScope(e))} title={t("gitActivity.clearTab")} className={iconButton}>
          <Trash2 size={14} />
        </button>
        <button onClick={onClose} title={t("gitActivity.close", { key: shortcutLabel("toggleActivityLog", overrides) })} className={iconButton}>
          <X size={14} />
        </button>
      </div>
      <div className="flex shrink-0 items-center gap-1 border-b border-neutral-800 px-2 py-1.5">
        {tab("app", t("gitActivity.tabApp"), counts.app)}
        {tab("repo", repoPath ? basename(repoPath) : t("gitActivity.tabRepo"), counts.repo, !repoPath)}
        <input
          value={filter}
          onChange={(e) => setFilter(e.currentTarget.value)}
          placeholder={t("gitActivity.filter")}
          className="ml-auto w-28 min-w-0 rounded bg-neutral-800 px-2 py-1 text-neutral-100 outline-none"
        />
      </div>
      <div
        ref={listRef}
        onScroll={(e) => {
          const el = e.currentTarget;
          atBottom.current = el.scrollHeight - el.scrollTop - el.clientHeight < 24;
        }}
        className="min-h-0 flex-1 overflow-y-auto font-mono"
      >
        {shown.length === 0 ? (
          <p className="p-4 text-center font-sans text-neutral-500">
            {scope === "repo" && !repoPath ? t("gitActivity.openRepoHint") : t("gitActivity.empty")}
          </p>
        ) : (
          <ul>
            {shown.map((e) => (
              <Row key={e.id} entry={e} />
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}
