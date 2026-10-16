import { ChevronDown, ChevronRight, Folder, FolderOpen, List, ListTree } from "lucide-react";
import { useMemo, useState } from "react";
import { useContextMenu, type ContextMenuItem } from "@/components/ContextMenu";
import { usePersistedValue } from "@/lib/use-persisted-config";
import type { FileDiff } from "./types";
import { useT } from "../../../lib/i18n/react";

type ViewMode = "list" | "tree";

function statusLabel(status: FileDiff["status"]): string {
  switch (status) {
    case "Added":
      return "A";
    case "Deleted":
      return "D";
    case "Renamed":
      return "R";
    case "Copied":
      return "C";
    case "TypeChange":
      return "T";
    case "Modified":
      return "M";
    default:
      return "?";
  }
}

type DirNode = {
  name: string;
  path: string;
  dirs: DirNode[];
  files: FileDiff[];
};

/** Builds a folder tree from flat paths, collapsing single-child folder
 * chains ("features/home/components") into one row like VS Code does. */
function buildTree(files: FileDiff[]): DirNode {
  type MutableDir = { name: string; path: string; dirs: Map<string, MutableDir>; files: FileDiff[] };
  const root: MutableDir = { name: "", path: "", dirs: new Map(), files: [] };

  for (const file of files) {
    const parts = file.path.split("/");
    let dir = root;
    for (const part of parts.slice(0, -1)) {
      let next = dir.dirs.get(part);
      if (!next) {
        next = { name: part, path: dir.path ? `${dir.path}/${part}` : part, dirs: new Map(), files: [] };
        dir.dirs.set(part, next);
      }
      dir = next;
    }
    dir.files.push(file);
  }

  const finalize = (dir: MutableDir): DirNode => {
    let current = dir;
    let name = dir.name;
    while (current !== root && current.files.length === 0 && current.dirs.size === 1) {
      current = [...current.dirs.values()][0];
      name = `${name}/${current.name}`;
    }
    return {
      name,
      path: current.path,
      dirs: [...current.dirs.values()].map(finalize).sort((a, b) => a.name.localeCompare(b.name)),
      files: [...current.files].sort((a, b) => a.path.localeCompare(b.path)),
    };
  };

  return finalize(root);
}

const fileName = (path: string) => path.slice(path.lastIndexOf("/") + 1);

function FileRow({
  file,
  label,
  depth,
  selected,
  onOpen,
  onContextMenu,
}: {
  file: FileDiff;
  label: string;
  depth: number;
  selected: boolean;
  onOpen: () => void;
  onContextMenu?: (e: React.MouseEvent) => void;
}) {
  return (
    <li
      onClick={onOpen}
      onContextMenu={onContextMenu}
      title={file.path}
      className={`flex cursor-pointer items-center gap-2 py-1 pr-3 ${
        selected ? "bg-neutral-800" : "hover:bg-neutral-900"
      }`}
      style={{ paddingLeft: 12 + depth * 14 }}
    >
      <span className="w-4 shrink-0 text-center font-mono text-xs text-neutral-500">
        {statusLabel(file.status)}
      </span>
      <span className="min-w-0 flex-1 truncate text-xs text-neutral-300">{label}</span>
    </li>
  );
}

export function ChangedFileList({
  title,
  files,
  isPending,
  selectedFilePath,
  onOpenFile,
  fileMenu,
}: {
  title: string;
  files: FileDiff[];
  isPending: boolean;
  selectedFilePath: string | null;
  onOpenFile: (file: FileDiff) => void;
  /** Right-click items for a file; no menu when omitted. */
  fileMenu?: (file: FileDiff) => ContextMenuItem[];
}) {
  const { open: openMenu, menu } = useContextMenu();
  const onFileContextMenu = fileMenu ? (file: FileDiff) => (e: React.MouseEvent) => openMenu(e, fileMenu(file)) : null;
  const t = useT();
  const [mode, setMode] = usePersistedValue<ViewMode>("ui.changedFilesView", "list");
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const tree = useMemo(() => buildTree(files), [files]);

  const toggleDir = (path: string) =>
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });

  const renderDir = (dir: DirNode, depth: number): React.ReactNode[] => [
    ...dir.dirs.map((sub) => {
      const isCollapsed = collapsed.has(sub.path);
      return (
        <li key={`d:${sub.path}`}>
          <div
            onClick={() => toggleDir(sub.path)}
            className="flex cursor-pointer items-center gap-1 py-1 pr-3 text-xs text-neutral-400 hover:bg-neutral-900"
            style={{ paddingLeft: 8 + depth * 14 }}
            title={sub.path}
          >
            {isCollapsed ? (
              <ChevronRight size={12} className="shrink-0" />
            ) : (
              <ChevronDown size={12} className="shrink-0" />
            )}
            {isCollapsed ? (
              <Folder size={12} className="shrink-0" />
            ) : (
              <FolderOpen size={12} className="shrink-0" />
            )}
            <span className="min-w-0 flex-1 truncate">{sub.name}</span>
          </div>
          {!isCollapsed && <ul>{renderDir(sub, depth + 1)}</ul>}
        </li>
      );
    }),
    ...dir.files.map((file) => (
      <FileRow
        key={file.path}
        file={file}
        label={fileName(file.path)}
        depth={depth}
        selected={selectedFilePath === file.path}
        onOpen={() => onOpenFile(file)}
        onContextMenu={onFileContextMenu?.(file)}
      />
    )),
  ];

  const toggleClass = (active: boolean) =>
    `rounded p-1 ${active ? "bg-neutral-800 text-neutral-200" : "text-neutral-500 hover:text-neutral-300"}`;

  return (
    <div className="flex-1 overflow-auto">
      <div className="sticky top-0 z-10 flex items-center justify-between bg-neutral-950 px-3 py-1">
        <p className="text-xs uppercase text-neutral-500">{title}</p>
        <div className="flex items-center gap-0.5">
          <button title={t("gitDiff.viewList")} onClick={() => setMode("list")} className={toggleClass(mode === "list")}>
            <List size={13} />
          </button>
          <button title={t("gitDiff.viewTree")} onClick={() => setMode("tree")} className={toggleClass(mode === "tree")}>
            <ListTree size={13} />
          </button>
        </div>
      </div>
      {isPending && <p className="px-3 py-1 text-neutral-500">{t("gitDiff.loading")}</p>}
      <ul>
        {mode === "list"
          ? files.map((file) => (
              <FileRow
                key={file.path}
                file={file}
                label={file.path}
                depth={0}
                selected={selectedFilePath === file.path}
                onOpen={() => onOpenFile(file)}
                onContextMenu={onFileContextMenu?.(file)}
              />
            ))
          : renderDir(tree, 0)}
      </ul>
      {menu}
    </div>
  );
}
