<script lang="ts">
  import { app } from '../stores/app.svelte'
  import Icon from './Icon.svelte'
  import Logo from './Logo.svelte'
  import { t } from '../i18n/index.svelte'

  // Escolha de quais workspaces abrir como abas. Vem marcado o que estava
  // aberto da última vez (ou o último ativo); Enter abre.
  const initial = app.openWorkspaceIds.length
    ? app.openWorkspaceIds
    : app.activeWorkspaceId
      ? [app.activeWorkspaceId]
      : app.workspaces.slice(0, 1).map((w) => w.id)
  let picked = $state<string[]>(initial.filter((id) => app.workspaces.some((w) => w.id === id)))

  /** dá para cancelar quando já há workspace aberto (veio do "+" da barra) */
  const canCancel = $derived(app.workspaceShown)
  const allPicked = $derived(picked.length === app.workspaces.length)

  function toggle(id: string) {
    picked = picked.includes(id) ? picked.filter((x) => x !== id) : [...picked, id]
  }

  // mantém a ordem das abas que já existiam; os novos entram no fim, na ordem da lista
  function confirm() {
    if (!picked.length) return
    const kept = app.openWorkspaceIds.filter((id) => picked.includes(id))
    const added = app.workspaces.map((w) => w.id).filter((id) => picked.includes(id) && !kept.includes(id))
    app.openWorkspacesFrom([...kept, ...added])
  }

  function onKey(e: KeyboardEvent) {
    if (app.newWorkspaceOpen || app.settingsOpen || app.paletteOpen) return
    if (e.key === 'Enter') {
      e.preventDefault()
      confirm()
    } else if (e.key === 'Escape' && canCancel) {
      e.preventDefault()
      app.pickingWorkspaces = false
    }
  }

  function authState(dir: string | null) {
    const a = app.auth[dir ?? '']
    if (!a) return 'unknown'
    return a.loggedIn ? 'ok' : 'bad'
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="picker">
  <div class="logo"><Logo size={56} /></div>
  <h1>{t('workspacePicker.title')}</h1>
  <p class="sub">{@html t('workspacePicker.sub')}</p>

  <div class="list" role="group" aria-label="Workspaces">
    {#each app.workspaces as w (w.id)}
      {@const on = picked.includes(w.id)}
      <label class="ws" class:on title={w.path}>
        <input type="checkbox" checked={on} onchange={() => toggle(w.id)} />
        <span class="acc {authState(w.claude_dir)}" title={t('workspacePicker.account', { account: app.accountLabel(w.claude_dir) })}></span>
        <span class="info">
          <span class="name">{w.name}</span>
          <span class="path mono">{w.path}</span>
        </span>
        <span class="acct mono">{app.accountLabel(w.claude_dir)}</span>
      </label>
    {/each}
  </div>

  <div class="actions">
    <button class="btn ghost" onclick={() => (picked = allPicked ? [] : app.workspaces.map((w) => w.id))}>
      {t(allPicked ? 'workspacePicker.unselectAll' : 'workspacePicker.selectAll')}
    </button>
    <button class="btn ghost" onclick={() => (app.newWorkspaceOpen = true)}><Icon name="plus" size={13} /> {t('workspacePicker.newWorkspace')}</button>
    <span class="grow"></span>
    {#if canCancel}<button class="btn ghost" onclick={() => (app.pickingWorkspaces = false)}>{t('workspacePicker.cancel')}</button>{/if}
    <button class="btn primary" disabled={!picked.length} onclick={confirm}>
      {picked.length > 1 ? t('workspacePicker.openMany', { n: picked.length }) : picked.length ? t('workspacePicker.openOne') : t('workspacePicker.open')} <span class="kbd">Enter</span>
    </button>
  </div>
</div>

<style>
  .picker {
    max-width: 620px;
    margin: 10vh auto 0;
    padding: 0 24px 32px;
    color: var(--text-muted);
  }
  .logo {
    display: flex;
    justify-content: center;
    margin-bottom: 8px;
  }
  h1 {
    margin: 4px 0 6px;
    text-align: center;
    color: var(--text);
  }
  .sub {
    text-align: center;
    margin: 0 0 20px;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-height: 52vh;
    overflow-y: auto;
  }
  .ws {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-elev);
    cursor: pointer;
  }
  .ws:hover {
    border-color: var(--border-strong);
  }
  .ws.on {
    border-color: color-mix(in srgb, var(--accent) 60%, var(--border));
    background: var(--accent-soft);
  }
  .ws input {
    accent-color: var(--accent);
    margin: 0;
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
  .info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .name {
    color: var(--text);
    font-weight: 500;
  }
  .path {
    font-size: 11px;
    color: var(--text-faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .acct {
    font-size: 11px;
    color: var(--text-faint);
    flex: none;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 16px;
  }
  .grow {
    flex: 1;
  }
</style>
