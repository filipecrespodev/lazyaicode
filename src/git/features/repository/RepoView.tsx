import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ArrowDown,
  ArrowUp,
  CircleX,
  PanelLeftClose,
  PanelLeftOpen,
  PanelRightClose,
  PanelRightOpen,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import { ResizableSidebar } from "@/components/ResizableSidebar";
import { BranchSidebar } from "@/features/branches/BranchSidebar";
import { lazyNamed } from "@/lib/lazy";
import { getHeadCommit } from "@/features/commit-graph/api";
import { CommitGraph } from "@/features/commit-graph/CommitGraph";
import type { GraphSelection } from "@/features/commit-graph/types";
import { CommitDetailsPanel } from "@/features/diff-viewer/CommitDetailsPanel";
import { MultiCommitDetailsPanel } from "@/features/diff-viewer/MultiCommitDetailsPanel";
import type { FileDiff } from "@/features/diff-viewer/types";
import { openBlame, openFileHistory } from "@/features/history/store";
import { LfsBanner } from "@/features/lfs/LfsBanner";
import { getRebaseStatus } from "@/features/rebase/api";
import { RebaseConflictPanel } from "@/features/rebase/RebaseConflictPanel";
import { useResolveConflict } from "@/features/rebase/useResolveConflict";
import { useRebaseStore } from "@/features/rebase/store";
import { fetchRemote } from "@/features/remote-ops/api";
import { refreshAfterBackgroundFetch } from "@/features/remote-ops/useAutoFetch";
import { StashDetailsPanel } from "@/features/stash/StashDetailsPanel";

// Telas pesadas carregam no primeiro uso: a pilha de diff/sintaxe e as telas
// de estatísticas/árvore ficam fora do bundle inicial.
const CommitTreeView = lazyNamed(() => import("@/features/commit-tree/CommitTreeView"), "CommitTreeView");
const ReflogView = lazyNamed(() => import("@/features/reflog/ReflogView"), "ReflogView");
const StatisticsView = lazyNamed(() => import("@/features/statistics/StatisticsView"), "StatisticsView");
const HistoryViewHost = lazyNamed(() => import("@/features/history/HistoryViewHost"), "HistoryViewHost");
const FileDiffFullView = lazyNamed(() => import("@/features/diff-viewer/FileDiffFullView"), "FileDiffFullView");
const ConflictResolutionView = lazyNamed(
  () => import("@/features/rebase/ConflictResolutionView"),
  "ConflictResolutionView",
);
import { StagingPanel } from "@/features/working/StagingPanel";
import type { WorkingSide } from "@/features/working/types";
import { confirm } from "@/lib/confirm-store";
import { queryKeys } from "@/lib/query-keys";
import { usePersistedValue } from "@/lib/use-persisted-config";
import type { RepoStatus, RepoSummary } from "./api";
import { getRepoStatus } from "./api";
import { useRepoPolling } from "./useRepoPolling";
import { useEmbed } from "@/embed";
import { useT } from "../../../lib/i18n/react";
import { shortcutLabel } from "../../../lib/shortcuts/shortcuts";
import { getOverrides } from "../../../lib/shortcuts/store.svelte";

const IN_PROGRESS_LABELS: Record<NonNullable<RepoStatus["inProgressOp"]>, string> = {
  Rebase: "gitRepo.opRebase",
  Merge: "gitRepo.opMerge",
  CherryPick: "gitRepo.opCherryPick",
  Revert: "gitRepo.opRevert",
};

export type RepoViewMode = "changes" | "history" | "tree" | "reflog" | "stats";

/** Painel Git de um repositório: `changes` mostra staging + diff do arquivo,
 * `history` o grafo com branches/tags/stash e os detalhes do que for
 * selecionado. `active` = painel visível (habilita Ctrl+Z e afins). */
/** Repositórios que já tiveram o fetch automático nesta sessão. Cada aba
 * Git monta o próprio RepoView (e trocar de thread remonta), mas o fetch é
 * de rede e roda na fila única do repositório no backend: repeti-lo a cada
 * montagem segurava status/diff/stage atrás dele. */
const autoFetched = new Set<string>();

