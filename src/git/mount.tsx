import { QueryClientProvider, useQuery } from "@tanstack/react-query";
import { StrictMode, useEffect } from "react";
import { createRoot, type Root } from "react-dom/client";
import { Toaster } from "sonner";
import { ConfirmDialogHost } from "./components/ConfirmDialogHost";
import { HttpsCredentialsHost } from "./components/HttpsCredentialsHost";
import { SshPassphraseHost } from "./components/SshPassphraseHost";
import { isGitViewActive, openedRepos, portalRoot, useEmbed } from "./embed";
import { ActivityLogHost } from "./features/activity/ActivityLogHost";
import { initActivityLog, useActivityStore } from "./features/activity/store";
import { AiSettings } from "./features/ai/AiSettings";
import { EditorSettings } from "./features/editor/EditorSettings";
import { IntegrationsSettings } from "./features/integrations/IntegrationsSettings";
import { FetchSettings } from "./features/remote-ops/FetchSettings";
import { useAutoFetch } from "./features/remote-ops/useAutoFetch";
import { useShortcut } from "./features/shortcuts/useShortcuts";
import { SigningSettings } from "./features/signing/SigningSettings";
import { ProfilesSettings } from "./features/profiles/ProfilesSettings";
import { useEnsureDefaultProfile } from "./features/profiles/useEnsureDefaultProfile";
import { CreateBranchModal } from "./features/branches/CreateBranchModal";
import { CreateTagModal } from "./features/branches/CreateTagModal";
import { closeCreateRef, useCreateRef } from "./features/branches/createRefStore";
import { RepoSshKeyModal } from "./features/ssh-keys/RepoSshKeyModal";
import { SshKeysSettings } from "./features/ssh-keys/SshKeysSettings";
import { openRepository, type RepoSummary } from "./features/repository/api";
import { checkoutBranch, createBranch, listBranches } from "./features/branches/api";
import { RepoActions } from "./features/repository/RepoActions";
import { RepoView, type RepoViewMode } from "./features/repository/RepoView";
import { queryClient } from "./lib/query-client";
import { errorMessage } from "./lib/tauri-client";
import { useThemeMode } from "./lib/use-theme";
import { useT } from "../lib/i18n/react";
import "./App.css";

/*
 * Painéis Git (React) montados como ilhas dentro da casca Svelte: cada aba
 * Mudanças/Histórico chama `mountRepoPanel` no próprio elemento. O que é
 * global (toasts, diálogos, log de atividades, senha de chave SSH) fica numa
 * raiz única, criada no primeiro painel.
 */

const norm = (p: string) => p.replace(/[\\/]+$/, "");

/** Um repositório aberto no backend por caminho, compartilhado entre painéis
 * (a thread e a tela do repositório não abrem o mesmo checkout duas vezes). */
const repos = new Map<string, Promise<RepoSummary>>();

export function repoAt(path: string): Promise<RepoSummary> {
  const key = norm(path);
  let p = repos.get(key);
  if (!p) {
    p = openRepository(key);
    repos.set(key, p);
    p.then((r) => openedRepos.set(key, r)).catch(() => repos.delete(key));
  }
  return p;
}

function Hosts() {
  const themeMode = useThemeMode();
  const sshKeyFor = useEmbed((s) => s.sshKeyFor);
  const createRef = useCreateRef((s) => s.open);
  useEnsureDefaultProfile();
  // fetch periódico (em segundo plano) dos repositórios abertos pelos painéis
  useAutoFetch();
  useEffect(() => {
    initActivityLog();
    // Só os menus próprios do git (ex.: clique direito numa branch); na
    // conversa o menu nativo de copiar/colar continua.
    const onContextMenu = (e: MouseEvent) => {
      if (isGitViewActive()) e.preventDefault();
    };
    document.addEventListener("contextmenu", onContextMenu);
    return () => document.removeEventListener("contextmenu", onContextMenu);
  }, []);
  useShortcut("toggleActivityLog", (e) => {
    if (!isGitViewActive() || (e.target instanceof Element && e.target.closest(".xterm"))) return;
    e.preventDefault();
    useActivityStore.getState().toggle();
  });
  return (
    <div className="git-root">
      <ActivityLogHost />
      <ConfirmDialogHost />
      <SshPassphraseHost />
      <HttpsCredentialsHost />
      {createRef?.kind === "branch" && (
        <CreateBranchModal repoId={createRef.repoId} startSha={createRef.start} onClose={closeCreateRef} />
      )}
      {createRef?.kind === "tag" && (
        <CreateTagModal repoId={createRef.repoId} startSha={createRef.start} onClose={closeCreateRef} />
      )}
      {sshKeyFor && <RepoSshKeyModal repoPath={sshKeyFor} onClose={() => useEmbed.setState({ sshKeyFor: null })} />}
      <Toaster theme={themeMode} position="bottom-right" closeButton />
    </div>
  );
}

let hosts: Root | null = null;

function ensureHosts() {
  if (hosts) return;
  const el = document.createElement("div");
  portalRoot().appendChild(el);
  hosts = createRoot(el);
  hosts.render(
    <StrictMode>
      <QueryClientProvider client={queryClient}>
        <Hosts />
      </QueryClientProvider>
    </StrictMode>,
  );
}

export type RepoPanelProps = { path: string; mode: RepoViewMode; active: boolean };

