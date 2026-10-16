import type { CommitNode } from "@/features/commit-graph/types";
import { t } from "../../../lib/i18n/react";

export type TodoAction = "pick" | "reword" | "squash" | "fixup" | "drop";

export const TODO_ACTIONS: TodoAction[] = ["pick", "reword", "squash", "fixup", "drop"];

export type TodoRow = {
  commit: CommitNode;
  action: TodoAction;
  /** Edited message, used when `action` is reword. */
  message: string;
};

/** What `start_interactive_rebase` takes, oldest first. */
export type TodoItem = { sha: string; action: TodoAction; message: string | null };

export function fullMessage(commit: CommitNode): string {
  return commit.body ? `${commit.summary}\n\n${commit.body}` : commit.summary;
}

export function initialRows(commits: CommitNode[]): TodoRow[] {
  return commits.map((commit) => ({ commit, action: "pick", message: fullMessage(commit) }));
}

export function moveRow<T>(rows: T[], from: number, to: number): T[] {
  if (from === to || from < 0 || to < 0 || from >= rows.length || to >= rows.length) return rows;
  const next = rows.slice();
  const [row] = next.splice(from, 1);
  next.splice(to, 0, row);
  return next;
}

/** Why the list can't run, or null. Mirrors the backend's checks. */
export function validateRows(rows: TodoRow[]): string | null {
  const kept = rows.filter((r) => r.action !== "drop");
  if (kept.length === 0) return t("gitInteractiveRebase.allDropped");
  if (kept[0].action === "squash" || kept[0].action === "fixup") {
    return t("gitInteractiveRebase.firstSquash");
  }
  const emptyReword = rows.find((r) => r.action === "reword" && !r.message.trim());
  if (emptyReword) return t("gitInteractiveRebase.emptyReword", { sha: emptyReword.commit.shortSha });
  return null;
}

/** True when running would change nothing (all picks, original order). */
export function isUnchanged(rows: TodoRow[], original: CommitNode[]): boolean {
  return rows.every(
    (r, i) =>
      r.commit.sha === original[i]?.sha &&
      (r.action === "pick" || (r.action === "reword" && r.message.trim() === fullMessage(r.commit).trim())),
  );
}

/** A reword whose message wasn't actually changed goes as a plain pick, so
 * the commit isn't amended (new committer date, rerun post-commit hook) for nothing. */
export function toItems(rows: TodoRow[]): TodoItem[] {
  return rows.map((r) => {
    const reword = r.action === "reword" && r.message.trim() !== fullMessage(r.commit).trim();
    return {
      sha: r.commit.sha,
      action: r.action === "reword" && !reword ? "pick" : r.action,
      message: reword ? r.message : null,
    };
  });
}

/** Single-key shortcuts on a focused row, like git's todo verbs. */
export function actionForKey(key: string): TodoAction | null {
  return ({ p: "pick", r: "reword", s: "squash", f: "fixup", d: "drop" } as const)[key.toLowerCase()] ?? null;
}
