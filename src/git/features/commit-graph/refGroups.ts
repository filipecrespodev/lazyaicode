import { splitRemoteBranch } from "@/features/remote-ops/remotes";
import type { RefInfo } from "./types";

export type GroupedRef = {
  name: string;
  isHead: boolean;
  isTag: boolean;
  hasLocal: boolean;
  hasRemote: boolean;
  /** Remotes this branch is on (at this commit), e.g. ["origin", "upstream"]. */
  remotes: string[];
};

/** Merges a local branch and its remote-tracking counterpart (e.g. "main" +
 * "origin/main", "upstream/main") into a single pill for the same commit row. */
export function groupRefs(refs: RefInfo[]): GroupedRef[] {
  const byKey = new Map<string, GroupedRef>();

  for (const ref of refs) {
    if (ref.kind === "Tag") {
      byKey.set(`tag:${ref.name}`, {
        name: ref.name,
        isHead: false,
        isTag: true,
        hasLocal: false,
        hasRemote: false,
        remotes: [],
      });
      continue;
    }

    if (ref.kind === "DetachedHead") {
      byKey.set(`head:${ref.name}`, {
        name: ref.name,
        isHead: true,
        isTag: false,
        hasLocal: false,
        hasRemote: false,
        remotes: [],
      });
      continue;
    }

    const remote = ref.kind === "RemoteBranch" ? splitRemoteBranch(ref.name) : null;
    const baseName = remote ? remote.branch : ref.name;
    const key = `branch:${baseName}`;
    const existing = byKey.get(key) ?? {
      name: baseName,
      isHead: false,
      isTag: false,
      hasLocal: false,
      hasRemote: false,
      remotes: [],
    };
    if (ref.kind === "LocalBranch") {
      existing.hasLocal = true;
      existing.isHead = existing.isHead || ref.isHead;
    } else if (remote) {
      existing.hasRemote = true;
      existing.remotes.push(remote.remote);
    }
    byKey.set(key, existing);
  }

  return [...byKey.values()];
}