function RepoPanel({ path, mode, active }: RepoPanelProps) {
  const t = useT();
  const { data: repo, error } = useQuery({
    queryKey: ["repoAt", norm(path)],
    queryFn: () => repoAt(path),
    staleTime: Infinity,
    retry: false,
  });
  if (error)
    return (
      <div className="flex h-full items-center justify-center p-6 text-sm text-red-400">
        {t("gitCommon.openRepoFailed", { path, error: errorMessage(error) })}
      </div>
    );
  if (!repo) return <div className="flex h-full items-center justify-center text-sm text-neutral-500">{t("gitCommon.openingRepo")}</div>;
  return <RepoView key={repo.repoId} repo={repo} mode={mode} active={active} />;
}

/** Monta um painel Git em `el`; `update` troca caminho/modo/visibilidade. */
export function mountRepoPanel(el: HTMLElement, props: RepoPanelProps) {
  ensureHosts();
  const root = createRoot(el);
  const render = (p: RepoPanelProps) =>
    root.render(
      <StrictMode>
        <QueryClientProvider client={queryClient}>
          <div className="git-root flex h-full w-full flex-col overflow-hidden bg-neutral-950 text-neutral-100">
            <RepoPanel {...p} />
          </div>
        </QueryClientProvider>
      </StrictMode>,
    );
  render(props);
  return { update: render, unmount: () => root.unmount() };
}

function RepoActionsFor({ path }: { path: string }) {
  const { data: repo } = useQuery({
    queryKey: ["repoAt", norm(path)],
    queryFn: () => repoAt(path),
    staleTime: Infinity,
    retry: false,
  });
  return repo ? <RepoActions repo={repo} /> : null;
}

/** Fetch/Push/Stash do repositório em `path`, na barra de abas. */
export function mountRepoActions(el: HTMLElement, path: string) {
  ensureHosts();
  const root = createRoot(el);
  const render = (p: string) =>
    root.render(
      <StrictMode>
        <QueryClientProvider client={queryClient}>
          <div className="git-root text-neutral-100">
            <RepoActionsFor path={p} />
          </div>
        </QueryClientProvider>
      </StrictMode>,
    );
  render(path);
  return { update: render, unmount: () => root.unmount() };
}

/* Troca e criação de branch pelo selo da branch no cabeçalho da thread.
 * Erros chegam como Error com a mensagem já legível. */

const readable = (e: unknown) => new Error(errorMessage(e));

/** Recarrega tudo o que os painéis git mostram deste repositório. */
const refreshRepo = (repoId: string) =>
  queryClient.invalidateQueries({ predicate: (q) => q.queryKey.includes(repoId) });

/** Branches locais do checkout em `path` (a atual vem com `isHead`). */
export async function gitLocalBranches(path: string) {
  try {
    const repo = await repoAt(path);
    return (await listBranches(repo.repoId)).filter((b) => !b.isRemote);
  } catch (e) {
    throw readable(e);
  }
}

export async function gitCheckout(path: string, name: string) {
  try {
    const repo = await repoAt(path);
    await checkoutBranch(repo.repoId, name);
    refreshRepo(repo.repoId);
  } catch (e) {
    throw readable(e);
  }
}

/** Cria `name` a partir da branch atual e faz checkout nela. */
export async function gitCreateBranch(path: string, name: string) {
  try {
    const repo = await repoAt(path);
    await createBranch(repo.repoId, name, null, true);
    refreshRepo(repo.repoId);
  } catch (e) {
    throw readable(e);
  }
}

/** Abre a chave SSH do repositório em `path` (botão na barra de abas). */
export function openSshKey(path: string) {
  ensureHosts();
  useEmbed.setState({ sshKeyFor: norm(path) });
}

/** Mostra/esconde o log de atividades do git (botão na barra de abas). */
export function toggleActivityLog() {
  ensureHosts();
  useActivityStore.getState().toggle();
}

/** O Histórico de `repoPath` abre com a linha de mudanças locais (staging). */
export function focusLocalChanges(repoPath: string) {
  useEmbed.setState({ focusWip: repoPath });
}

/** A casca Svelte informa como trocar de aba (ex.: Árvore → Histórico). */
export function setGitNavigator(navigate: (tab: "history") => void) {
  useEmbed.setState({ navigate });
}

/** Liga os atalhos/menus do git enquanto algum painel está visível. */
export function setGitActive(active: boolean) {
  useEmbed.setState({ active });
}

export type GitSettingsSection = "git" | "integrations";

/** Monta uma seção de configurações do git (aplicadas na hora) em `el`,
 * dentro da janela de Configurações do app. */
export function mountGitSettings(el: HTMLElement, section: GitSettingsSection) {
  ensureHosts();
  const root = createRoot(el);
  root.render(
    <StrictMode>
      <QueryClientProvider client={queryClient}>
        <div className="git-root space-y-8 text-sm text-neutral-100">
          {section === "git" ? (
            <>
              <ProfilesSettings />
              <SshKeysSettings />
              <FetchSettings />
              <SigningSettings />
              <EditorSettings />
              <AiSettings />
            </>
          ) : (
            <IntegrationsSettings />
          )}
        </div>
      </QueryClientProvider>
    </StrictMode>,
  );
  return { unmount: () => root.unmount() };
}
