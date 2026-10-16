<script lang="ts">
  import { app } from '../stores/app.svelte'
  import Icon from './Icon.svelte'
  import { t } from '../i18n/index.svelte'
  import { shortcutLabel } from '../shortcuts/shortcuts'
  import { getOverrides, keyHint } from '../shortcuts/store.svelte'

  type Entry = { id: string; label: string; hint?: string; icon: string; run: () => void }
  let q = $state('')
  let idx = $state(0)

  const actions = $derived.by<Entry[]>(() => {
    const list: Entry[] = []
    if (app.selectedProject)
      list.push({ id: 'new', label: t('palette.newThreadIn', { name: app.selectedProject.name }), hint: shortcutLabel('newThread', getOverrides()), icon: 'plus', run: () => app.newThread(app.selectedProject!.id) })
    const target = app.selectedProject ?? app.projects[0]
    if (target)
      list.push({ id: 'new-skip', label: t('palette.newThreadSkipIn', { name: target.name }), hint: '--dangerously-skip-permissions', icon: 'alert',
        run: () => app.newThread(target.id, undefined, 'claude', undefined, true) })
    for (const p of app.projects)
      if (p.id !== app.selectedProject?.id)
        list.push({ id: 'new-' + p.id, label: t('palette.newThreadIn', { name: p.name }), icon: 'plus', run: () => app.newThread(p.id) })
    if (app.selected || app.repoProject) {
      list.push({ id: 'chat', label: t(app.selected ? 'palette.goToChat' : 'palette.repoThreads'), hint: 'Ctrl 1', icon: 'sparkle', run: () => (app.tab = 'chat') })
      if (app.isGit) {
        list.push({ id: 'changes', label: t('palette.changes'), hint: keyHint('toggleChanges'), icon: 'diff', run: () => app.showChanges() })
        list.push({ id: 'history', label: t('palette.history'), hint: 'Ctrl 2', icon: 'branch', run: () => (app.tab = 'history') })
        list.push({ id: 'tree', label: t('palette.tree'), hint: 'Ctrl 3', icon: 'tree', run: () => (app.tab = 'tree') })
        list.push({ id: 'reflog', label: t('palette.reflog'), hint: 'Ctrl 4', icon: 'history', run: () => (app.tab = 'reflog') })
        list.push({ id: 'stats', label: t('palette.stats'), hint: 'Ctrl 5', icon: 'chart', run: () => (app.tab = 'stats') })
      }
      list.push({ id: 'term', label: t('palette.terminal'), hint: app.isGit ? 'Ctrl 6' : 'Ctrl 2', icon: 'terminal', run: () => (app.tab = 'terminal') })
    }
    if (app.selected) list.push({ id: 'arch', label: t('palette.archiveCurrent'), icon: 'archive', run: () => app.archive(app.selectedId!) })
    for (const p of app.projects)
      list.push({ id: 'repo-' + p.id, label: t('palette.openRepo', { name: p.name }), hint: p.path, icon: 'folder', run: () => app.openRepo(p.id) })
    list.push({ id: 'arch-list', label: t(app.showArchived ? 'palette.showActive' : 'palette.showArchived'), icon: 'archive', run: () => (app.showArchived = !app.showArchived) })
    list.push({ id: 'settings', label: t('palette.settings'), hint: shortcutLabel('openSettings', getOverrides()), icon: 'settings', run: () => app.openSettings() })
    list.push({ id: 'accounts', label: t('palette.accounts'), icon: 'key', run: () => app.openSettings('accounts') })
    list.push({ id: 'git-settings', label: t('palette.gitSettings'), icon: 'branch', run: () => app.openSettings('git') })
    list.push({ id: 'integrations', label: t('palette.integrations'), icon: 'git', run: () => app.openSettings('integrations') })
    list.push({ id: 'appearance', label: t('palette.appearance'), icon: 'palette', run: () => app.openSettings('appearance') })
    list.push({ id: 'new-ws', label: t('palette.newWorkspace'), icon: 'layers', run: () => (app.newWorkspaceOpen = true) })
    app.openWorkspaces.forEach((w, i) => {
      if (w.id !== app.activeWorkspaceId)
        list.push({ id: 'ws-' + w.id, label: t('palette.goToWorkspace', { name: w.name }), hint: i < 9 ? `Alt ${i + 1}` : undefined, icon: 'layers', run: () => app.switchWorkspace(w.id) })
    })
    for (const w of app.closedWorkspaces)
      list.push({ id: 'ws-' + w.id, label: t('palette.openWorkspace', { name: w.name }), icon: 'layers', run: () => app.switchWorkspace(w.id) })
    if (app.workspaces.length > 1)
      list.push({ id: 'pick-ws', label: t('palette.pickWorkspaces'), icon: 'layers', run: () => (app.pickingWorkspaces = true) })
    for (const th of [...app.threads].sort((a, b) => b.updated_at - a.updated_at)) {
      const p = app.projects.find((x) => x.id === th.project_id)
      list.push({ id: th.id, label: th.title, hint: (th.archived ? t('palette.archived') : '') + (p?.name ?? ''), icon: th.isolated ? 'layers' : 'folder', run: () => { if (th.archived) app.showArchived = true; app.select(th.id) } })
    }
    return list
  })

  function score(label: string, query: string) {
    if (!query) return 1
    const l = label.toLowerCase()
    const qq = query.toLowerCase()
    if (l.includes(qq)) return 2 + (l.startsWith(qq) ? 1 : 0)
    let i = 0
    for (const c of l) if (c === qq[i]) i++
    return i === qq.length ? 1 : 0
  }

  const results = $derived(
    actions
      .map((a) => ({ a, s: score(a.label + ' ' + (a.hint ?? ''), q) }))
      .filter((x) => x.s > 0)
      .sort((x, y) => y.s - x.s)
      .slice(0, 40)
      .map((x) => x.a),
  )

  $effect(() => {
    void q
    idx = 0
  })

  function run(e: Entry) {
    app.paletteOpen = false
    e.run()
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'ArrowDown') { idx = Math.min(idx + 1, results.length - 1); e.preventDefault() }
    else if (e.key === 'ArrowUp') { idx = Math.max(idx - 1, 0); e.preventDefault() }
    else if (e.key === 'Enter' && results[idx]) run(results[idx])
    else if (e.key === 'Escape') app.paletteOpen = false
  }
