import { t } from "../../../lib/i18n/react";
/** The remote used when nothing more specific applies: `origin` if there is
 * one, else the first. Same rule as the backend's push fallback. */
export function defaultRemote(names: readonly string[]): string | null {
  return names.includes("origin") ? "origin" : (names[0] ?? null);
}

/** Splits a remote-tracking branch name ("origin/feat/x") into its remote and
 * branch. The remote is the first segment, as git's default refspec lays it out. */
export function splitRemoteBranch(fullName: string): { remote: string; branch: string } {
  const idx = fullName.indexOf("/");
  return idx === -1
    ? { remote: fullName, branch: "" }
    : { remote: fullName.slice(0, idx), branch: fullName.slice(idx + 1) };
}

/** Problem with a remote name typed into the Remotes dialog, or null if it's
 * fine. Mirrors the common cases of git's own check (the backend has the final say). */
export function remoteNameProblem(name: string, existing: readonly string[], current?: string): string | null {
  const n = name.trim();
  if (!n) return t("gitRemote.nameRequired");
  if (n !== current && existing.includes(n)) return t("gitRemote.remoteExists", { name: n });
  if (/[\s~^:?*[\\]|\.\.|@\{|^[./-]|[./]$|\.lock$/.test(n)) return t("gitRemote.invalidRemoteName");
  return null;
}

/** Whether a background fetch may have moved refs, so the graph / branches /
 * status are worth reloading. `summary` null: it failed — with several remotes
 * the others may still have moved refs; with one, nothing changed. Skipping the
 * no-op case keeps periodic fetches from reloading big commit logs every round. */
export function fetchMovedRefs(summary: { updatedRefs: number } | null, remoteCount: number): boolean {
  return summary ? summary.updatedRefs > 0 : remoteCount > 1;
}

export const AUTO_FETCH_CHOICES = [0, 5, 10, 15, 30] as const;
export const DEFAULT_AUTO_FETCH_MINUTES = 10;

/** Open repos grouped by the `.git` dir they share (a repo and its
 * worktrees), keyed by the first of each group: they share remotes and
 * remote-tracking refs, so one fetch serves the whole group. */
export function groupSharedRepos(repos: readonly { repoId: string; commonDir?: string }[]): Map<string, string[]> {
  const byDir = new Map<string, string[]>();
  for (const { repoId, commonDir } of repos) {
    const key = commonDir ? commonDir.replace(/\\/g, "/").replace(/\/+$/, "") : `id:${repoId}`;
    const group = byDir.get(key) ?? [];
    if (!group.includes(repoId)) group.push(repoId);
    byDir.set(key, group);
  }
  return new Map([...byDir.values()].map((ids) => [ids[0], ids]));
}

/** Open repos due for a periodic fetch, the longest-waiting first: their last
 * fetch (of any kind) ended at least `intervalMs` ago — or never happened —
 * and none is running right now. */
export function dueForAutoFetch(
  repoIds: readonly string[],
  lastFetchedAt: (repoId: string) => number | undefined,
  isFetching: (repoId: string) => boolean,
  now: number,
  intervalMs: number,
): string[] {
  return [...new Set(repoIds)]
    .filter((id) => !isFetching(id))
    .map((id) => [id, lastFetchedAt(id) ?? 0] as const)
    .filter(([, last]) => now - last >= intervalMs)
    .sort((a, b) => a[1] - b[1])
    .map(([id]) => id);
}
