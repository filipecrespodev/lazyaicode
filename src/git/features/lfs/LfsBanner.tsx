import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { AlertTriangle, Loader2, X } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { queryKeys } from "@/lib/query-keys";
import { errorMessage } from "@/lib/tauri-client";
import { getLfsStatus, runLfsPull } from "./api";
import { useT } from "../../../lib/i18n/react";

/** Dismissed for the rest of the session, by repo path (ids change on reopen). */
const dismissed = new Set<string>();

/** Non-blocking notice for repos that use Git LFS, which libgit2 can't smudge. */
export function LfsBanner({ repoId, repoPath }: { repoId: string; repoPath: string }) {
  const t = useT();
  const queryClient = useQueryClient();
  const [hidden, setHidden] = useState(() => dismissed.has(repoPath));
  const { data } = useQuery({
    queryKey: ["lfsStatus", repoId],
    queryFn: () => getLfsStatus(repoId),
    // Read once per repo open: .gitattributes rarely changes under us.
    staleTime: Infinity,
  });
  const pull = useMutation({
    mutationFn: () => runLfsPull(repoId),
    onSuccess: () => {
      toast.success(t("gitLfs.pullDone"));
      queryClient.invalidateQueries({ queryKey: queryKeys.workingStatus(repoId) });
    },
    onError: (e) => toast.error(errorMessage(e)),
  });

  if (!data?.usesLfs || hidden) return null;

  return (
    <div className="flex shrink-0 items-center gap-2 border-b border-amber-900/60 bg-amber-950/40 px-4 py-1.5 text-xs text-amber-100">
      <AlertTriangle size={13} className="shrink-0 text-amber-400" />
      <span className="min-w-0 flex-1">
        {data.installed ? t("gitLfs.bannerInstalled") : t("gitLfs.bannerNotInstalled")}
      </span>
      {data.installed && (
        <button
          onClick={() => pull.mutate()}
          disabled={pull.isPending}
          className="flex shrink-0 items-center gap-1 rounded bg-amber-800 px-2 py-0.5 text-amber-50 hover:bg-amber-700 disabled:opacity-50"
        >
          {pull.isPending && <Loader2 size={11} className="animate-spin" />} {t("gitLfs.runPull")}
        </button>
      )}
      <button
        onClick={() => {
          dismissed.add(repoPath);
          setHidden(true);
        }}
        title={t("gitLfs.hide")}
        className="shrink-0 rounded p-0.5 text-amber-300 hover:bg-amber-900"
      >
        <X size={13} />
      </button>
    </div>
  );
}
