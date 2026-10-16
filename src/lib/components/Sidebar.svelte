<script lang="ts">
  import { tick } from 'svelte'
  import { app } from '../stores/app.svelte'
  import { api, type Thread } from '../api'
  import { relTime } from '../markdown'
  import { buildGroups, flatten, type Row, type TreeMode } from '../tree'
  import Icon from './Icon.svelte'
  import Avatar from './Avatar.svelte'
  import { t } from '../i18n/index.svelte'
  import { keyHint } from '../shortcuts/store.svelte'

  const PER_GROUP = 8
  const STATUS_KEY: Record<string, string> = { running: 'sidebar.statusRunning', awaiting: 'sidebar.statusAwaiting', error: 'sidebar.statusError' }

  let filter = $state('')
  let mode = $state<TreeMode>('project')
  let collapsed = $state<Record<string, boolean>>({})
  let showAll = $state<Record<string, boolean>>({})
  let focusIdx = $state(-1)
  let listEl: HTMLDivElement | undefined = $state()
  let filterEl: HTMLInputElement | undefined = $state()
  // `menu` guarda a última posição/thread; só `menuOpen` fecha. Zerar `menu`
  // no clique fazia o {@const t = menu.thread} reavaliar com null.
  let menu = $state<{ x: number; y: number; thread: Thread } | null>(null)
  let menuOpen = $state(false)
  let renaming = $state<string | null>(null)
  let renameValue = $state('')

  // ---- estado da árvore por workspace (aberto/fechado e modo) ----
  const storeKey = $derived(`tree:${app.activeWorkspaceId}`)
  let loadedKey = ''
  $effect(() => {
    const k = storeKey
    if (k === loadedKey) return
    loadedKey = k
    try {
      const saved = JSON.parse(localStorage.getItem(k) ?? '{}')
      collapsed = saved.collapsed ?? {}
      mode = saved.mode ?? 'project'
    } catch {
      collapsed = {}
      mode = 'project'
    }
    showAll = {}
    focusIdx = -1
  })
  function persist() {
    try {
      localStorage.setItem(storeKey, JSON.stringify({ collapsed, mode }))
    } catch {}
  }

  const filtering = $derived(filter.trim().length > 0)
  const groups = $derived(buildGroups(app.threads, app.projects, { mode, filter, archived: app.showArchived }))
  const rows = $derived(flatten(groups, collapsed, showAll, { perGroup: PER_GROUP, filtering }))
  const anyOpen = $derived(rows.some((r) => r.type === 'group' && r.open && r.group.kind !== 'idle'))

  function toggle(key: string, open?: boolean) {
    const row = rows.find((r) => r.type === 'group' && r.key === key)
    const isOpenNow = row?.type === 'group' ? row.open : false
    collapsed[key] = open === undefined ? isOpenNow : !open
    persist()
  }

  function setAll(open: boolean) {
    const next: Record<string, boolean> = {}
    const walk = (gs: typeof groups) => {
      for (const g of gs) {
        next[g.key] = !open
        if (g.children) walk(g.children)
      }
    }
    walk(groups)
    collapsed = next
    persist()
  }

  function setMode(m: TreeMode) {
    mode = m
    persist()
  }

  // ---- revela a thread aberta: abre o grupo e rola até ela ----
  $effect(() => {
    const id = app.selectedId
    if (!id) return
    const t = app.threads.find((x) => x.id === id)
    if (!t) return
    const key = mode === 'project' ? `p:${t.project_id}` : null
    if (key && collapsed[key]) {
      collapsed[key] = false
      persist()
    }
    const g = groups.find((x) => x.key === key)
    if (g && g.threads.findIndex((x) => x.id === id) >= PER_GROUP) showAll[g.key] = true
    tick().then(() => listEl?.querySelector('.thread.active')?.scrollIntoView({ block: 'nearest' }))
  })

  // ---- teclado: ↑/↓ percorrem, ←/→ fecham/abrem, Enter abre ----
  function focusRow(i: number) {
    focusIdx = Math.max(0, Math.min(rows.length - 1, i))
    tick().then(() => listEl?.querySelector(`[data-idx="${focusIdx}"]`)?.scrollIntoView({ block: 'nearest' }))
  }

  function activate(r: Row) {
    if (r.type === 'group') toggle(r.key)
    else if (r.type === 'thread') app.select(r.thread.id)
    else if (r.type === 'more') showAll[r.parent] = true
  }

  function onKey(e: KeyboardEvent) {
    if (renaming || (e.target as HTMLElement).tagName === 'INPUT') return
    const r = rows[focusIdx]
    if (e.key === 'ArrowDown') { e.preventDefault(); focusRow(focusIdx + 1) }
    else if (e.key === 'ArrowUp') { e.preventDefault(); focusRow(focusIdx - 1) }
    else if (e.key === 'Home') { e.preventDefault(); focusRow(0) }
    else if (e.key === 'End') { e.preventDefault(); focusRow(rows.length - 1) }
    else if (e.key === 'ArrowRight' && r?.type === 'group') { e.preventDefault(); if (!r.open) toggle(r.key, true); else focusRow(focusIdx + 1) }
    else if (e.key === 'ArrowLeft' && r) {
      e.preventDefault()
      if (r.type === 'group' && r.open) toggle(r.key, false)
      else if (r.type !== 'group') focusRow(rows.findIndex((x) => x.key === r.parent))
    } else if ((e.key === 'Enter' || e.key === ' ') && r) { e.preventDefault(); activate(r) }
    else if (e.key === '/' ) { e.preventDefault(); filterEl?.focus() }
  }

  // ---- ações de thread ----
  function startRename(t: Thread) {
    renaming = t.id
    renameValue = t.title
    menuOpen = false
  }

  async function commitRename(t: Thread) {
    const v = renameValue.trim()
    renaming = null
    if (v && v !== t.title) app.upsert(await api.updateThreadMeta(t.id, { title: v }))
  }

  async function toggleBookmark(t: Thread) {
    menuOpen = false
    app.upsert(await api.updateThreadMeta(t.id, { bookmarked: !t.bookmarked }))
  }

  async function markUnread(t: Thread) {
    menuOpen = false
    app.upsert(await api.updateThreadMeta(t.id, { unread: !t.unread }))
  }

  function onContext(e: MouseEvent, t: Thread) {
    e.preventDefault()
    menu = { x: e.clientX, y: e.clientY, thread: t }
    menuOpen = true
  }

  function newThread() {
    const p = app.selectedProject ?? groups.find((g) => g.kind === 'project')?.project ?? app.projects[0]
    if (p) app.newThread(p.id)
    else app.openSettings('workspaces')
  }

  const projectName = (id: string) => app.projects.find((p) => p.id === id)?.name ?? ''
