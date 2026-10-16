import type { ReactNode } from "react";
import { createPortal } from "react-dom";
import { create } from "zustand";
import { openPath } from "@tauri-apps/plugin-opener";
import type { CommitNode } from "./features/commit-graph/types";
import type { RepoSummary } from "./features/repository/api";

/**
 * Estado dos painéis Git (React) montados dentro da casca Svelte. `active`
 * fica ligado enquanto uma aba Mudanças ou Histórico está visível: atalhos
 * globais do git (Ctrl+Z, Ctrl+L…) e o menu de contexto só agem nesse caso.
 * `repoPath` é o repositório desse painel visível.
 */
type EmbedState = {
  active: boolean;
  repoPath: string | null;
  /** Commit que o Histórico deve abrir (clique duplo na Árvore). */
  focusCommit: { repoPath: string; commit: CommitNode } | null;
  /** Repositório cujo Histórico deve selecionar a linha de mudanças locais. */
  focusWip: string | null;
  /** Repositório cuja chave SSH está sendo editada (modal). */
  sshKeyFor: string | null;
  /** Troca a aba da thread/repositório; registrado pela casca Svelte. */
  navigate: (tab: "history") => void;
};

export const useEmbed = create<EmbedState>(() => ({
  active: false,
  repoPath: null,
  focusCommit: null,
  focusWip: null,
  sshKeyFor: null,
  navigate: () => {},
}));

export const isGitViewActive = () => useEmbed.getState().active;

const normPath = (p: string) => p.replace(/[\\/]+$/, "");

/** Repositórios já abertos no backend pelos painéis (por caminho): o fetch
 * periódico passa por eles, e remover um worktree avisa se ele está aberto. */
export const openedRepos = new Map<string, RepoSummary>();

export const isRepoOpen = (path: string) => openedRepos.has(normPath(path));

/** Abre a pasta (worktree, submódulo) no gerenciador de arquivos. */
export const openFolder = (path: string) => openPath(path).catch(() => {});

let portal: HTMLElement | null = null;

/** Destino dos portais (modais, menus, popovers): fora de `.git-root`, mas
 * com o preflight do Tailwind restrito via `.git-portal`. */
export function portalRoot(): HTMLElement {
  if (!portal || !portal.isConnected) {
    portal = document.createElement("div");
    portal.className = "git-portal";
    document.body.appendChild(portal);
  }
  return portal;
}

/** Telas como Árvore/Reflog/Estatísticas: sobrepostas (portal) quando têm
 * `onClose`, ou embutidas na aba quando não têm. */
export const overlayOrInline = (node: ReactNode, overlay: boolean) =>
  overlay ? createPortal(node, portalRoot()) : node;

export const overlayClass = (overlay: boolean) => (overlay ? "fixed inset-0 z-[150]" : "h-full");
