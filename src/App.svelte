<script lang="ts">
  import { onMount } from 'svelte'
  import { app, isGitTab } from './lib/stores/app.svelte'
  import { api, errText } from './lib/api'
  import Sidebar from './lib/components/Sidebar.svelte'
  import WorkspacePicker from './lib/components/WorkspacePicker.svelte'
  import ThreadView from './lib/components/ThreadView.svelte'
  import RepoScreen from './lib/components/RepoScreen.svelte'
  import TerminalHost from './lib/components/TerminalHost.svelte'
  import CommandPalette from './lib/components/CommandPalette.svelte'
  import SettingsModal from './lib/components/SettingsModal.svelte'
  import TitleBar from './lib/components/TitleBar.svelte'
  import NewWorkspaceModal from './lib/components/NewWorkspaceModal.svelte'
  import Icon from './lib/components/Icon.svelte'
  import Logo from './lib/components/Logo.svelte'
  import { relTime } from './lib/markdown'
  import { applyUpdate } from './lib/updater'
  import { t } from './lib/i18n/index.svelte'
  import { actionFor, shortcutLabel } from './lib/shortcuts/shortcuts'
  import { getOverrides, loadOverrides } from './lib/shortcuts/store.svelte'
  import ShortcutsReference from './lib/components/ShortcutsReference.svelte'

  let sidebarW = $state(272)
  let dragging: null | 'side' = null
  let ready = $state(false)
  let updating = $state<{ version: string; pct: number | null } | null>(null)
  let shortcutsOpen = $state(false)

  onMount(async () => {
    loadOverrides()
    try {
      sidebarW = Number(localStorage.getItem('sidebarW')) || sidebarW
    } catch {}
    if (!import.meta.env.DEV) await applyUpdate((version, pct) => (updating = { version, pct }))
    updating = null
    try {
      await app.init()
    } catch (e) {
      app.toast(errText(e), 'error')
    }
    ready = true
  })

  function startDrag(which: 'side') {
    dragging = which
    document.body.style.cursor = 'col-resize'
  }

  function onMove(e: MouseEvent) {
    if (dragging === 'side') sidebarW = Math.min(Math.max(200, e.clientX), 480)
  }

  function endDrag() {
    if (!dragging) return
    dragging = null
    document.body.style.cursor = ''
    try {
      localStorage.setItem('sidebarW', String(sidebarW))
    } catch {}
  }

  function cycleThread(dir: 1 | -1) {
    const list = app.threads.filter((t) => t.archived === app.showArchived).sort((a, b) => b.updated_at - a.updated_at)
    if (!list.length) return
    const i = list.findIndex((t) => t.id === app.selectedId)
    const next = list[(i + dir + list.length) % list.length]
    app.select(next.id)
  }

  // Ctrl+1…6 (abas) e Alt+1…9 (workspaces) são fixos; o resto passa pelo
  // registro de atalhos (remapeável em Configurações → Atalhos).
  async function onKey(e: KeyboardEvent) {
    const mod = e.ctrlKey || e.metaKey
    if (mod && !e.shiftKey && !e.altKey && /^[1-6]$/.test(e.key) && (app.selected || app.repoProject)) {
      e.preventDefault()
      const tab = app.tabs[Number(e.key) - 1]
      if (tab) app.tab = tab
      return
    }
    if (e.altKey && /^[1-9]$/.test(e.key) && app.openWorkspaces[Number(e.key) - 1]) {
      e.preventDefault()
      app.switchWorkspace(app.openWorkspaces[Number(e.key) - 1].id)
      return
    }
    const id = actionFor(e, getOverrides())
    switch (id) {
      case 'commandPalette':
        e.preventDefault()
        app.paletteOpen = !app.paletteOpen
        break
      case 'newThread':
        e.preventDefault()
        if (app.selectedProject) app.newThread(app.selectedProject.id)
        else if (app.projects[0]) app.newThread(app.projects[0].id)
        break
      case 'toggleChanges':
        e.preventDefault()
        if (app.tab === 'history') app.tab = 'chat'
        else app.showChanges()
        break
      case 'toggleTerminal':
        e.preventDefault()
        app.tab = app.tab === 'terminal' ? 'chat' : 'terminal'
        break
      case 'toggleSidebar':
        e.preventDefault()
        app.toggleSidebar()
        break
      case 'openSettings':
        e.preventDefault()
        app.openSettings()
        break
      case 'showShortcuts':
        e.preventDefault()
        shortcutsOpen = !shortcutsOpen
        break
      case 'nextWorkspace':
      case 'prevWorkspace': {
        if (app.openWorkspaces.length < 2) break
        e.preventDefault()
        const i = app.openWorkspaces.findIndex((w) => w.id === app.activeWorkspaceId)
        const n = app.openWorkspaces.length
        const dir = id === 'prevWorkspace' ? -1 : 1
        app.switchWorkspace(app.openWorkspaces[(i + dir + n) % n].id)
        break
      }
      case 'toggleBookmark':
        if (!app.selected) break
        e.preventDefault()
        app.upsert(await api.updateThreadMeta(app.selected.id, { bookmarked: !app.selected.bookmarked }))
        break
      case 'prevThread':
      case 'nextThread':
        e.preventDefault()
        cycleThread(id === 'prevThread' ? -1 : 1)
        break
    }
  }

  // pasta sem `.git`: as abas git somem; se uma estava aberta, volta à conversa
  $effect(() => {
    if (!app.isGit && isGitTab(app.tab)) app.tab = 'chat'
  })

  // atalhos e menu de contexto do git só valem com um painel Git visível
  let gitLoaded = false
  $effect(() => {
    const on = !!(app.selected || app.repoProject) && isGitTab(app.tab)
    if (!on && !gitLoaded) return
    gitLoaded = true
    import('./git/mount').then((m) => {
      m.setGitNavigator((tab) => (app.tab = tab))
      m.setGitActive(on)
    })
  })

  // terminal da thread (no checkout dela) ou do repositório aberto
  const termKey = $derived(app.selected?.id ?? (app.repoProject ? `repo-${app.repoProject.id}` : null))
  const termCwd = $derived(app.selected?.cwd ?? app.repoProject?.path ?? null)

  const recent = $derived(
    [...app.threads].filter((t) => !t.archived).sort((a, b) => b.updated_at - a.updated_at).slice(0, 6),
  )
