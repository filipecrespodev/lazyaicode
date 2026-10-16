<script lang="ts">
  import { app } from '../stores/app.svelte'
  import { api, errText, type CapItem, type McpServer } from '../api'
  import Icon from './Icon.svelte'
  import { t } from '../i18n/index.svelte'

  // Páginas Skills, Agentes e MCP das configurações. Como no Delta: escopo
  // Global (conta do workspace) ou um projeto, nome + descrição, itens que
  // não carregaram com o caminho e o erro, abrir e apagar.
  let { kind }: { kind: 'skills' | 'agents' | 'mcp' } = $props()

  let scope = $state<string>('') // '' = global; senão, caminho do projeto
  let q = $state('')
  let showBuiltin = $state(false)
  let openMcp = $state<Record<string, boolean>>({})

  const key = $derived(app.capKey(scope || null))
  const caps = $derived(app.capabilities[key])
  const loading = $derived(!!app.capabilitiesLoading[key])
  const error = $derived(app.capabilitiesError[key])

  $effect(() => {
    void key
    app.loadCapabilities(scope || null)
  })

  const match = (i: { name: string; description?: string }) =>
    !q || i.name.toLowerCase().includes(q.toLowerCase()) || (i.description ?? '').toLowerCase().includes(q.toLowerCase())

  const ORDER = ['project', 'global', 'plugin', 'other', 'builtin']
  const LABEL: Record<string, string> = {
    project: 'capabilities.groupProject',
    global: 'capabilities.groupGlobal',
    plugin: 'capabilities.groupPlugin',
    other: 'capabilities.groupOther',
    builtin: 'capabilities.groupBuiltin',
  }

  const items = $derived.by<CapItem[]>(() => {
    if (!caps) return []
    const list = kind === 'agents' ? caps.agents : caps.commands.filter((c) => showBuiltin || !c.builtin)
    return list.filter(match)
  })
  const groups = $derived(
    ORDER.map((s) => ({ scope: s, items: items.filter((i) => i.scope === s) })).filter((g) => g.items.length),
  )
  const broken = $derived(items.filter((i) => !i.loaded).length)
  const mcp = $derived((caps?.mcp ?? []).filter(match))

  async function trash(i: CapItem) {
    const what = i.kind === 'skill' ? t('capabilities.trashSkill', { name: i.name }) : `"${i.name}"`
    if (!confirm(t('capabilities.trashConfirm', { what }))) return
    try {
      await api.trashItem(app.activeWorkspaceId!, i.path)
      app.toast(t('capabilities.movedToTrash'), 'ok')
      await app.loadCapabilities(scope || null, true)
    } catch (e) {
      app.toast(errText(e), 'error')
    }
  }

  function folderOf(path: string) {
    return path.replace(/\/[^/]+$/, '')
  }

  function addFolder() {
    const base = scope ? `${scope}/.claude` : caps?.claudeDir
    if (base) api.openPath(`${base}/${kind === 'agents' ? 'agents' : 'skills'}`).catch((e) => app.toast(errText(e), 'error'))
  }

  const STATUS: Record<string, string> = {
    connected: 'capabilities.statusConnected',
    failed: 'capabilities.statusFailed',
    'needs-auth': 'capabilities.statusNeedsAuth',
    pending: 'capabilities.statusPending',
    disabled: 'capabilities.statusDisabled',
  }
</script>

