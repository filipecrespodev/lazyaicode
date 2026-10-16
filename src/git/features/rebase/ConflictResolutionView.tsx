import { Bot, Check, ChevronDown, ChevronUp, Loader2, Plus, RotateCcw } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { parseDiff } from "react-diff-view";
import RawEditor from "react-simple-code-editor";
import { toast } from "sonner";
import { resolveConflictWithAi } from "@/features/ai/api";
import { useAiSettings } from "@/features/ai/useAiSettings";
import { errorMessage } from "@/lib/tauri-client";
import { guessLanguage, highlightLine } from "@/lib/syntax";
import { useResolveConflict } from "./useResolveConflict";
import type { ConflictedFile } from "./types";
import { useT } from "../../../lib/i18n/react";

// react-simple-code-editor's CJS build has no `__esModule` marker, so Vite's
// dev-time interop resolves the default import to the whole module object
// instead of the component — unwrap it defensively.
const Editor = (RawEditor as unknown as { default?: typeof RawEditor }).default ?? RawEditor;

type RowType = "normal" | "insert" | "delete";
type Row = { id: string; type: RowType; content: string };
type RenderItem = { kind: "row"; row: Row } | { kind: "block"; id: string; rows: Row[] };

function flattenRows(patch: string): Row[] {
  let file;
  try {
    file = parseDiff(patch)[0];
  } catch {
    return [];
  }
  if (!file) return [];

  const rows: Row[] = [];
  file.hunks.forEach((hunk, hi) => {
    hunk.changes.forEach((change, ci) => {
      rows.push({ id: `${hi}-${ci}`, type: change.type, content: change.content });
    });
  });
  return rows;
}

/** Groups consecutive changed (non-normal) rows into blocks, each rendered
 * with its own single accept/reject-all toggle in addition to the existing
 * per-line ones — context rows break a block and pass through untouched.
 * Single-line blocks skip the extra toggle: it'd be a redundant twin of the
 * line's own. */
function groupRows(rows: Row[]): RenderItem[] {
  const items: RenderItem[] = [];
  let i = 0;
  while (i < rows.length) {
    const row = rows[i];
    if (row.type === "normal") {
      items.push({ kind: "row", row });
      i++;
      continue;
    }
    const block: Row[] = [];
    while (i < rows.length && rows[i].type !== "normal") {
      block.push(rows[i]);
      i++;
    }
    if (block.length > 1) {
      items.push({ kind: "block", id: block[0].id, rows: block });
    }
    block.forEach((row) => items.push({ kind: "row", row }));
  }
  return items;
}

/** Splits the full (unfiltered) row list into conflict groups — contiguous
 * runs of changed lines — used for prev/next navigation across both panes. */
function conflictGroupsOf(rows: Row[]): Row[][] {
  const groups: Row[][] = [];
  let i = 0;
  while (i < rows.length) {
    if (rows[i].type === "normal") {
      i++;
      continue;
    }
    const start = i;
    while (i < rows.length && rows[i].type !== "normal") i++;
    groups.push(rows.slice(start, i));
  }
  return groups;
}

/** Finds the row id to scroll a given side to for a conflict group: a row of
 * `wantType` inside the group if the group has one, else the nearest
 * preceding context line (visible on both sides). */
function anchorId(rows: Row[], group: Row[], wantType: RowType): string | null {
  const direct = group.find((r) => r.type === wantType);
  if (direct) return direct.id;
  const startIdx = rows.findIndex((r) => r.id === group[0].id);
  for (let j = startIdx - 1; j >= 0; j--) {
    if (rows[j].type === "normal") return rows[j].id;
  }
  return null;
}

