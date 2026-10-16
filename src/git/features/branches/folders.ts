import type { BranchInfo } from "./types";

export type BranchLeaf = { label: string; branch: BranchInfo };
export type BranchFolder = { name: string; path: string; folders: BranchFolder[]; leaves: BranchLeaf[] };

/** Groups names by their "/" segments (feat/x, feat/y → feat ▸ x, y), the way
 * most git clients show branch folders. `prefix` is stripped first (used to
 * drop the "origin/" part of remote branch names). */
export function buildFolders(branches: BranchInfo[], prefix: string): BranchFolder {
  const root: BranchFolder = { name: "", path: "", folders: [], leaves: [] };
  for (const branch of branches) {
    const parts = branch.name.slice(prefix.length).split("/");
    let folder = root;
    for (const part of parts.slice(0, -1)) {
      let next = folder.folders.find((f) => f.name === part);
      if (!next) {
        next = { name: part, path: `${folder.path}/${part}`, folders: [], leaves: [] };
        folder.folders.push(next);
      }
      folder = next;
    }
    folder.leaves.push({ label: parts[parts.length - 1], branch });
  }
  const sort = (f: BranchFolder) => {
    f.folders.sort((a, b) => a.name.localeCompare(b.name));
    f.leaves.sort((a, b) => a.label.localeCompare(b.label));
    f.folders.forEach(sort);
  };
  sort(root);
  return root;
}

/** Every branch under `folder`, nested folders included. */
export function folderBranches(folder: BranchFolder): BranchInfo[] {
  return [...folder.leaves.map((l) => l.branch), ...folder.folders.flatMap(folderBranches)];
}

/** Paths of `folder`'s nested folders (not its own), for expand/collapse all. */
export function nestedFolderPaths(folder: BranchFolder): string[] {
  return folder.folders.flatMap((f) => [f.path, ...nestedFolderPaths(f)]);
}
