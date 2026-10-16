<script lang="ts">
  import { app, isGitTab, type MainTab } from '../stores/app.svelte'
  import Icon from './Icon.svelte'
  import RepoActions from './RepoActions.svelte'
  import { t } from '../i18n/index.svelte'
  import { keyHint } from '../shortcuts/store.svelte'

  // Abas da thread e da tela do repositório (Ctrl+1…6), com as ações do
  // repositório (fetch, push, stash, chave SSH, log de atividades) à direita.
  // `git`: sem `.git` só há Conversa e Terminal, e nenhuma ação de repositório
  let { chatLabel, repoPath, git = true }: { chatLabel?: string; repoPath: string; git?: boolean } = $props()

  type Tab = { id: MainTab; icon: string; label: string }
  const TABS: Tab[] = $derived(([
    { id: 'chat', icon: 'sparkle', label: chatLabel ?? t('mainTabs.chat') },
    { id: 'history', icon: 'branch', label: t('mainTabs.history') },
    { id: 'tree', icon: 'tree', label: t('mainTabs.tree') },
    { id: 'reflog', icon: 'history', label: t('mainTabs.reflog') },
    { id: 'stats', icon: 'chart', label: t('mainTabs.stats') },
    { id: 'terminal', icon: 'terminal', label: t('mainTabs.terminal') },
  ] as Tab[]).filter((tab) => git || !isGitTab(tab.id)))

  async function sshKey() {
    const { openSshKey } = await import('../../git/mount')
    openSshKey(repoPath)
  }

  async function activityLog() {
    const { toggleActivityLog } = await import('../../git/mount')
    toggleActivityLog()
  }
</script>

<nav class="tabs">
  {#each TABS as tab, i (tab.id)}
    <button class:on={app.tab === tab.id} title={t('mainTabs.tabTitle', { label: tab.label, n: i + 1 })} onclick={() => (app.tab = tab.id)}>
      <Icon name={tab.icon} size={13} /> <span class="lbl">{tab.label}</span>
    </button>
  {/each}
  <span class="sp"></span>
  {#if git}
    <RepoActions path={repoPath} />
    <button class="act" title={t('mainTabs.sshKey')} onclick={sshKey}><Icon name="key" size={14} /></button>
    <button class="act" title={t('mainTabs.activityLog', { key: keyHint('toggleActivityLog') })} onclick={activityLog}><Icon name="list" size={14} /></button>
  {/if}
</nav>

<style>
  .tabs {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 0 8px 0 12px;
    border-bottom: 1px solid var(--border);
    flex: none;
    overflow-x: auto;
    scrollbar-width: none;
    container-type: inline-size;
  }
  .tabs button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: none;
    border: none;
    padding: 8px 10px 7px;
    color: var(--text-muted);
    border-bottom: 2px solid transparent;
    margin-bottom: -1px;
    white-space: nowrap;
    flex: none;
  }
  .tabs button:hover {
    color: var(--text);
  }
  .tabs button.on {
    color: var(--text);
    border-bottom-color: var(--accent);
  }
  .sp {
    flex: 1;
  }
  .tabs .act {
    padding: 6px;
    border-radius: var(--radius-sm);
    border-bottom: none;
    margin: 0;
  }
  .tabs .act:hover {
    background: var(--bg-hover);
  }
  /* sem espaço para abas + ações, as abas mostram só o ícone (o nome fica na
     dica); os rótulos de desfazer/refazer continuam */
  @container (max-width: 1240px) {
    .lbl {
      display: none;
    }
  }
</style>