export function RepoView({ repo, mode, active }: { repo: RepoSummary; mode: RepoViewMode; active: boolean }) {
  const t = useT();
  const rebaseStatus = useRebaseStore((s) => s.status);
  const setRebaseStatus = useRebaseStore((s) => s.setStatus);
  const queryClient = useQueryClient();
  const [selected, setSelected] = useState<GraphSelection>(null);
  const [openFile, setOpenFile] = useState<FileDiff | null>(null);
  // Só faz sentido para arquivos da cópia de trabalho (definido pelo staging).
  const [openFileSide, setOpenFileSide] = useState<WorkingSide | null>(null);
  const [branchesOpen, setBranchesOpen] = usePersistedValue("sidebar.branchesOpen", true);
  // Right-hand details panel can be tucked away (e.g. to read a diff at full
  // width); picking something new in the graph brings it back.
  const [detailsOpen, setDetailsOpen] = useState(true);
  const [openConflictPath, setOpenConflictPath] = useState<string | null>(null);
  const showRebaseConflicts = rebaseStatus?.state === "Conflicts";

  useEffect(() => {
    if (!showRebaseConflicts) setOpenConflictPath(null);
  }, [showRebaseConflicts]);

  const { data: status } = useQuery({
    queryKey: queryKeys.repoStatus(repo.repoId),
    queryFn: () => getRepoStatus(repo.repoId),
  });
  const { data: headCommit } = useQuery({
    queryKey: queryKeys.headCommit(repo.repoId),
    queryFn: () => getHeadCommit(repo.repoId),
  });

  useEffect(() => {
    // Sync with the backend's real rebase state instead of trusting only past
    // mutation results — covers a lost/stale in-memory session on either side.
    getRebaseStatus(repo.repoId).then(setRebaseStatus);
  }, [repo, setRebaseStatus]);

  useEffect(() => {
    // Melhor esforço, uma vez por repositório na sessão: fetch de todos os
    // remotes em segundo plano (nunca pede senha/token; sem remote, sem rede
    // ou chave bloqueada falham em silêncio). Repositórios já em dia (o caso
    // comum) não recarregam o log que acabaram de carregar.
    if (autoFetched.has(repo.repoId)) return;
    autoFetched.add(repo.repoId);
    fetchRemote(repo.repoId, null, { background: true })
      .catch(() => null)
      .then((summary) => refreshAfterBackgroundFetch(queryClient, repo.repoId, summary));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [repo.repoId]);

  useRepoPolling(repo.repoId, active);

  // Auto-select local changes on open, once per repo — but never fight a
  // deliberate deselect, so this only fires the first time status loads.
  const autoSelectedRepoId = useRef<string | null>(null);
  useEffect(() => {
    if (status === undefined || mode === "history") return;
    if (autoSelectedRepoId.current === repo.repoId) return;
    autoSelectedRepoId.current = repo.repoId;
    const hasChanges = (status.dirtyFiles + status.stagedFiles) > 0;
    if (hasChanges) setSelected({ kind: "wip" });
  }, [repo, status]);

  useEffect(() => {
    if (active) useEmbed.setState({ repoPath: repo.path });
  }, [active, repo.path]);

  // clique duplo num nó da Árvore: o Histórico abre aquele commit
  const focusCommit = useEmbed((s) => s.focusCommit);
  useEffect(() => {
    if (mode !== "history" || !focusCommit || focusCommit.repoPath !== repo.path) return;
    setSelected({ kind: "commit", commit: focusCommit.commit });
    setOpenFile(null);
    setDetailsOpen(true);
    useEmbed.setState({ focusCommit: null });
  }, [mode, focusCommit, repo.path]);

  // atalho/rodapé "mudanças": o Histórico seleciona a linha de mudanças locais
  const focusWip = useEmbed((s) => s.focusWip);
  useEffect(() => {
    if (mode !== "history" || focusWip !== repo.path) return;
    setSelected({ kind: "wip" });
    setOpenFile(null);
    useEmbed.setState({ focusWip: null });
  }, [mode, focusWip, repo.path]);


  const { abort: abortOperation } = useResolveConflict(repo.repoId);
  const abortInProgress = async () => {
    const op = status?.inProgressOp;
    if (!op) return;
    const name = t(IN_PROGRESS_LABELS[op]).toLowerCase();
    const confirmed = await confirm(t("gitRepo.abortConfirm", { op: name, key: shortcutLabel("undo", getOverrides()) }), {
      title: t("gitRepo.abortTitle", { op: name }),
      kind: "warning",
    });
    if (!confirmed) return;
    await abortOperation();
    toast.success(t("gitRepo.aborted", { op: t(IN_PROGRESS_LABELS[op]) }));
  };

  const hasLocalChanges = ((status?.dirtyFiles ?? 0) + (status?.stagedFiles ?? 0)) > 0;
  const openFileSha =
    mode === "changes"
      ? null
      : selected?.kind === "commit"
      ? selected.commit.sha
      : selected?.kind === "commits"
        ? selected.toSha
        : selected?.kind === "stash"
          ? selected.stash.sha
          : null;
  const changesMode = mode === "changes";
  // no Histórico o staging só aparece ao clicar na linha de mudanças locais
  const showStaging = changesMode || selected?.kind === "wip";
  const showCommitDetails = !changesMode && selected?.kind === "commit";
  const showMultiCommitDetails = !changesMode && selected?.kind === "commits";
  const showStashDetails = !changesMode && selected?.kind === "stash";
  const hasDetailsPanel =
    showRebaseConflicts || showStaging || showCommitDetails || showMultiCommitDetails || showStashDetails;

  if (mode === "tree")
    return (
      <CommitTreeView
        repoId={repo.repoId}
        onOpenCommit={(commit) => {
          useEmbed.setState({ focusCommit: { repoPath: repo.path, commit } });
          useEmbed.getState().navigate("history");
        }}
      />
    );
  if (mode === "stats") return <StatisticsView repoId={repo.repoId} />;
  if (mode === "reflog")
    return (
      <ReflogView
        repoId={repo.repoId}
        currentBranch={status?.currentBranch ?? null}
        hasLocalChanges={hasLocalChanges}
      />
    );

  return (
    <main className="flex h-full w-full flex-col overflow-hidden bg-neutral-950 text-neutral-100">
      <LfsBanner repoId={repo.repoId} repoPath={repo.path} />
      <HistoryViewHost
        repoId={repo.repoId}
        active={active}
        onSelectCommit={(commit) => {
          // na aba Mudanças não há grafo: o commit abre no Histórico
          if (changesMode) {
            useEmbed.setState({ focusCommit: { repoPath: repo.path, commit } });
            useEmbed.getState().navigate("history");
            return;
          }
          setSelected({ kind: "commit", commit });
          setOpenFile(null);
          setDetailsOpen(true);
        }}
      />
      {/* a branch e a pasta já estão no cabeçalho da thread; aqui ficam os
          botões dos painéis, à frente/atrás e o aviso de operação em andamento */}
      {!changesMode && <div className="flex items-center justify-between px-4 py-1.5">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            {!changesMode && <button
              onClick={() => setBranchesOpen(!branchesOpen)}
              title={branchesOpen ? t("gitRepo.hideBranches") : t("gitRepo.showBranches")}
              className="shrink-0 rounded p-1 text-neutral-400 hover:bg-neutral-800 hover:text-neutral-100"
            >
              {branchesOpen ? <PanelLeftClose size={16} /> : <PanelLeftOpen size={16} />}
            </button>}
            {status && (status.ahead > 0 || status.behind > 0) && (
              <span className="flex shrink-0 items-center gap-1.5 text-xs text-neutral-400" title={t("gitRepo.aheadBehind")}>
                <span className="inline-flex items-center">
                  <ArrowUp size={12} />
                  {status.ahead}
                </span>
                <span className="inline-flex items-center">
                  <ArrowDown size={12} />
                  {status.behind}
                </span>
              </span>
            )}
            {status?.inProgressOp && (
              <span className="flex shrink-0 items-center gap-1.5 rounded bg-amber-900/60 py-0.5 pr-0.5 pl-1.5 text-xs text-amber-200">
                {t("gitRepo.opInProgress", { op: t(IN_PROGRESS_LABELS[status.inProgressOp]) })}
                <button
                  onClick={abortInProgress}
                  title={t("gitRepo.abortOpTitle", { op: t(IN_PROGRESS_LABELS[status.inProgressOp]).toLowerCase() })}
                  className="flex items-center gap-1 rounded bg-red-900 px-1.5 py-px text-red-100 hover:bg-red-800"
                >
                  <CircleX size={11} /> {t("gitRebase.abort")}
                </button>
              </span>
            )}
          </div>
        </div>
        {hasDetailsPanel && !changesMode && (
          <button
            onClick={() => setDetailsOpen(!detailsOpen)}
            title={detailsOpen ? t("gitRepo.hideDetails") : t("gitRepo.showDetails")}
            className="shrink-0 rounded p-1 text-neutral-400 hover:bg-neutral-800 hover:text-neutral-100"
          >
            {detailsOpen ? <PanelRightClose size={16} /> : <PanelRightOpen size={16} />}
          </button>
        )}
      </div>}

      <div className={`flex min-h-0 min-w-0 flex-1 gap-2 overflow-hidden px-4 pb-4 ${changesMode ? "pt-4" : ""}`}>
        {branchesOpen && !changesMode && (
          <ResizableSidebar
            side="left"
            configKey="sidebar.branchesWidth"
            defaultWidth={240}
            minWidth={160}
            maxWidth={480}
          >
            <BranchSidebar
              repoId={repo.repoId}
              currentBranch={status?.currentBranch ?? null}
              selectedStashSha={selected?.kind === "stash" ? selected.stash.sha : null}
              onSelectStash={(stash) => {
                setSelected({ kind: "stash", stash });
                setOpenFile(null);
                setDetailsOpen(true);
              }}
            />
          </ResizableSidebar>
        )}
        <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden rounded border border-neutral-800">
          <div className="min-h-0 flex-1 overflow-hidden">
            {openConflictPath && showRebaseConflicts ? (
              (() => {
                const conflict = rebaseStatus!.conflicts.find((c) => c.path === openConflictPath);
                return conflict ? (
                  <ConflictResolutionView
                    key={conflict.path}
                    repoId={repo.repoId}
                    file={conflict}
                    onResolved={() => setOpenConflictPath(null)}
                    onClose={() => setOpenConflictPath(null)}
                  />
                ) : null;
              })()
            ) : openFile ? (
              <FileDiffFullView
                repoId={repo.repoId}
                sha={openFileSha}
                file={openFile}
                workingSide={openFileSha === null && showStaging ? openFileSide : null}
                onBack={() => setOpenFile(null)}
                actions={
                  <>
                    <button
                      onClick={() => openFileHistory(repo.repoId, openFile.path, openFileSha)}
                      className="rounded bg-neutral-800 px-2 py-1 text-xs hover:bg-neutral-700"
                    >
                      {t("gitRepo.history")}
                    </button>
                    {openFile.status !== "Deleted" && (
                      <button
                        onClick={() => openBlame(repo.repoId, openFile.path, openFileSha)}
                        title={openFileSha ? t("gitRepo.blameAtCommit") : t("gitRepo.blameAtHead")}
                        className="rounded bg-neutral-800 px-2 py-1 text-xs hover:bg-neutral-700"
                      >
                        {t("gitRepo.blame")}
                      </button>
                    )}
                  </>
                }
              />
            ) : changesMode ? (
              <div className="flex h-full items-center justify-center p-6 text-center text-sm text-neutral-500">
                {hasLocalChanges
                  ? t("gitRepo.selectFile")
                  : t("gitRepo.noChanges")}
              </div>
            ) : (
              <CommitGraph
                repoId={repo.repoId}
                selection={selected}
                currentBranch={status?.currentBranch ?? null}
                hasLocalChanges={hasLocalChanges}
                onSelectionChange={(next) => {
                  setSelected(next);
                  setDetailsOpen(true);
                  setOpenFile(null);
                }}
              />
            )}
          </div>
        </div>
        {!detailsOpen && !changesMode ? null : showRebaseConflicts ? (
          <ResizableSidebar configKey="sidebar.detailsWidth">
            <RebaseConflictPanel
              repoId={repo.repoId}
              onOpenConflict={setOpenConflictPath}
              openConflictPath={openConflictPath}
            />
          </ResizableSidebar>
        ) : (showStaging || showCommitDetails || showMultiCommitDetails || showStashDetails) && (
          <ResizableSidebar configKey="sidebar.detailsWidth">
            {showCommitDetails && selected?.kind === "commit" ? (
              <CommitDetailsPanel
                repoId={repo.repoId}
                commit={selected.commit}
                selectedFilePath={openFile?.path ?? null}
                onOpenFile={setOpenFile}
                onAmended={(updated) => setSelected({ kind: "commit", commit: updated })}
              />
            ) : showMultiCommitDetails && selected?.kind === "commits" ? (
              <MultiCommitDetailsPanel
                repoId={repo.repoId}
                commits={selected.commits}
                fromSha={selected.fromSha}
                toSha={selected.toSha}
                selectedFilePath={openFile?.path ?? null}
                onOpenFile={setOpenFile}
              />
            ) : showStashDetails && selected?.kind === "stash" ? (
              <StashDetailsPanel
                repoId={repo.repoId}
                stash={selected.stash}
                selectedFilePath={openFile?.path ?? null}
                onOpenFile={setOpenFile}
              />
            ) : (
              <StagingPanel
                repoId={repo.repoId}
                selectedFilePath={openFile?.path ?? null}
                selectedSide={openFileSide}
                onOpenFile={(file, side) => {
                  setOpenFile(file);
                  setOpenFileSide(side ?? null);
                }}
                headCommit={headCommit ?? null}
              />
            )}
          </ResizableSidebar>
        )}
      </div>
    </main>
  );
}