</script>

<div class="overlay" role="presentation" onclick={() => (app.paletteOpen = false)}>
  <div class="pal" role="dialog" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={onKey}>
    <!-- svelte-ignore a11y_autofocus -->
    <input autofocus placeholder={t('palette.placeholder')} bind:value={q} />
    <div class="res">
      {#each results as r, i (r.id)}
        <button class:on={i === idx} onmouseenter={() => (idx = i)} onclick={() => run(r)}>
          <Icon name={r.icon} size={14} /> <span class="l">{r.label}</span>
          {#if r.hint}<span class="h">{r.hint}</span>{/if}
        </button>
      {:else}
        <div class="none">{t('palette.nothingFound')}</div>
      {/each}
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.35);
    z-index: 100;
    display: flex;
    justify-content: center;
    padding-top: 12vh;
  }
  .pal {
    width: min(620px, 92vw);
    max-height: 60vh;
    display: flex;
    flex-direction: column;
    background: var(--bg-elev);
    border: 1px solid var(--border-strong);
    border-radius: 12px;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.45);
    overflow: hidden;
  }
  input {
    border: none;
    border-bottom: 1px solid var(--border);
    background: transparent;
    padding: 14px 16px;
    font-size: 14px;
    outline: none;
  }
  .res {
    overflow-y: auto;
    padding: 4px;
  }
  .res button {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 7px 10px;
    border: none;
    background: none;
    border-radius: 6px;
    text-align: left;
    color: var(--text-muted);
  }
  .res button.on {
    background: var(--bg-active);
    color: var(--text);
  }
  .l {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .h {
    font-size: 11px;
    color: var(--text-faint);
  }
  .none {
    padding: 16px;
    color: var(--text-faint);
    text-align: center;
  }
</style>