export function ConflictResolutionView({
  repoId,
  file,
  onResolved,
  onClose,
}: {
  repoId: string;
  file: ConflictedFile;
  onResolved: () => void;
  onClose: () => void;
}) {
  const t = useT();
  const { resolve } = useResolveConflict(repoId);
  const language = file.isBinary ? null : guessLanguage(file.path);
  const rows = useMemo(
    () => (file.isBinary ? [] : flattenRows(file.patch)),
    [file.patch, file.isBinary],
  );
  const conflictGroups = useMemo(() => conflictGroupsOf(rows), [rows]);
  const [accepted, setAccepted] = useState<Set<string>>(new Set());
  const [manualOutput, setManualOutput] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [aiBusy, setAiBusy] = useState(false);
  const { settings: ai } = useAiSettings();
  const [groupIdx, setGroupIdx] = useState(0);

  const scrollToGroup = (idx: number) => {
    const group = conflictGroups[idx];
    if (!group) return;
    const leftId = anchorId(rows, group, "delete");
    const rightId = anchorId(rows, group, "insert");
    if (leftId) document.getElementById(`row-left-${leftId}`)?.scrollIntoView({ block: "center" });
    if (rightId) document.getElementById(`row-right-${rightId}`)?.scrollIntoView({ block: "center" });
  };

  useEffect(() => {
    setGroupIdx(0);
    // Wait a tick for the panes to actually render before scrolling.
    const t = setTimeout(() => scrollToGroup(0), 0);
    return () => clearTimeout(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [file.path]);

  const goToGroup = (idx: number) => {
    if (idx < 0 || idx >= conflictGroups.length) return;
    setGroupIdx(idx);
    scrollToGroup(idx);
  };

  const outputLines = useMemo(
    () => rows.filter((r) => r.type === "normal" || accepted.has(r.id)),
    [rows, accepted],
  );
  const derivedOutput = useMemo(() => outputLines.map((r) => r.content).join("\n"), [outputLines]);
  const output = manualOutput ?? derivedOutput;

  const toggle = (id: string) => {
    setAccepted((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const toggleMany = (ids: string[], accept: boolean) => {
    setAccepted((prev) => {
      const next = new Set(prev);
      ids.forEach((id) => (accept ? next.add(id) : next.delete(id)));
      return next;
    });
  };

  // Fills the editable output with the model's merge; nothing is saved until
  // the user presses Save.
  const resolveWithAi = async () => {
    setAiBusy(true);
    try {
      setManualOutput(
        await resolveConflictWithAi({
          model: ai.commitMessageModel,
          path: file.path,
          ancestor: file.ancestorContent,
          ours: file.oursContent ?? "",
          theirs: file.theirsContent ?? "",
          customInstructions: ai.conflictCustomInstructions,
        }),
      );
    } catch (e) {
      toast.error(errorMessage(e));
    } finally {
      setAiBusy(false);
    }
  };

  const save = async () => {
    setSaving(true);
    const ok = await resolve(file.path, { kind: "UseManualContent", content: output });
    setSaving(false);
    if (ok) onResolved();
  };

  // Colors the accepted output by which side each line came from — only
  // meaningful while following the derived selection; once manually edited,
  // line indices no longer map to a known origin.
  const highlightOutput = (code: string) => {
    const lines = code.split("\n");
    return lines.map((line, i) => {
      const origin = manualOutput === null ? outputLines[i]?.type : undefined;
      const bg = origin === "delete" ? "bg-red-950/40" : origin === "insert" ? "bg-emerald-950/40" : "";
      return (
        <span key={i} className={`block ${bg}`}>
          {line ? highlightLine(line, language) : " "}
        </span>
      );
    });
  };

  if (file.isBinary) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-2 text-sm text-neutral-400">
        <p className="font-mono">{file.path}</p>
        <p>{t("gitRebase.binaryFile")}</p>
        <button onClick={onClose} className="rounded bg-neutral-800 px-3 py-1.5 hover:bg-neutral-700">
          {t("gitRebase.back")}
        </button>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <div className="flex shrink-0 items-center justify-between border-b border-neutral-800 bg-neutral-900 px-3 py-2 text-sm text-neutral-200">
        <span className="min-w-0 truncate font-mono">{file.path}</span>
        <div className="flex shrink-0 items-center gap-3">
          {conflictGroups.length > 0 && (
            <div className="flex items-center gap-1 text-xs text-neutral-400">
              <span>
                {t("gitRebase.conflictOf", { index: groupIdx + 1, total: conflictGroups.length })}
              </span>
              <button
                onClick={() => goToGroup(groupIdx - 1)}
                disabled={groupIdx <= 0}
                title={t("gitRebase.prevConflict")}
                className="rounded p-0.5 hover:bg-neutral-800 disabled:opacity-30"
              >
                <ChevronUp size={14} />
              </button>
              <button
                onClick={() => goToGroup(groupIdx + 1)}
                disabled={groupIdx >= conflictGroups.length - 1}
                title={t("gitRebase.nextConflict")}
                className="rounded p-0.5 hover:bg-neutral-800 disabled:opacity-30"
              >
                <ChevronDown size={14} />
              </button>
            </div>
          )}
          <div className="flex shrink-0 gap-2">
            <button
              onClick={resolveWithAi}
              disabled={aiBusy || saving || file.oursContent === null || file.theirsContent === null}
              title={t("gitRebase.resolveWithAiTitle")}
              className="flex items-center gap-1 rounded bg-neutral-800 px-2 py-1 text-xs hover:bg-neutral-700 disabled:opacity-50"
            >
              {aiBusy ? <Loader2 size={12} className="animate-spin" /> : <Bot size={12} />} {t("gitRebase.resolveWithAi")}
            </button>
            <button onClick={onClose} className="rounded bg-neutral-800 px-2 py-1 text-xs hover:bg-neutral-700">
              {t("gitRebase.back")}
            </button>
            <button
              onClick={save}
              disabled={saving}
              className="rounded bg-emerald-800 px-2 py-1 text-xs hover:bg-emerald-700 disabled:opacity-50"
            >
              {t("gitRebase.save")}
            </button>
          </div>
        </div>
      </div>

      <div className="grid min-h-0 flex-1 grid-cols-2 divide-x divide-neutral-800 overflow-hidden">
        <Pane
          side="left"
          title={t("gitRebase.current")}
          rows={rows.filter((r) => r.type !== "insert")}
          accepted={accepted}
          onToggle={toggle}
          onToggleMany={toggleMany}
          language={language}
        />
        <Pane
          side="right"
          title={t("gitRebase.incoming")}
          rows={rows.filter((r) => r.type !== "delete")}
          accepted={accepted}
          onToggle={toggle}
          onToggleMany={toggleMany}
          language={language}
        />
      </div>

      <div className="flex shrink-0 flex-col border-t border-neutral-800" style={{ height: "35%" }}>
        <div className="flex shrink-0 items-center justify-between bg-neutral-900 px-3 py-1.5 text-xs uppercase text-neutral-500">
          <span>{t("gitRebase.result")}</span>
          {manualOutput !== null && (
            <button
              onClick={() => setManualOutput(null)}
              className="flex items-center gap-1 rounded bg-neutral-800 px-2 py-0.5 normal-case text-neutral-300 hover:bg-neutral-700"
            >
              <RotateCcw size={11} /> {t("gitRebase.restore")}
            </button>
          )}
        </div>
        <div className="flex-1 overflow-auto bg-neutral-950">
          <Editor
            value={output}
            onValueChange={setManualOutput}
            highlight={highlightOutput}
            padding={8}
            textareaClassName="outline-none"
            style={{ fontFamily: "var(--font-mono, ui-monospace, monospace)", fontSize: "var(--code-size, 12px)", lineHeight: "18px", minHeight: "100%" }}
          />
        </div>
      </div>
    </div>
  );
}

function Pane({
  side,
  title,
  rows,
  accepted,
  onToggle,
  onToggleMany,
  language,
}: {
  side: "left" | "right";
  title: string;
  rows: Row[];
  accepted: Set<string>;
  onToggle: (id: string) => void;
  onToggleMany: (ids: string[], accept: boolean) => void;
  language: string | null;
}) {
  const t = useT();
  const items = useMemo(() => groupRows(rows), [rows]);

  return (
    <div className="flex min-h-0 flex-col overflow-hidden">
      <div className="shrink-0 bg-neutral-900 px-3 py-1.5 text-xs font-medium text-neutral-300">{title}</div>
      <div className="flex-1 overflow-auto font-mono text-xs">
        {items.map((item) => {
          if (item.kind === "block") {
            const ids = item.rows.map((r) => r.id);
            const blockAccepted = ids.every((id) => accepted.has(id));
            return (
              <div
                key={`block-${item.id}`}
                className="flex items-center gap-2 border-y border-dashed border-neutral-700 bg-neutral-900/60 px-2 py-0.5"
              >
                <button
                  onClick={() => onToggleMany(ids, !blockAccepted)}
                  title={blockAccepted ? t("gitRebase.removeBlock") : t("gitRebase.addBlock")}
                  className={`flex h-4 w-4 shrink-0 items-center justify-center rounded ${
                    blockAccepted ? "bg-emerald-700 text-emerald-50" : "bg-neutral-700 text-neutral-300"
                  }`}
                >
                  {blockAccepted ? <Check size={10} /> : <Plus size={10} />}
                </button>
                <span className="text-[10px] uppercase tracking-wide text-neutral-500">
                  {t(ids.length === 1 ? "gitRebase.blockLinesOne" : "gitRebase.blockLinesOther", { count: ids.length })}
                </span>
              </div>
            );
          }

          const row = item.row;
          return (
            <div
              key={row.id}
              id={`row-${side}-${row.id}`}
              className={`flex items-start gap-2 px-2 py-0.5 ${
                row.type === "delete" ? "bg-red-950/40" : row.type === "insert" ? "bg-emerald-950/40" : ""
              }`}
            >
              {row.type === "normal" ? (
                <span className="w-4 shrink-0" />
              ) : (
                <button
                  onClick={() => onToggle(row.id)}
                  title={accepted.has(row.id) ? t("gitRebase.removeLine") : t("gitRebase.addLine")}
                  className={`flex h-4 w-4 shrink-0 items-center justify-center rounded ${
                    accepted.has(row.id) ? "bg-emerald-700 text-emerald-50" : "bg-neutral-800 text-neutral-400"
                  }`}
                >
                  {accepted.has(row.id) ? <Check size={10} /> : <Plus size={10} />}
                </button>
              )}
              <span className="whitespace-pre-wrap break-all text-neutral-200">
                {row.content ? highlightLine(row.content, language) : " "}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