<div class="panel">
  <div class="top">
    <select class="field scope" bind:value={scope} title={t('capabilities.scope')}>
      <option value="">{t('capabilities.globalOption', { account: app.accountLabel(app.activeWorkspace?.claude_dir) })}</option>
      {#each app.projects as p (p.id)}<option value={p.path}>{t('capabilities.projectOption', { name: p.name })}</option>{/each}
    </select>
    <input class="field search" placeholder={t('capabilities.filter')} bind:value={q} />
    {#if kind === 'skills'}
      <label class="chk"><input type="checkbox" bind:checked={showBuiltin} /> {t('capabilities.builtins')}</label>
    {/if}
    <button class="btn ghost icon" title={t('capabilities.reload')} disabled={loading} onclick={() => app.loadCapabilities(scope || null, true)}>
      <Icon name="refresh" size={13} />
    </button>
  </div>
  <p class="help">
    {#if kind === 'skills'}
      {@html t(scope ? 'capabilities.skillsHelpProject' : 'capabilities.skillsHelpAccount')}
    {:else if kind === 'agents'}
      {@html t('capabilities.agentsHelp')}
    {:else}
      {t(scope ? 'capabilities.mcpHelpProject' : 'capabilities.mcpHelpAccount')}
    {/if}
  </p>

  {#if error}
    <div class="err">{error}</div>
  {:else if !caps && loading}
    <div class="muted pad">{t('capabilities.loading')}</div>
  {:else if caps && kind !== 'mcp'}
    {#if broken}<div class="warn"><Icon name="alert" size={13} /> {t(broken === 1 ? 'capabilities.brokenOne' : 'capabilities.brokenOther', { n: broken })}</div>{/if}
    {#each groups as g (g.scope)}
      <div class="group">{t(LABEL[g.scope])} <span class="n">{g.items.length}</span></div>
      {#each g.items as i (i.scope + i.name + i.path)}
        <div class="item" class:bad={!i.loaded}>
          <div class="main">
            <div class="name mono">
              {kind === 'agents' ? i.name : '/' + i.name}
              {#if i.argument_hint}<span class="hint">{i.argument_hint}</span>{/if}
              {#if i.model}<span class="badge">{i.model}</span>{/if}
            </div>
            {#if i.description}<div class="desc">{i.description}</div>{/if}
            {#if i.tools}<div class="meta mono">{t('capabilities.tools', { tools: i.tools })}</div>{/if}
            {#if !i.loaded}<div class="error mono">{i.error} — {i.path}</div>{/if}
          </div>
          {#if i.path}
            <div class="actions">
              <button class="btn ghost icon" title={t('capabilities.openFile')} onclick={() => api.openPath(i.path)}><Icon name="external" size={13} /></button>
              <button class="btn ghost icon" title={t('capabilities.openFolder')} onclick={() => api.openPath(folderOf(i.path))}><Icon name="folder" size={13} /></button>
              {#if i.scope === 'global' || i.scope === 'project'}
                <button class="btn ghost icon danger" title={t('capabilities.moveToTrash')} onclick={() => trash(i)}><Icon name="trash" size={13} /></button>
              {/if}
            </div>
          {/if}
        </div>
      {/each}
    {:else}
      <div class="muted pad">{t('capabilities.nothingFound')}</div>
    {/each}
    <button class="btn add" onclick={addFolder}><Icon name="folder" size={13} /> {t(kind === 'agents' ? (scope ? 'capabilities.openAgentsProject' : 'capabilities.openAgentsAccount') : scope ? 'capabilities.openSkillsProject' : 'capabilities.openSkillsAccount')}</button>
  {:else if caps}
    {#each mcp as m (m.name)}
      <div class="item mcp">
        <div class="main">
          <div class="name">
            <span class="st st-{m.status}"></span>
            <b>{m.name}</b>
            <span class="badge">{STATUS[m.status] ? t(STATUS[m.status]) : m.status}</span>
            {#if m.scope}<span class="badge">{m.scope}</span>{/if}
          </div>
          <div class="meta mono">
            {m.transport ?? ''}{m.url ? ` · ${m.url}` : ''}{m.command ? ` · ${m.command}` : ''}
            {#if m.serverInfo?.name}· {m.serverInfo.name} {m.serverInfo.version ?? ''}{/if}
          </div>
          {#if m.error}<div class="error mono">{m.error}</div>{/if}
          {#if m.tools.length}
            <button class="tools-toggle" onclick={() => (openMcp[m.name] = !openMcp[m.name])}>
              {t(`capabilities.${openMcp[m.name] ? 'hide' : 'show'}Tools${m.tools.length === 1 ? 'One' : 'Other'}`, { n: m.tools.length })}
            </button>
            {#if openMcp[m.name]}
              <ul class="tools">
                {#each m.tools as tool (tool.name)}<li><code>{tool.name}</code> {#if tool.description}<span>{tool.description}</span>{/if}</li>{/each}
              </ul>
            {/if}
          {/if}
        </div>
      </div>
    {:else}
      <div class="muted pad">{t(scope ? 'capabilities.noMcpProject' : 'capabilities.noMcpAccount')}</div>
    {/each}
  {/if}
  {#if caps}<div class="foot muted">Claude Code {caps.version} · {app.accountLabel(caps.claudeDir)}</div>{/if}
</div>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .top {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .field {
    height: 30px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg-input);
    padding: 0 8px;
    outline: none;
  }
  .scope {
    max-width: 280px;
  }
  .search {
    flex: 1;
    min-width: 80px;
    user-select: text;
  }
  .chk {
    display: inline-flex;
    gap: 4px;
    align-items: center;
    font-size: 12px;
    color: var(--text-muted);
    white-space: nowrap;
  }
  .help {
    color: var(--text-muted);
    font-size: 12px;
    margin: 4px 0 6px;
  }
  code {
    font-size: 11px;
    background: var(--bg-panel);
    padding: 0 4px;
    border-radius: 4px;
  }
  .group {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.6px;
    color: var(--text-faint);
    margin: 10px 0 2px;
  }
  .n {
    color: var(--text-faint);
    margin-left: 4px;
  }
  .item {
    display: flex;
    gap: 8px;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
  }
  .item.bad {
    border-color: color-mix(in srgb, var(--red) 40%, var(--border));
  }
  .main {
    flex: 1;
    min-width: 0;
  }
  .name {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12.5px;
  }
  .hint {
    color: var(--text-faint);
    font-size: 11px;
  }
  .desc {
    color: var(--text-muted);
    font-size: 12px;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    margin-top: 2px;
  }
  .meta {
    color: var(--text-faint);
    font-size: 11px;
    margin-top: 2px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .error {
    color: var(--red);
    font-size: 11px;
    margin-top: 3px;
    user-select: text;
    word-break: break-all;
  }
  .actions {
    display: flex;
    align-items: flex-start;
    gap: 2px;
  }
  .actions .btn {
    width: 24px;
    height: 24px;
  }
  .warn {
    color: var(--amber);
    font-size: 12px;
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .err {
    color: var(--red);
    user-select: text;
  }
  .muted {
    color: var(--text-faint);
  }
  .pad {
    padding: 20px 0;
  }
  .add {
    align-self: flex-start;
    margin-top: 10px;
  }
  .st {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--text-faint);
  }
  .st-connected {
    background: var(--accent);
  }
  .st-failed {
    background: var(--red);
  }
  .st-needs-auth,
  .st-pending {
    background: var(--amber);
  }
  .tools-toggle {
    background: none;
    border: none;
    padding: 0;
    margin-top: 4px;
    font-size: 11.5px;
    color: var(--text-muted);
    text-decoration: underline;
  }
  .tools {
    list-style: none;
    margin: 6px 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 11.5px;
    color: var(--text-muted);
  }
  .foot {
    font-size: 11px;
    margin-top: 10px;
  }
</style>
