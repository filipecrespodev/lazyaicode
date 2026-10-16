<script lang="ts">
  import { app } from '../stores/app.svelte'
  import Icon from './Icon.svelte'
  import WindowControls from './WindowControls.svelte'
  import Logo from './Logo.svelte'
  import { t } from '../i18n/index.svelte'
  import { keyHint } from '../shortcuts/store.svelte'

  let menu = $state<{ x: number; y: number; id: string } | null>(null)
  /** menu do "+": abrir um workspace fechado, escolher vários ou criar um */
  let addMenu = $state<{ x: number; y: number } | null>(null)

  function openAddMenu(e: MouseEvent) {
    e.stopPropagation()
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect()
    addMenu = addMenu ? null : { x: r.left, y: r.bottom + 4 }
    menu = null
  }

  function authState(dir: string | null) {
    const a = app.auth[dir ?? '']
    if (!a) return 'unknown'
    return a.loggedIn ? 'ok' : 'bad'
  }

  function onContext(e: MouseEvent, id: string) {
    e.preventDefault()
    menu = { x: e.clientX, y: e.clientY, id }
  }
</script>

<svelte:window onclick={() => { menu = null; addMenu = null }} />

<!-- Faz as vezes da barra de título: as áreas vazias arrastam a janela e o
     duplo clique maximiza (data-tauri-drag-region). -->
