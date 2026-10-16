<script lang="ts">
  import { onMount } from 'svelte'
  import { app, accountKey, type SettingsTab } from '../stores/app.svelte'
  import { api, MODELS, PERMISSION_MODES, type ClaudeDir, type RepoEntry } from '../api'
  import { LOCALES, setLocale, t } from '../i18n/index.svelte'
  import Icon from './Icon.svelte'
  import AccountSelect from './AccountSelect.svelte'
  import CapabilitiesPanel from './CapabilitiesPanel.svelte'
  import GitSettings from './GitSettings.svelte'
  import ProvidersPanel from './ProvidersPanel.svelte'
  import AppearanceSettings from './AppearanceSettings.svelte'
  import ShortcutsList from './ShortcutsList.svelte'
  import { getOverrides, resetAllBindings } from '../shortcuts/store.svelte'

  const TABS: { id: SettingsTab; labelKey: string; icon: string }[] = [
    { id: 'workspaces', labelKey: 'settings.tabWorkspaces', icon: 'layers' },
    { id: 'appearance', labelKey: 'settings.tabAppearance', icon: 'palette' },
    { id: 'skills', labelKey: 'settings.tabSkills', icon: 'sparkle' },
    { id: 'agents', labelKey: 'settings.tabAgents', icon: 'user' },
    { id: 'mcp', labelKey: 'settings.tabMcp', icon: 'git' },
    { id: 'accounts', labelKey: 'settings.tabAccounts', icon: 'key' },
    { id: 'providers', labelKey: 'settings.tabProviders', icon: 'sparkle' },
    { id: 'defaults', labelKey: 'settings.tabDefaults', icon: 'sliders' },
    { id: 'git', labelKey: 'settings.tabGit', icon: 'branch' },
    { id: 'integrations', labelKey: 'settings.tabIntegrations', icon: 'git' },
    { id: 'shortcuts', labelKey: 'settings.tabShortcuts', icon: 'keyboard' },
  ]

  let dirs = $state<ClaudeDir[]>([])
  let repos = $state<RepoEntry[]>([])
  let reposLoading = $state(false)
  let nameDraft = $state('')

  const ws = $derived(app.workspaces.find((w) => w.id === app.settingsWorkspaceId) ?? app.workspaces[0] ?? null)

  onMount(async () => {
    dirs = await api.listClaudeDirs()
    for (const d of dirs) if (app.auth[d.is_default ? '' : d.path] === undefined) app.refreshAuth(d.is_default ? null : d.path)
  })

  // repositórios e nome do workspace em edição
  let loadedFor = ''
  $effect(() => {
    const w = ws
    if (!w || w.id === loadedFor) return
    loadedFor = w.id
    nameDraft = w.name
    reposLoading = true
    api
      .workspaceRepos(w.id)
      .then((r) => (repos = r))
      .catch(() => (repos = []))
      .finally(() => (reposLoading = false))
  })

  function toggleRepo(path: string) {
    if (!ws) return
    const ignored = new Set(ws.ignored_paths)
    if (ignored.has(path)) ignored.delete(path)
    else ignored.add(path)
    app.updateWorkspace(ws.id, { ignoredPaths: [...ignored] })
  }

  function saveName() {
    if (ws && nameDraft.trim() && nameDraft.trim() !== ws.name) app.updateWorkspace(ws.id, { name: nameDraft.trim() })
  }

  async function remove() {
    if (!ws) return
    if (!confirm(t('settings.removeConfirm', { name: ws.name }))) return
    await app.removeWorkspace(ws.id)
  }

  async function saveDefaults() {
    if (app.settings) await api.saveSettings(app.settings)
  }

  function usedBy(dir: ClaudeDir) {
    const k = dir.is_default ? '' : dir.path
    return app.workspaces.filter((w) => accountKey(w.claude_dir) === k).map((w) => w.name)
  }

  const included = $derived(ws ? repos.filter((r) => !ws.ignored_paths.includes(r.path)).length : 0)
</script>

