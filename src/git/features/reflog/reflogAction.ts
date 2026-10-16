export type ReflogKind = "commit" | "checkout" | "reset" | "merge" | "rebase" | "cherry-pick" | "branch" | "other";

export type ReflogAction = {
  kind: ReflogKind;
  /** The action as git wrote it, e.g. "commit (amend)", "rebase (finish)". */
  label: string;
  /** What follows the action, e.g. the commit message or "moving from a to b". */
  detail: string;
};

const KINDS: [prefix: string, kind: ReflogKind][] = [
  ["commit", "commit"],
  ["checkout", "checkout"],
  ["reset", "reset"],
  ["merge", "merge"],
  ["pull", "merge"],
  ["rebase", "rebase"],
  ["cherry-pick", "cherry-pick"],
  ["revert", "cherry-pick"],
  ["branch", "branch"],
];

/** Splits a reflog message ("rebase (finish): returning to refs/heads/x")
 * into its action and detail. Git writes `<action>: <detail>`; anything
 * else is kept whole as the detail. */
export function parseReflogMessage(message: string): ReflogAction {
  const colon = message.indexOf(": ");
  if (colon === -1) return { kind: "other", label: "", detail: message };
  const label = message.slice(0, colon);
  const detail = message.slice(colon + 2);
  const head = label.toLowerCase();
  const kind = KINDS.find(([prefix]) => head === prefix || head.startsWith(`${prefix} `))?.[1] ?? "other";
  return { kind, label, detail };
}
