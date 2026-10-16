import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import type { ContextMenuItem } from "@/components/ContextMenu";
import { getSubmoduleCount, submoduleCountKey } from "@/features/submodules/api";
import { SubmodulesModal } from "@/features/submodules/SubmodulesModal";
import { WorktreesModal } from "@/features/worktrees/WorktreesModal";
import { useT } from "../../../lib/i18n/react";

/** Worktrees / Submodules entries for the repo "..." menu, plus their modals.
 * Submodules only shows up when the repo has any. */
export function useRepoToolsMenu(repoId: string, repoPath: string) {
  const t = useT();
  const [open, setOpen] = useState<"worktrees" | "submodules" | null>(null);
  // Only decides whether the entry is shown: a count, not the status walk
  // the modal does (which it refreshes on open).
  const { data: submoduleCount } = useQuery({
    queryKey: submoduleCountKey(repoId),
    queryFn: () => getSubmoduleCount(repoId),
    staleTime: 5 * 60_000,
  });

  const items: ContextMenuItem[] = [
    { label: t("gitRepo.worktrees"), onClick: () => setOpen("worktrees") },
    ...(submoduleCount ? [{ label: t("gitRepo.submodulesCount", { count: submoduleCount }), onClick: () => setOpen("submodules") }] : []),
  ];

  const close = () => setOpen(null);
  const modals =
    open === "worktrees" ? (
      <WorktreesModal repoId={repoId} repoPath={repoPath} onClose={close} />
    ) : open === "submodules" ? (
      <SubmodulesModal repoId={repoId} onClose={close} />
    ) : null;

  return { items, modals };
}