<div class="overlay" role="presentation" onclick={() => (app.settingsOpen = false)}>
  <div class="modal" role="dialog" tabindex="-1" onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.key === 'Escape' && (app.settingsOpen = false)}>
    <header>
      <h3>{t('settings.title')}</h3>
      <button class="btn ghost icon" title={t('settings.close')} onclick={() => (app.settingsOpen = false)}><Icon name="x" /></button>
    </header>
    <div class="layout">
      <nav>
        {#each TABS as tab (tab.id)}
          <button class:on={app.settingsTab === tab.id} onclick={() => (app.settingsTab = tab.id)}>
            <Icon name={tab.icon} size={14} /> {t(tab.labelKey)}
          </button>
        {/each}
      </nav>

      <div class="content">
        {#if app.settingsTab === 'workspaces'}
          <div class="ws-layout">
            <div class="ws-list">
              {#each app.workspaces as w (w.id)}
                <button class:on={ws?.id === w.id} onclick={() => (app.settingsWorkspaceId = w.id)}>
                  <span class="dot" class:ok={app.authOf(w)?.loggedIn} class:bad={app.authOf(w) && !app.authOf(w)?.loggedIn}></span>
                  <span class="wn">{w.name}</span>
                </button>
              {/each}
              <button class="add" onclick={() => (app.newWorkspaceOpen = true)}><Icon name="plus" size={13} /> {t('settings.newWorkspace')}</button>
            </div>

            {#if ws}
              <div class="ws-edit">
                <span class="label">{t('settings.name')}</span>
                <input class="field" bind:value={nameDraft} onblur={saveName} onkeydown={(e) => e.key === 'Enter' && saveName()} />

                <span class="label">{t('settings.folder')}</span>
                <div class="row">
                  <code class="path">{ws.path}</code>
                  <button class="btn ghost icon" title={t('settings.openFolder')} onclick={() => api.openPath(ws.path)}><Icon name="external" size={13} /></button>
                </div>

                <span class="label">{t('settings.claudeAccount')}</span>
                {#key ws.id}
                  <AccountSelect value={ws.claude_dir} onchange={(v) => app.updateWorkspace(ws.id, { claudeDir: v })} />
                {/key}
                {@render authLine(ws.claude_dir)}
                <p class="hint">{t('settings.accountHint')}</p>

                <span class="label">{t('settings.reposIncluded', { included, total: repos.length })}</span>
                <ul class="repos">
                  {#if reposLoading}
                    <li class="muted">{t('settings.searchingRepos')}</li>
                  {/if}
                  {#each repos as r (r.path)}
                    {@const inc = !ws.ignored_paths.includes(r.path)}
                    <li>
                      <label>
                        <input type="checkbox" checked={inc} onchange={() => toggleRepo(r.path)} />
                        <span class:off={!inc}>{r.name}</span>
                      </label>
                    </li>
                  {:else}
                    {#if !reposLoading}<li class="muted">{t('settings.noRepos')}</li>{/if}
                  {/each}
                </ul>

                <div class="danger-zone">
                  <button class="btn danger" onclick={remove}><Icon name="trash" size={13} /> {t('settings.removeWorkspace')}</button>
                </div>
              </div>
            {:else}
              <div class="ws-edit muted">{t('settings.noWorkspaces')}</div>
            {/if}
          </div>
        {:else if app.settingsTab === 'skills' || app.settingsTab === 'agents' || app.settingsTab === 'mcp'}
          {#key app.settingsTab}<CapabilitiesPanel kind={app.settingsTab} />{/key}
        {:else if app.settingsTab === 'appearance'}
          <AppearanceSettings />
        {:else if app.settingsTab === 'providers'}
          <ProvidersPanel />
        {:else if app.settingsTab === 'accounts'}
          <!-- textos estáticos das traduções; nenhum dado do usuário entra aqui -->
          <p class="help">
            {@html t('settings.accountsHelp', {
              login: t('settings.accountsHelpLogin', { cmd: '<code>claude</code>' }),
              apiKey: '<code>ANTHROPIC_API_KEY</code>',
            })}
          </p>
          <div class="bin mono">CLI: {app.claudeBinary ?? t('settings.cliNotFound')}</div>
          {#each dirs as d (d.path)}
            {@const users = usedBy(d)}
            <div class="account">
              <div class="acc-head">
                <code>{d.label}</code>
                {#if d.is_default}<span class="badge">{t('settings.default')}</span>{/if}
                {#if users.length}<span class="badge accent">{users.join(', ')}</span>{/if}
              </div>
              {@render authLine(d.is_default ? null : d.path)}
            </div>
          {/each}
        {:else if app.settingsTab === 'defaults' && app.settings}
          <p class="help">{t('settings.defaultsHelp')}</p>
          <div class="grid">
            <label for="lang">{t('settings.language')}</label>
            <select id="lang" class="field" bind:value={app.settings.language} onchange={() => { setLocale(app.settings?.language); saveDefaults() }}>
              {#each LOCALES as l}<option value={l.id}>{l.label}</option>{/each}
            </select>
            <label for="m">{t('settings.model')}</label>
            <select id="m" class="field" bind:value={app.settings.default_model} onchange={saveDefaults}>
              {#each MODELS as m}<option value={m.id}>{m.label}</option>{/each}
            </select>
            <label for="p">{t('settings.permissions')}</label>
            <select id="p" class="field" bind:value={app.settings.default_permission_mode} onchange={saveDefaults}>
              {#each PERMISSION_MODES as m}<option value={m.id}>{m.label}</option>{/each}
            </select>
            <label for="sk">{t('settings.skipPermissions')}</label>
            <label class="ck"><input id="sk" type="checkbox" bind:checked={app.settings.default_skip_permissions} onchange={saveDefaults} />
              {t('settings.skipPermissionsLabel')} <code>--dangerously-skip-permissions</code></label>
            <span></span>
            <p class="hint">{t('settings.skipPermissionsHint')}</p>
            <label for="nt">{t('settings.onFinish')}</label>
            <div class="checks">
              <label class="ck"><input id="nt" type="checkbox" bind:checked={app.settings.notify_on_finish} onchange={saveDefaults} /> {t('settings.systemNotification')}</label>
              <label class="ck"><input type="checkbox" bind:checked={app.settings.sound_on_finish} onchange={saveDefaults} /> {t('settings.sound')}</label>
            </div>
            <span></span>
            <p class="hint">{t('settings.onFinishHint')}</p>
            <label for="s">{t('settings.sendMessage')}</label>
            <select id="s" class="field" bind:value={app.settings.send_with_modifier} onchange={saveDefaults}>
              <option value={false}>Enter</option>
              <option value={true}>Ctrl+Enter</option>
            </select>
          </div>
        {:else if app.settingsTab === 'git' || app.settingsTab === 'integrations'}
          {#key app.settingsTab}<GitSettings section={app.settingsTab} />{/key}
        {:else if app.settingsTab === 'shortcuts'}
          <div class="sc-head">
            <p class="help">{t('shortcuts.settingsHint')}</p>
            <button class="btn ghost" disabled={!Object.keys(getOverrides()).length} onclick={resetAllBindings}>{t('shortcuts.resetAll')}</button>
          </div>
          <ShortcutsList editable />
        {/if}
      </div>
    </div>
  </div>
</div>

{#snippet authLine(dir: string | null)}
  {@const a = app.auth[dir ?? '']}
  <div class="auth">
    {#if !a}
      <span class="muted">{t('settings.checking')}</span>
    {:else if a.error === 'claude_not_found'}
      <span class="bad"><Icon name="alert" size={13} /> {t('settings.cliMissing')}</span>
    {:else if a.loggedIn}
      <span class="ok"><Icon name="check" size={13} /> {t('settings.connected')}</span>
      {#if a.email}<span>{a.email}</span>{/if}
      {#if a.orgName}<span class="muted">· {a.orgName}</span>{/if}
      {#if a.subscriptionType}<span class="muted">· {t('settings.plan', { plan: a.subscriptionType })}</span>{/if}
    {:else}
      {@const run = t('settings.runInTerminal', { cmd: '\u0000' }).split('\u0000')}
      <span class="bad"><Icon name="alert" size={13} /> {t('settings.notLoggedIn')}</span>
      <span class="muted">{run[0]}<code>{dir ? `CLAUDE_CONFIG_DIR=${dir} ` : ''}claude auth login</code>{run[1] ?? ''}</span>
    {/if}
    <button class="btn ghost icon" title={t('settings.checkAgain')} onclick={() => app.refreshAuth(dir)}><Icon name="refresh" size={13} /></button>
  </div>
{/snippet}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.4);
    z-index: 100;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .modal {
    width: min(860px, 94vw);
    height: min(600px, 88vh);
    display: flex;
    flex-direction: column;
    background: var(--bg-elev);
    border: 1px solid var(--border-strong);
    border-radius: 12px;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.45);
    overflow: hidden;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 14px;
    border-bottom: 1px solid var(--border);
  }
  h3 {
    margin: 0;
    font-size: 14px;
  }
  .layout {
    flex: 1;
    display: flex;
    min-height: 0;
  }
  nav {
    width: 170px;
    flex: none;
    padding: 8px;
    border-right: 1px solid var(--border);
    background: var(--bg-panel);
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  nav button,
  .ws-list button {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 8px;
    border: none;
    border-radius: 5px;
    background: none;
    color: var(--text-muted);
    text-align: left;
  }
  nav button:hover,
  .ws-list button:hover {
    background: var(--bg-hover);
  }
  nav button.on,
  .ws-list button.on {
    background: var(--bg-active);
    color: var(--text);
  }
  .content {
    flex: 1;
    min-width: 0;
    overflow-y: auto;
    padding: 16px;
  }
  .ws-layout {
    display: flex;
    gap: 16px;
    min-height: 100%;
  }
  .ws-list {
    width: 180px;
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .ws-list .add {
    color: var(--text-faint);
    margin-top: 6px;
  }
  .wn {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dot {
    background: var(--text-faint);
  }
  .dot.ok {
    background: var(--accent);
  }
  .dot.bad {
    background: var(--red);
  }
  .ws-edit {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .label,
  .grid label {
    font-size: 12px;
    color: var(--text-muted);
    margin-top: 8px;
  }
  .field {
    height: 30px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg-input);
    padding: 0 8px;
    outline: none;
    user-select: text;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .path {
    flex: 1;
    font-size: 12px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    user-select: text;
  }
  .repos {
    list-style: none;
    margin: 0;
    padding: 4px 0;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    max-height: 220px;
    overflow-y: auto;
  }
  .repos li {
    padding: 0 10px;
  }
  .repos label {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 0;
  }
  .off {
    color: var(--text-faint);
    text-decoration: line-through;
  }
  .danger-zone {
    margin-top: auto;
    padding-top: 16px;
  }
  .help {
    color: var(--text-muted);
    margin: 0 0 12px;
  }
  .hint {
    font-size: 12px;
    color: var(--text-faint);
    margin: 0;
  }
  .muted {
    color: var(--text-faint);
  }
  code {
    font-size: 11.5px;
    background: var(--bg-panel);
    padding: 0 4px;
    border-radius: 4px;
  }
  .bin {
    font-size: 11px;
    color: var(--text-faint);
    margin-bottom: 12px;
  }
  .account {
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 10px 12px;
    margin-bottom: 8px;
  }
  .acc-head {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 6px;
  }
  .auth {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
    user-select: text;
    font-size: 12.5px;
  }
  .ok {
    color: var(--accent);
    display: inline-flex;
    gap: 4px;
    align-items: center;
  }
  .bad {
    color: var(--red);
    display: inline-flex;
    gap: 4px;
    align-items: center;
  }
  .grid {
    display: grid;
    grid-template-columns: 150px 1fr;
    gap: 8px 12px;
    align-items: center;
  }
  .grid label {
    margin: 0;
  }
  .checks {
    display: flex;
    gap: 16px;
  }
  .ck {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--text);
    margin: 0;
  }
  .sc-head {
    display: flex;
    align-items: flex-start;
    gap: 12px;
  }
  .sc-head .help {
    flex: 1;
  }
</style>
