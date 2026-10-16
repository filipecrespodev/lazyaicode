import { useIsMutating, useQuery } from "@tanstack/react-query";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Ellipsis, GitMerge, Loader2, Package, Redo2, Undo2 } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { useContextMenu } from "@/components/ContextMenu";
import { HOSTING_LABELS } from "@/features/integrations/provider";
import { useHostingRemote } from "@/features/integrations/useHostingRemote";
import { useOpenMergeRequests } from "@/features/integrations/useOpenMergeRequests";
import { RemotesModal } from "@/features/remote-ops/RemotesModal";
import { RemoteOpsButtons } from "@/features/remote-ops/RemoteOpsButtons";
import { useStashActions } from "@/features/stash/useStashActions";
import { useUndo } from "@/features/undo/useUndo";
import { useEmbed } from "@/embed";
import { formatDateTime } from "@/lib/format-date";
import { queryKeys } from "@/lib/query-keys";
import { getRepoStatus, type RepoSummary } from "./api";
import { useRepoToolsMenu } from "./useRepoToolsMenu";
import { useT } from "../../../lib/i18n/react";
import { shortcutLabel } from "../../../lib/shortcuts/shortcuts";
import { useShortcutOverrides } from "@/features/shortcuts/useShortcuts";

/** Desfazer/refazer, Fetch, Push e Stash do repositório, na barra de abas da
 * thread (valem em qualquer aba, não só nas do git). Ctrl+Z/Ctrl+Shift+Z só
 * desfazem ação git com uma aba git visível. */
export function RepoActions({ repo }: { repo: RepoSummary }) {
  const t = useT();
  const { overrides } = useShortcutOverrides();
  const anyMutating = useIsMutating() > 0;
  const { save: stashSave } = useStashActions(repo.repoId);
  const { data: status } = useQuery({
    queryKey: queryKeys.repoStatus(repo.repoId),
    queryFn: () => getRepoStatus(repo.repoId),
  });
  const branch = status?.currentBranch ?? repo.currentBranch;
  const hasLocalChanges = ((status?.dirtyFiles ?? 0) + (status?.stagedFiles ?? 0)) > 0;
  const undo = useUndo(repo.repoId, useEmbed((s) => s.active));
  const toolsMenu = useContextMenu();
  const [remotesOpen, setRemotesOpen] = useState(false);
  const repoTools = useRepoToolsMenu(repo.repoId, repo.path);
  // Mesma consulta de MRs/PRs abertos do grafo (mesma chave): sem chamadas extras.
  const hosting = useHostingRemote(repo.repoId);
  const openMrByBranch = useOpenMergeRequests(repo.repoId, hosting);
  const openMr = branch ? openMrByBranch.get(branch) : undefined;
  const labels = hosting ? HOSTING_LABELS[hosting.provider] : null;
  const undoButton = "flex items-center gap-1.5 rounded bg-neutral-800 px-2.5 py-1.5 text-sm hover:bg-neutral-700 disabled:opacity-50";

  return (
    <div className="flex items-center gap-1.5 [&_button]:!py-1 [&_button]:text-[13px]">
      <button
        onClick={undo.undo}
        disabled={!undo.canUndo}
        title={undo.status?.undo ? t("gitUndo.undoTitle", { action: undo.status.undo, key: shortcutLabel("undo", overrides) }) : t("gitUndo.nothingToUndo")}
        className={undoButton}
      >
        <Undo2 size={14} /> {t("gitUndo.undo")}
      </button>
      <button
        onClick={undo.redo}
        disabled={!undo.canRedo}
        title={undo.status?.redo ? t("gitUndo.redoTitle", { action: undo.status.redo, key: shortcutLabel("redo", overrides) }) : t("gitUndo.nothingToRedo")}
        className={undoButton}
      >
        <Redo2 size={14} /> {t("gitUndo.redo")}
      </button>
      <span className="mx-0.5 h-4 w-px bg-neutral-800" />
      <RemoteOpsButtons repoId={repo.repoId} branch={branch} />
      {openMr && labels && (
        <button
          onClick={() => openUrl(openMr.webUrl)}
          title={t("gitRepo.openRequest", { request: labels.request, provider: labels.name, title: openMr.title })}
          className="flex items-center gap-1.5 rounded bg-neutral-800 px-3 py-1.5 text-sm hover:bg-neutral-700"
        >
          <GitMerge size={14} /> {`${labels.short} ${labels.refPrefix}${openMr.iid}`}
        </button>
      )}
      {/* One click: stash everything (untracked included). The message leads
          with the date/time so it's the first thing visible in the branches
          sidebar's Stashes list, where the newest stash sits on top. */}
      <button
        onClick={() =>
          stashSave.mutate(
            {
              message: `${formatDateTime(new Date())} — WIP on ${branch ?? "detached HEAD"}`,
              includeUntracked: true,
            },
            { onSuccess: () => toast.success(t("gitRepo.stashSaved")) },
          )
        }
        disabled={anyMutating || !hasLocalChanges}
        title={hasLocalChanges ? t("gitRepo.stashAll") : t("gitRepo.nothingToStash")}
        className="flex items-center gap-1.5 rounded bg-neutral-800 px-3 py-1.5 text-sm hover:bg-neutral-700 disabled:opacity-50"
      >
        {stashSave.isPending ? <Loader2 size={14} className="animate-spin" /> : <Package size={14} />} {t("gitRepo.stash")}
      </button>
      <button
        onClick={(e) =>
          toolsMenu.open(e, [...repoTools.items, { label: t("gitRepo.remotes"), onClick: () => setRemotesOpen(true) }])
        }
        title={t("gitRepo.repoToolsTitle")}
        className="shrink-0 rounded bg-neutral-800 px-2 py-1.5 hover:bg-neutral-700"
      >
        <Ellipsis size={14} />
      </button>
      {toolsMenu.menu}
      {repoTools.modals}
      {remotesOpen && <RemotesModal repoId={repo.repoId} onClose={() => setRemotesOpen(false)} />}
    </div>
  );
}