<header class="bar" data-tauri-drag-region>
  <button class="icon" class:on={app.sidebarOpen} title={t(app.sidebarOpen ? 'titlebar.hideSidebar' : 'titlebar.showSidebar', { key: keyHint('toggleSidebar') })} onclick={() => app.toggleSidebar()}>
    <Icon name="sidebar" size={15} />
  </button>
  <div class="brand" data-tauri-drag-region><Logo size={18} /> LazyAICode</div>

  <nav class="tabs">
    {#each app.openWorkspaces as w (w.id)}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <div
        class="tab"
        class:on={w.id === app.activeWorkspaceId && !app.pickingWorkspaces}
        role="tab"
        tabindex="0"
        aria-selected={w.id === app.activeWorkspaceId}
        onclick={() => (w.id !== app.activeWorkspaceId || app.pickingWorkspaces) && app.switchWorkspace(w.id)}
        onauxclick={(e) => e.button === 1 && app.closeWorkspace(w.id)}
        ondblclick={() => app.openSettings('workspaces', w.id)}
        oncontextmenu={(e) => onContext(e, w.id)}
        title={t('titlebar.accountTitle', { path: w.path, account: app.accountLabel(w.claude_dir) })}
      >
        <span class="acc {authState(w.claude_dir)}"></span>
        <span class="name">{w.name}</span>
        <span class="acct">{app.accountLabel(w.claude_dir).replace('~/.', '')}</span>
        <button class="close" title={t('titlebar.closeWorkspaceHint')}
          onclick={(e) => { e.stopPropagation(); app.closeWorkspace(w.id) }}><Icon name="x" size={11} /></button>
      </div>
    {/each}
    <button class="add" class:on={!!addMenu} title={t('titlebar.openWorkspace')} onclick={openAddMenu}><Icon name="plus" size={14} /></button>
  </nav>

  <div class="drag" data-tauri-drag-region></div>

  <button class="icon" title={t('titlebar.commandPalette', { key: keyHint('commandPalette') })} onclick={() => (app.paletteOpen = true)}><Icon name="search" size={15} /></button>
  <button class="icon" title={t('titlebar.settings', { key: keyHint('openSettings') })} onclick={() => app.openSettings()}><Icon name="settings" size={15} /></button>
  <WindowControls />
</header>

{#if menu}
  {@const id = menu.id}
  <div class="ctx" style="left:{menu.x}px; top:{menu.y}px">
    <button onclick={() => { menu = null; app.openSettings('workspaces', id) }}>{t('titlebar.editWorkspace')}</button>
    <button onclick={() => { menu = null; app.closeWorkspace(id) }}>{t('titlebar.closeWorkspace')}</button>
    <hr />
    <button onclick={() => { menu = null; app.newWorkspaceOpen = true }}>{t('titlebar.newWorkspace')}</button>
  </div>
{/if}

{#if addMenu}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="ctx" style="left:{addMenu.x}px; top:{addMenu.y}px" onclick={(e) => e.stopPropagation()}>
    {#if app.closedWorkspaces.length}
      <div class="ctx-title">{t('titlebar.openWorkspace')}</div>
      {#each app.closedWorkspaces as w (w.id)}
        <button title={w.path} onclick={() => { addMenu = null; app.switchWorkspace(w.id) }}>
          <span class="acc {authState(w.claude_dir)}"></span> {w.name}
        </button>
      {/each}
      <hr />
    {/if}
    {#if app.workspaces.length > 1}
      <button onclick={() => { addMenu = null; app.pickingWorkspaces = true }}>{t('titlebar.chooseWorkspaces')}</button>
    {/if}
    <button onclick={() => { addMenu = null; app.newWorkspaceOpen = true }}>{t('titlebar.newWorkspace')}</button>
  </div>
{/if}

<style>
  .bar {
    height: 38px;
    display: flex;
    align-items: center;
    gap: 4px;
    padding-left: 6px;
    background: var(--bg-panel);
    border-bottom: 1px solid var(--border);
    user-select: none;
    flex: none;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 6px;
    font-weight: 600;
    margin-right: 10px;
    white-space: nowrap;
  }
  .tabs {
    display: flex;
    align-items: center;
    gap: 2px;
    min-width: 0;
    overflow-x: auto;
    scrollbar-width: none;
  }
  .tab {
    display: flex;
    align-items: center;
    gap: 7px;
    cursor: pointer;
    height: 28px;
    padding: 0 10px;
    border-radius: 6px;
    border: 1px solid transparent;
    background: none;
    color: var(--text-muted);
    white-space: nowrap;
    max-width: 240px;
  }
  .tab:hover {
    background: var(--bg-hover);
    color: var(--text);
  }
  .tab .close {
    width: 16px;
    height: 16px;
    margin-right: -4px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    border: none;
    border-radius: 4px;
    background: none;
    color: var(--text-faint);
    visibility: hidden;
    flex: none;
  }
  .tab:hover .close,
  .tab.on .close {
    visibility: visible;
  }
  .tab .close:hover {
    background: var(--bg-active);
    color: var(--text);
  }
  .tab.on {
    background: var(--bg-active);
    border-color: var(--border);
    color: var(--text);
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .acct {
    font-size: 10.5px;
    color: var(--text-faint);
    font-family: var(--mono);
  }
  .acc {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--text-faint);
    flex: none;
  }
  .acc.ok {
    background: var(--accent);
  }
  .acc.bad {
    background: var(--red);
  }
  .add,
  .icon {
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    border: none;
    border-radius: 6px;
    background: none;
    color: var(--text-muted);
    flex: none;
  }
  .icon.on {
    color: var(--text);
  }
  .add:hover,
  .icon:hover {
    background: var(--bg-hover);
    color: var(--text);
  }
  .drag {
    flex: 1;
    align-self: stretch;
    min-width: 24px;
  }
  .ctx {
    position: fixed;
    z-index: 60;
    min-width: 180px;
    background: var(--bg-elev);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    padding: 4px;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.35);
  }
  .ctx button {
    display: block;
    width: 100%;
    text-align: left;
    padding: 6px 10px;
    background: none;
    border: none;
    border-radius: 4px;
  }
  .ctx button:hover {
    background: var(--bg-hover);
  }
  .ctx button .acc {
    display: inline-block;
    margin-right: 6px;
    vertical-align: middle;
  }
  .ctx-title {
    padding: 4px 10px 2px;
    font-size: 11px;
    color: var(--text-faint);
  }
  .ctx hr {
    border: none;
    border-top: 1px solid var(--border);
    margin: 4px 0;
  }
  .add.on {
    background: var(--bg-active);
    color: var(--text);
  }
</style>
