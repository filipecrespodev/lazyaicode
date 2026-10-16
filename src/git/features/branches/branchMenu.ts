import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { toast } from "sonner";
import { t } from "../../../lib/i18n/react";
import { openCreateRef } from "./createRefStore";
import type { useBranchActions } from "./useBranchActions";

/** `remotes`: the remotes this branch exists on (delete items act on each). */
export type MenuTarget = { name: string; isHead: boolean; hasLocal: boolean; remotes: string[] };
type Actions = ReturnType<typeof useBranchActions>;

/** "Copiar nome da branch/tag" — mesmo item no menu da sidebar e das pílulas do grafo. */
export const copyNameItem = (kind: "branch" | "tag", name: string) => ({
  label: t(kind === "tag" ? "gitBranches.copyTagName" : "gitBranches.copyBranchName"),
  onClick: () => {
    void writeText(name);
    toast.success(t(kind === "tag" ? "gitBranches.tagNameCopied" : "gitBranches.branchNameCopied"));
  },
});

/** Branch context-menu / drop-menu items, shared by the commit graph's pills
 * and the sidebar rows. `extra` = items only the caller can build (pin, MR).
 * Rebase moves HEAD on the backend, so it's offered only when `origin` is the
 * checked-out branch; merge and reset check the target out first if needed.
 * Drag-mode (dropping `origin` onto `target`) keeps only the comparison items. */
export function buildBranchMenuItems(
  a: Actions,
  anyMutating: boolean,
  target: MenuTarget,
  origin: string | null,
  originCheckedOut: boolean,
  dragMode: boolean,
  extra: {
    leading?: { label: string; onClick: () => void }[];
    comparison?: { label: string; onClick: () => void }[];
    /** Onde "Criar branch/tag a partir daqui" aponta (sha ou nome da branch). */
    startAt?: { repoId: string; start: string };
  } = {},
) {
  const busy = a.rebaseInProgress || anyMutating;
  const other = origin && origin !== target.name ? origin : null;
  return [
    ...(dragMode || !extra.startAt
      ? []
      : [
          { label: t("gitBranches.createBranchFrom", { name: target.name }), onClick: () => openCreateRef({ kind: "branch", ...extra.startAt! }), disabled: busy },
          { label: t("gitBranches.createTagAt", { name: target.name }), onClick: () => openCreateRef({ kind: "tag", ...extra.startAt! }), disabled: busy },
        ]),
    ...(dragMode ? [] : [...(extra.leading ?? []), copyNameItem("branch", target.name)]),
    ...(other && originCheckedOut
      ? [{ label: t("gitBranches.rebaseOnto", { other, target: target.name }), onClick: () => a.rebaseOnto.mutate(target.name), disabled: busy }]
      : []),
    ...(other
      ? [
          { label: t("gitBranches.mergeInto", { other, target: target.name }), onClick: () => a.mergeIntoWithFeedback({ origin: other, target: target.name }), disabled: busy },
          { label: t("gitBranches.resetTo", { other, target: target.name }), onClick: () => a.resetBranchToWithConfirm({ origin: other, target: target.name }), disabled: busy },
        ]
      : []),
    ...(other
      ? [
          {
            label: dragMode
              ? t("gitBranches.compareWith", { other, target: target.name })
              : t(originCheckedOut ? "gitBranches.compareToCurrent" : "gitBranches.compareTo", { other }),
            onClick: () => a.compareBranches(target.name, other),
          },
        ]
      : []),
    ...(other ? (extra.comparison ?? []) : []),
    ...(dragMode
      ? []
      : [
          ...(a.hasCommitsToPush(target.name)
            ? [{ label: t("gitBranches.pushToOrigin", { name: target.name }), onClick: () => a.pushToOrigin(target.name), disabled: busy }]
            : []),
          ...(target.hasLocal && !target.isHead
            ? [{ label: t("gitBranches.deleteLocal"), onClick: () => a.deleteLocalWithConfirm(target.name) }]
            : []),
          ...target.remotes.map((remote) => ({
            label: t("gitBranches.deleteBranchOnRemote", { remote }),
            onClick: () => a.deleteRemoteWithConfirm(target.name, remote),
          })),
          ...(target.hasLocal && target.remotes.length > 0 && !target.isHead
            ? [{ label: t("gitBranches.deleteBothRemotes", { remotes: target.remotes.join(", ") }), onClick: () => a.deleteBothWithConfirm(target.name, target.remotes) }]
            : []),
        ]),
  ];
}
