<script lang="ts">
  import { app } from '../stores/app.svelte'
  import type { Project } from '../api'
  import { relTime } from '../markdown'
  import Icon from './Icon.svelte'
  import MainTabs from './MainTabs.svelte'
  import GitPanel from './GitPanel.svelte'
  import { t } from '../i18n/index.svelte'

  // Tela do repositório: o checkout principal da pasta, sem thread.
  let { project }: { project: Project } = $props()

  const GIT_VIEWS = ['history', 'tree', 'reflog', 'stats'] as const

  const threads = $derived(
    app.threads
      .filter((th) => th.project_id === project.id && !th.archived)
      .sort((a, b) => b.updated_at - a.updated_at),
  )
</script>

<section class="repo" class:collapsed={app.tab === 'terminal'}>
  <header class="head">
    <div class="crumbs">
      <div class="title"><Icon name="folder" size={14} /> {project.name}</div>
      <div class="path mono" title={project.path}>{project.path}</div>
    </div>
    <button class="btn" onclick={() => app.newThread(project.id)}><Icon name="plus" /> {t('repoScreen.newThread')}</button>
  </header>

  <MainTabs chatLabel={t('repoScreen.threads')} repoPath={project.path} git={project.is_git} />

  <div class="pane" class:hidden={app.tab !== 'chat'}>
    <div class="list">
      {#each threads as t (t.id)}
        <button class="row" onclick={() => app.select(t.id)}>
          {#if t.status === 'running'}<span class="spinner"></span>
          {:else}<span class="dot {t.status !== 'idle' ? t.status : t.unread ? 'unread' : ''}"></span>{/if}
          <span class="rt">{t.title}</span>
          {#if t.branch}<span class="rb mono"><Icon name="branch" size={11} /> {t.branch}</span>{/if}
          <span class="rp">{relTime(t.updated_at)}</span>
        </button>
      {:else}
        <div class="empty">{t('repoScreen.empty')}</div>
      {/each}
    </div>
  </div>
  {#if project.is_git}
    {#each GIT_VIEWS as m (m)}
      <div class="pane" class:hidden={app.tab !== m}>
        <GitPanel path={project.path} mode={m} active={app.tab === m} />
      </div>
    {/each}
  {/if}
</section>

<style>
  .repo {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
    min-width: 0;
  }
  .repo.collapsed {
    flex: none;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 12px 8px 18px;
    border-bottom: 1px solid var(--border);
    min-height: 54px;
  }
  .crumbs {
    flex: 1;
    min-width: 0;
  }
  .title {
    display: flex;
    align-items: center;
    gap: 6px;
    font-weight: 600;
    font-size: 14px;
  }
  .path {
    font-size: 11.5px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .pane {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .pane.hidden {
    display: none;
  }
  .list {
    overflow-y: auto;
    padding: 12px;
    max-width: 780px;
    width: 100%;
    margin: 0 auto;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    background: none;
    border: none;
    border-radius: var(--radius-sm);
    padding: 7px 10px;
    text-align: left;
    color: var(--text);
  }
  .row:hover {
    background: var(--bg-hover);
  }
  .rt {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .rb,
  .rp {
    font-size: 11.5px;
    color: var(--text-faint);
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .empty {
    color: var(--text-muted);
    text-align: center;
    padding: 40px 10px;
  }
</style>