</script>

<svelte:window onkeydown={onKey} onmousemove={onMove} onmouseup={endDrag} />

<div class="window">
<TitleBar />
{#if ready && !app.workspaces.length}
  <div class="first">
    <div class="logo"><Logo size={88} /></div>
    <h1>{t('shell.welcomeTitle')}</h1>
    <p>{t('shell.welcomeText')}</p>
    <button class="btn primary" onclick={() => (app.newWorkspaceOpen = true)}><Icon name="plus" /> {t('shell.createWorkspace')}</button>
  </div>
{:else if app.pickingWorkspaces}
  <WorkspacePicker />
{:else}
<div class="shell" style="grid-template-columns: {app.sidebarOpen ? `${sidebarW}px 4px` : ''} 1fr">
  {#if app.sidebarOpen}
    <Sidebar />
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div class="gutter" role="separator" aria-orientation="vertical" onmousedown={() => startDrag('side')}></div>
  {/if}

  <main>
    {#if app.selected}
      <ThreadView thread={app.selected} />
    {:else if app.repoProject}
      <RepoScreen project={app.repoProject} />
    {:else if ready}
      <div class="welcome">
        <div class="logo"><Logo size={64} /></div>
        <h1>{app.activeWorkspace?.name}</h1>
        <p>
          {t(app.projects.length === 1 ? 'shell.reposOne' : 'shell.reposOther', { n: app.projects.length })} · {t('shell.account')} <code>{app.accountLabel(app.activeWorkspace?.claude_dir)}</code>
          {#if app.authOf(app.activeWorkspace) && !app.authOf(app.activeWorkspace)?.loggedIn}
            <br /><span class="warn">{t('shell.notLoggedIn')}</span>
          {/if}
        </p>
        <div class="cta">
          {#if app.projects.length}
            <button class="btn primary" onclick={() => app.newThread(app.projects[0].id)}><Icon name="plus" /> {t('shell.newThreadIn', { name: app.projects[0].name })}</button>
          {/if}
          <button class="btn ghost" onclick={() => (app.paletteOpen = true)}>{t('shell.palette')} <span class="kbd">{shortcutLabel('commandPalette', getOverrides())}</span></button>
        </div>
        {#if recent.length}
          <div class="recent">
            <div class="rl">{t('shell.recent')}</div>
            {#each recent as t (t.id)}
              <button class="ri" onclick={() => app.select(t.id)}>
                {#if t.status === 'running'}<span class="spinner"></span>
                {:else}<span class="dot {t.status !== 'idle' ? t.status : t.unread ? 'unread' : ''}"></span>{/if}
                <span class="rt">{t.title}</span>
                <span class="rp">{app.projects.find((p) => p.id === t.project_id)?.name}</span>
                <span class="rp">{relTime(t.updated_at)}</span>
              </button>
            {/each}
          </div>
        {/if}
      </div>
    {/if}
    <TerminalHost {termKey} cwd={termCwd} visible={!!termKey && app.tab === 'terminal'} />
  </main>

</div>

{/if}
</div>

{#if updating}
  <div class="updating">
    <div class="logo"><Logo size={64} /></div>
    <div>{t('shell.updating', { version: updating.version })}</div>
    <div class="bar"><div style="width:{Math.round((updating.pct ?? 0) * 100)}%"></div></div>
  </div>
{/if}

{#if app.paletteOpen}<CommandPalette />{/if}
{#if app.newWorkspaceOpen}<NewWorkspaceModal />{/if}
{#if app.settingsOpen}<SettingsModal />{/if}
{#if shortcutsOpen}<ShortcutsReference onclose={() => (shortcutsOpen = false)} />{/if}

<div class="toasts">
  {#each app.toasts as t (t.id)}
    <div class="toast {t.kind}">{t.text}</div>
  {/each}
</div>

<style>
  .window {
    display: flex;
    flex-direction: column;
    height: 100vh;
    width: 100vw;
  }
  .shell {
    display: grid;
    /* a linha ocupa exatamente a altura disponível: sem isso ela cresce com o
       conteúdo e as listas internas nunca ganham scroll */
    grid-template-rows: minmax(0, 1fr);
    flex: 1;
    min-height: 0;
  }
  main {
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }
  .first {
    max-width: 520px;
    margin: 16vh auto 0;
    padding: 0 24px;
    text-align: center;
    color: var(--text-muted);
  }
  .first h1 {
    margin: 4px 0 10px;
  }
  .first p {
    margin-bottom: 20px;
  }
  .warn {
    color: var(--amber);
  }
  .welcome code {
    font-size: 12px;
  }
  .gutter {
    cursor: col-resize;
    background: transparent;
    margin: 0 -2px;
    z-index: 5;
  }
  .gutter:hover {
    background: var(--accent-soft);
  }
  .welcome {
    max-width: 560px;
    margin: 14vh auto 0;
    padding: 0 24px;
    text-align: center;
    color: var(--text-muted);
  }
  .logo {
    display: flex;
    justify-content: center;
    margin-bottom: 6px;
  }
  h1 {
    color: var(--text);
    font-size: 22px;
    margin: 4px 0 8px;
  }
  .cta {
    display: flex;
    gap: 8px;
    justify-content: center;
    flex-wrap: wrap;
    margin-top: 18px;
  }
  .recent {
    margin-top: 36px;
    text-align: left;
  }
  .rl {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.6px;
    color: var(--text-faint);
    margin-bottom: 6px;
  }
  .ri {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 7px 10px;
    border: none;
    background: none;
    border-radius: 6px;
    text-align: left;
  }
  .ri:hover {
    background: var(--bg-hover);
  }
  .rt {
    flex: 1;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .rp {
    font-size: 12px;
    color: var(--text-faint);
  }
  .updating {
    position: fixed;
    inset: 0;
    z-index: 300;
    background: var(--bg);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    color: var(--text-muted);
  }
  .updating .bar {
    width: 260px;
    height: 4px;
    border-radius: 4px;
    background: var(--bg-active);
    overflow: hidden;
  }
  .updating .bar div {
    height: 100%;
    background: var(--accent);
    transition: width 0.2s;
  }
  .toasts {
    position: fixed;
    right: 16px;
    bottom: 16px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    z-index: 200;
    max-width: 420px;
  }
  .toast {
    background: var(--bg-elev);
    border: 1px solid var(--border-strong);
    border-left: 3px solid var(--blue);
    border-radius: var(--radius);
    padding: 9px 12px;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.3);
    white-space: pre-wrap;
    user-select: text;
  }
  .toast.error {
    border-left-color: var(--red);
  }
  .toast.ok {
    border-left-color: var(--accent);
  }
</style>
