import { useQuery } from "@tanstack/react-query";
import { Search } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Modal } from "@/components/Modal";
import { searchCommits, type SearchMode } from "./api";
import type { CommitNode } from "./types";
import { useT } from "../../../lib/i18n/react";

const MODES: SearchMode[] = ["message", "author", "sha", "file", "diff"];

export function CommitSearchPalette({
  repoId,
  onClose,
  onPick,
}: {
  repoId: string;
  onClose: () => void;
  onPick: (commit: CommitNode) => void;
}) {
  const t = useT();
  const [mode, setMode] = useState<SearchMode>("message");
  const [query, setQuery] = useState("");
  const [debounced, setDebounced] = useState("");
  const [selected, setSelected] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => inputRef.current?.focus(), []);
  useEffect(() => {
    const t = setTimeout(() => setDebounced(query.trim()), 250);
    return () => clearTimeout(t);
  }, [query]);
  useEffect(() => setSelected(0), [debounced, mode]);

  const { data: results = [], isFetching } = useQuery({
    queryKey: ["commitSearch", repoId, mode, debounced],
    queryFn: () => searchCommits(repoId, debounced, mode),
    enabled: debounced.length > 0,
  });

  const pick = (i: number) => {
    const commit = results[i];
    if (commit) {
      onPick(commit);
      onClose();
    }
  };

  return (
    <Modal onClose={onClose} align="top" width="w-[640px]">
      <div
        onKeyDown={(e) => {
          if (e.key === "ArrowDown") {
            e.preventDefault();
            setSelected((s) => Math.min(s + 1, results.length - 1));
          } else if (e.key === "ArrowUp") {
            e.preventDefault();
            setSelected((s) => Math.max(s - 1, 0));
          } else if (e.key === "Enter") {
            e.preventDefault();
            pick(selected);
          }
        }}
      >
        <div className="flex items-center gap-2 border-b border-neutral-800 px-3 py-2.5">
          <Search size={15} className="shrink-0 text-neutral-500" />
          <input
            ref={inputRef}
            value={query}
            onChange={(e) => setQuery(e.currentTarget.value)}
            placeholder={t("gitGraph.searchPlaceholder")}
            className="min-w-0 flex-1 bg-transparent text-sm text-neutral-100 outline-none placeholder:text-neutral-500"
          />
          {isFetching && <span className="text-xs text-neutral-500">{t("gitGraph.searching")}</span>}
        </div>
        <div className="flex gap-1 border-b border-neutral-800 px-3 py-1.5">
          {MODES.map((m) => (
            <button
              key={m}
              onClick={() => {
                setMode(m);
                inputRef.current?.focus();
              }}
              className={`rounded px-2 py-0.5 text-xs ${
                mode === m ? "bg-neutral-700 text-neutral-100" : "text-neutral-400 hover:bg-neutral-800"
              }`}
            >
              {t(`gitGraph.searchMode_${m}`)}
            </button>
          ))}
        </div>
        <ul className="max-h-80 overflow-auto p-1">
          {results.map((c, i) => (
            <li
              key={c.sha}
              onMouseEnter={() => setSelected(i)}
              onClick={() => pick(i)}
              className={`flex cursor-pointer items-center gap-3 rounded px-3 py-1.5 ${
                i === selected ? "bg-neutral-800" : "hover:bg-neutral-900"
              }`}
            >
              <span className="shrink-0 font-mono text-xs text-neutral-500">{c.shortSha}</span>
              <span className="min-w-0 flex-1 truncate text-sm text-neutral-100">{c.summary}</span>
              <span className="shrink-0 text-xs text-neutral-500">{c.authorName}</span>
            </li>
          ))}
          {debounced && !isFetching && results.length === 0 && (
            <li className="px-3 py-2 text-sm text-neutral-500">{t("gitGraph.noCommitsFound")}</li>
          )}
        </ul>
      </div>
    </Modal>
  );
}