</script>

<svelte:window onclick={() => (menuOpen = false)} />

<aside class="sidebar">
  <div class="head">
    <span class="label">{app.showArchived ? t('sidebar.archived') : t('sidebar.threads')}</span>
    <div class="tools">
      <button class="tb" class:on={mode === 'recent'} title={mode === 'project' ? t('sidebar.groupByDate') : t('sidebar.groupByProject')}
        onclick={() => setMode(mode === 'project' ? 'recent' : 'project')}>
        <Icon name={mode === 'project' ? 'clock' : 'folder'} size={13} />
      </button>
      <button class="tb" title={anyOpen ? t('sidebar.collapseAll') : t('sidebar.expandAll')} onclick={() => setAll(!anyOpen)}>
        <Icon name={anyOpen ? 'collapse' : 'expand'} size={13} />
      </button>
      <button class="tb" class:on={app.showArchived} title={app.showArchived ? t('sidebar.showActive') : t('sidebar.showArchived')}
        onclick={() => (app.showArchived = !app.showArchived)}>
        <Icon name="archive" size={13} />
      </button>
      <button class="tb" title={t('sidebar.newThread', { key: keyHint('newThread') })} disabled={app.creating} onclick={newThread}><Icon name="plus" size={14} /></button>
    </div>
  </div>

  <div class="search">
    <Icon name="search" size={12} />
    <input bind:this={filterEl} placeholder={t('sidebar.filterPlaceholder')} bind:value={filter}
      onkeydown={(e) => {
        if (e.key === 'Escape') { filter = ''; listEl?.focus() }
        if (e.key === 'ArrowDown') { e.preventDefault(); listEl?.focus(); focusRow(0) }
      }} />
    {#if filter}<button class="clear" title={t('sidebar.clear')} onclick={() => (filter = '')}><Icon name="x" size={11} /></button>{/if}
  </div>
  {#if app.creating}<div class="creating"><span class="dot running"></span> {t('sidebar.preparingCheckout')}</div>{/if}

  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div class="list" bind:this={listEl} tabindex="0" role="tree" aria-label="Threads" onkeydown={onKey}
    onfocus={() => focusIdx < 0 && rows.length && (focusIdx = rows.findIndex((r) => r.type === 'thread' && r.thread.id === app.selectedId))}>
    {#each rows as r, i (r.key)}
      {#if r.type === 'group'}
        {@const g = r.group}
        {@const repoId = g.kind === 'project' ? g.project?.id : undefined}
        {@const running = g.threads.filter((t) => t.status === 'running').length}
        <!-- numa pasta, clicar no nome abre a tela do repositório; a seta recolhe/expande -->
        <div class="node group {g.kind}" class:focus={i === focusIdx} class:repo-on={!!repoId && app.repoProjectId === repoId}
          data-idx={i} style="--d:{r.depth}"
          role="treeitem" aria-expanded={r.open} aria-selected={false} tabindex="-1"
          onclick={() => { focusIdx = i; if (repoId) app.openRepo(repoId); else toggle(r.key) }} onkeydown={() => {}}
          title={g.project ? `${g.project.path}\n${t('sidebar.openRepoHint')}` : ''}>
          <button class="chev" class:open={r.open} title={t('sidebar.collapseExpand')}
            onclick={(e) => { e.stopPropagation(); focusIdx = i; toggle(r.key) }}><Icon name="chevron" size={11} /></button>
          {#if g.kind === 'favorites'}<Icon name="bookmark" size={12} />
          {:else if g.kind === 'project'}<Icon name="folder" size={12} />
          {:else if g.kind === 'idle'}<Icon name="layers" size={12} />{/if}
          <span class="gname">{g.label}</span>
          {#if running}
            <span class="run-badge" title={t(running === 1 ? 'sidebar.runningOne' : 'sidebar.runningOther', { n: running })}>
              <span class="spinner"></span>{running}
            </span>
          {:else if !r.open && g.activity}<span class="dot {g.activity}" title={t('sidebar.needsAttention')}></span>{/if}
          <span class="count">{g.kind === 'idle' ? g.children?.length : g.threads.length}</span>
          {#if g.kind === 'project' && g.project}
            {@const pid = g.project.id}
            <button class="act" title={t('sidebar.newThreadIn', { name: g.label })} onclick={(e) => { e.stopPropagation(); app.newThread(pid) }}>
              <Icon name="plus" size={12} />
            </button>
          {/if}
        </div>
      {:else if r.type === 'thread'}
        {@const th = r.thread}
        <div class="node thread" class:active={app.selectedId === th.id} class:focus={i === focusIdx} data-idx={i} style="--d:{r.depth}"
          role="treeitem" aria-selected={app.selectedId === th.id} tabindex="-1"
          onclick={() => { focusIdx = i; app.select(th.id) }} onkeydown={() => {}}
          ondblclick={() => startRename(th)} oncontextmenu={(e) => onContext(e, th)}>
          <span class="status">
            {#if th.status === 'running'}<span class="spinner" title={t(STATUS_KEY.running)}></span>
            {:else if th.status !== 'idle'}<span class="dot {th.status}" title={STATUS_KEY[th.status] ? t(STATUS_KEY[th.status]) : th.status}></span>
            {:else if th.unread}<span class="dot unread"></span>{/if}
          </span>
          {#if renaming === th.id}
            <!-- svelte-ignore a11y_autofocus -->
            <input class="rename" bind:value={renameValue} autofocus
              onblur={() => commitRename(th)}
              onkeydown={(e) => { e.stopPropagation(); if (e.key === 'Enter') commitRename(th); if (e.key === 'Escape') renaming = null }}
              onclick={(e) => e.stopPropagation()} />
          {:else}
            <span class="title" class:unread={th.unread}>{th.title}</span>
          {/if}
          {#if mode === 'recent' || r.parent === 'fav'}<span class="proj">{projectName(th.project_id)}</span>{/if}
          {#if th.skip_permissions}<span class="skip" title={t('sidebar.noPermissions')}><Icon name="alert" size={11} /></span>{/if}
          {#if th.provider !== 'claude'}<span class="prov prov-{th.provider}" title={th.provider === 'codex' ? 'OpenAI Codex' : 'Google Gemini'}>{th.provider === 'codex' ? 'O' : 'G'}</span>{/if}
          {#if th.isolated}<span class="iso" title={t('sidebar.isolated', { branch: th.branch ?? '' })}><Icon name="layers" size={10} /></span>{/if}
          <span class="time">{relTime(th.updated_at)}</span>
          <button class="act" title={th.archived ? t('sidebar.unarchive') : t('sidebar.archive')} onclick={(e) => { e.stopPropagation(); app.archive(th.id, !th.archived) }}>
            <Icon name="archive" size={12} />
          </button>
        </div>
      {:else if r.type === 'more'}
        <button class="node more" class:focus={i === focusIdx} data-idx={i} style="--d:{r.depth}" onclick={() => (showAll[r.parent] = true)}>
          {t('sidebar.showMore', { n: r.hidden })}
        </button>
      {:else}
        <div class="node empty" data-idx={i} style="--d:{r.depth}">{t('sidebar.noThreads')}</div>
      {/if}
    {/each}

    {#if !rows.length && !app.loading}
      <div class="onboard">
        {#if filtering}
          {t('sidebar.nothingMatches', { filter })}
        {:else if app.showArchived}
          {t('sidebar.noArchived')}
        {:else if !app.projects.length}
          <p>{t('sidebar.noReposPrefix')}<code>{app.activeWorkspace?.path}</code>{t('sidebar.noReposSuffix')}</p>
          <button class="btn" onclick={() => app.openSettings('workspaces')}>{t('sidebar.configureWorkspace')}</button>
        {/if}
      </div>
    {/if}
  </div>

  <div class="me">
    <Avatar kind="user" src={app.profile?.avatar} name={app.profile?.name} size={24} />
    <span class="me-name" title={app.profile?.email ?? ''}>{app.profile?.name ?? t('sidebar.you')}</span>
    <button class="btn ghost icon" title={t('sidebar.settings', { key: keyHint('openSettings') })} onclick={() => app.openSettings()}><Icon name="settings" size={14} /></button>
  </div>
</aside>

{#if menuOpen && menu}
  {@const th = menu.thread}
  <div class="ctx" style="left:{menu.x}px; top:{menu.y}px">
    <button onclick={() => startRename(th)}>{t('sidebar.rename')}</button>
    <button onclick={() => toggleBookmark(th)}>{th.bookmarked ? t('sidebar.removeFavorite') : t('sidebar.favorite')}</button>
    <button onclick={() => markUnread(th)}>{th.unread ? t('sidebar.markRead') : t('sidebar.markUnread')}</button>
    <button onclick={() => { menuOpen = false; api.openPath(th.cwd) }}>{t('sidebar.openCheckout')}</button>
    <hr />
    <button onclick={() => { menuOpen = false; app.archive(th.id, !th.archived) }}>{th.archived ? t('sidebar.unarchive') : t('sidebar.archive')}</button>
    {#if th.archived}
      <button class="danger" onclick={() => { menuOpen = false; if (confirm(t('sidebar.confirmDelete', { title: th.title, branch: th.branch ?? '' }))) app.remove(th.id) }}>{t('sidebar.delete')}</button>
    {/if}
  </div>
{/if}

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    overflow: hidden;
    background: var(--bg-panel);
    border-right: 1px solid var(--border);
    min-width: 0;
  }
  .head {
    display: flex;
    align-items: center;
    padding: 8px 8px 4px 14px;
  }
  .label {
    flex: 1;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.6px;
    text-transform: uppercase;
    color: var(--text-faint);
  }
  .tools {
    display: flex;
    gap: 1px;
  }
  .tb {
    width: 24px;
    height: 24px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: none;
    border-radius: 5px;
    background: none;
    color: var(--text-muted);
  }
  .tb:hover:not(:disabled) {
    background: var(--bg-hover);
    color: var(--text);
  }
  .tb.on {
    background: var(--bg-active);
    color: var(--text);
  }
  .search {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 4px 8px 6px;
    padding: 0 8px;
    height: 28px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg-input);
    color: var(--text-faint);
  }
  .search:focus-within {
    border-color: var(--border-strong);
  }
  .search input {
    flex: 1;
    min-width: 0;
    border: none;
    outline: none;
    background: transparent;
    font-size: 12.5px;
    color: var(--text);
    user-select: text;
  }
  .clear {
    border: none;
    background: none;
    color: var(--text-faint);
    display: inline-flex;
    padding: 2px;
  }
  .creating {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11.5px;
    color: var(--text-muted);
    padding: 0 14px 6px;
  }
  .list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overscroll-behavior: contain;
    padding: 0 6px 10px;
    outline: none;
  }
  .node {
    --indent: calc(var(--d) * 14px);
    position: relative;
    display: flex;
    align-items: center;
    gap: 6px;
    height: var(--row-h, 28px);
    padding: 0 6px 0 calc(6px + var(--indent));
    border-radius: 5px;
    cursor: default;
    color: var(--text-muted);
    width: 100%;
  }
  /* linha-guia da hierarquia */
  .node.thread::before,
  .node.more::before,
  .node.empty::before,
  .node.group.project:not([style*='--d:0'])::before {
    content: '';
    position: absolute;
    left: calc(var(--indent) - 4px);
    top: 0;
    bottom: 0;
    border-left: 1px solid var(--border);
  }
  .node:hover {
    background: var(--bg-hover);
  }
  .list:focus-visible .node.focus {
    outline: 1px solid color-mix(in srgb, var(--accent) 60%, transparent);
    outline-offset: -1px;
  }
  .group {
    font-weight: 500;
    color: var(--text);
    margin-top: 2px;
  }
  .group.idle,
  .group.favorites {
    color: var(--text-muted);
  }
  .group.repo-on {
    background: var(--bg-active);
  }
  .chev {
    display: inline-flex;
    background: none;
    border: none;
    padding: 0;
    color: var(--text-faint);
    transition: transform 0.12s;
  }
  .chev.open {
    transform: rotate(90deg);
  }
  .gname {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .count {
    font-size: 11px;
    color: var(--text-faint);
    font-weight: 400;
  }
  .act {
    display: none;
    width: 20px;
    height: 20px;
    align-items: center;
    justify-content: center;
    border: none;
    border-radius: 4px;
    background: none;
    color: var(--text-muted);
    flex: none;
  }
  .act:hover {
    background: var(--bg-active);
    color: var(--text);
  }
  .node:hover .act {
    display: inline-flex;
  }
  .group:hover .count,
  .thread:hover .time {
    display: none;
  }
  .thread.active {
    background: var(--bg-active);
    color: var(--text);
  }
  .run-badge {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
    color: var(--accent);
  }
  .status {
    width: 10px;
    display: inline-flex;
    justify-content: center;
    flex: none;
  }
  .title {
    flex: 1;
    min-width: 45%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .title.unread {
    color: var(--text);
    font-weight: 600;
  }
  .proj {
    font-size: 10.5px;
    color: var(--text-faint);
    max-width: 64px;
    flex: 0 1 auto;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .rename {
    flex: 1;
    min-width: 0;
    background: var(--bg-input);
    border: 1px solid var(--border-strong);
    border-radius: 4px;
    padding: 0 4px;
    outline: none;
  }
  .skip {
    color: var(--red);
    display: inline-flex;
    flex: none;
  }
  .iso {
    color: var(--text-faint);
    display: inline-flex;
  }
  .prov {
    font-size: 9.5px;
    font-weight: 700;
    width: 14px;
    height: 14px;
    border-radius: 4px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: none;
  }
  .time {
    font-size: 10.5px;
    color: var(--text-faint);
    flex: none;
  }
  .more {
    border: none;
    background: none;
    font-size: 11.5px;
    color: var(--text-faint);
    text-align: left;
    padding-left: calc(20px + var(--indent));
  }
  .more:hover {
    color: var(--text);
  }
  .empty {
    font-size: 11.5px;
    color: var(--text-faint);
    padding-left: calc(20px + var(--indent));
  }
  .onboard {
    padding: 20px 10px;
    color: var(--text-muted);
    font-size: 12.5px;
  }
  .onboard code {
    font-size: 11px;
  }
  .me {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    border-top: 1px solid var(--border);
  }
  .me-name {
    flex: 1;
    min-width: 0;
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ctx {
    position: fixed;
    z-index: 50;
    min-width: 190px;
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
  .ctx .danger {
    color: var(--red);
  }
  .ctx hr {
    border: none;
    border-top: 1px solid var(--border);
    margin: 4px 0;
  }
</style>
