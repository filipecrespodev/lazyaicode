import { useMemo, type RefObject } from "react";
import type { HunkData } from "react-diff-view";
import { useT } from "../../../lib/i18n/react";

type MarkerType = "insert" | "delete";
type Marker = { top: number; height: number; type: MarkerType };

// Git always emits a replaced line's delete rows before its insert rows —
// segmenting by contiguous same-type runs naturally gives a red segment
// immediately followed by a green one for a modified region (instead of
// merging them into a single ambiguous "mixed" color).
function computeMarkers(hunks: HunkData[]): Marker[] {
  const rows: Array<"normal" | "insert" | "delete"> = [];
  hunks.forEach((hunk) => hunk.changes.forEach((change) => rows.push(change.type)));

  const total = rows.length || 1;
  const markers: Marker[] = [];
  let i = 0;
  while (i < rows.length) {
    const type = rows[i];
    if (type === "normal") {
      i++;
      continue;
    }
    const start = i;
    while (i < rows.length && rows[i] === type) i++;
    markers.push({
      top: start / total,
      height: Math.max((i - start) / total, 0.004),
      type,
    });
  }
  return markers;
}

const COLOR: Record<MarkerType, string> = {
  insert: "bg-emerald-500",
  delete: "bg-red-500",
};

/** A GitKraken-style change strip alongside the full-file diff: a colored
 * tick per change block, proportional to its position in the file, clicking
 * scrolls the diff to it. */
export function DiffMinimap({
  hunks,
  scrollRef,
}: {
  hunks: HunkData[];
  scrollRef: RefObject<HTMLDivElement | null>;
}) {
  const t = useT();
  const markers = useMemo(() => computeMarkers(hunks), [hunks]);

  const jumpTo = (ratio: number) => {
    const el = scrollRef.current;
    if (!el) return;
    const target = ratio * (el.scrollHeight - el.clientHeight);
    el.scrollTo({ top: Math.max(target, 0), behavior: "smooth" });
  };

  if (markers.length === 0) return null;

  return (
    <div
      className="relative w-3 shrink-0 cursor-pointer bg-neutral-900"
      title={t("gitDiff.jumpToChange")}
      onClick={(e) => {
        const rect = e.currentTarget.getBoundingClientRect();
        jumpTo((e.clientY - rect.top) / rect.height);
      }}
    >
      {markers.map((m, i) => (
        <div
          key={i}
          onClick={(e) => {
            e.stopPropagation();
            jumpTo(m.top);
          }}
          className={`absolute inset-x-0.5 rounded-sm ${COLOR[m.type]}`}
          style={{ top: `${m.top * 100}%`, height: `${m.height * 100}%` }}
        />
      ))}
    </div>
  );
}
