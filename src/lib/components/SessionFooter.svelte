<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog'
  import { app } from '../stores/app.svelte'
  import { api, errText, EFFORTS, MODELS, PERMISSION_MODES, PROVIDERS, type ProviderId, type Thread } from '../api'
  import { fmtTokens, knownProviderModels } from '../session'
  import Icon from './Icon.svelte'
  import Menu from './Menu.svelte'
  import McpChip from './McpChip.svelte'
  import { intlLocale, t } from '../i18n/index.svelte'

  // Rodapé da sessão, como no Delta: à esquerda onde a sessão trabalha
  // (projeto / branch, diff, pastas extras, agente); à direita como ela roda
  // (contexto, permissões, esforço, modelo) e o enviar.
  let {
    thread,
    projectPath,
    busy,
    canSend,
    onSend,
    onStop,
  }: {
    thread: Thread
    projectPath: string | null
    busy: boolean
    canSend: boolean
    onSend: () => void
    onStop: () => void
  } = $props()

  let dirsOpen = $state(false)

  const project = $derived(app.projects.find((p) => p.id === thread.project_id))
  const caps = $derived(app.capabilities[app.capKey(projectPath)])
  const stats = $derived(app.diffStats[thread.id])
  const ctx = $derived(app.context[thread.id])
  const pct = $derived(ctx ? Math.min(1, ctx.used / ctx.window) : 0)

  $effect(() => {
    app.loadCapabilities(projectPath)
  })

  const isClaude = $derived(thread.provider === 'claude')
  const acp = $derived(app.acpSession[thread.id])
  const vendorOf = (p: ProviderId) => PROVIDERS.find((x) => x.id === p)!

  // Um seletor só para os três provedores, como no Delta. O valor é
  // "provedor::modelo"; modelo vazio = padrão do agente.
  const modelOptions = $derived.by(() => {
    const claude = caps?.models?.length
      ? caps.models.map((m) => ({ id: m.value, name: m.displayName }))
      : MODELS.map((m) => ({ id: m.id, name: m.label }))
    const out: { value: string; label: string; hint?: string; group: string; icon: string }[] = claude.map((m) => ({
      value: `claude::${m.id}`,
      label: m.name,
      group: 'Anthropic · Claude Code',
      icon: 'sparkle',
    }))
    for (const p of ['codex', 'gemini'] as ProviderId[]) {
      const v = vendorOf(p)
      const models = thread.provider === p && acp?.models?.length ? acp.models : knownProviderModels(p)
      out.push({ value: `${p}::`, label: t('footer.defaultOf', { name: v.label }), hint: models.length ? undefined : t('footer.modelsAfterFirst'), group: `${v.vendor} · ${v.label}`, icon: 'sparkle' })
      for (const m of models) out.push({ value: `${p}::${m.id}`, label: m.name || m.id, group: `${v.vendor} · ${v.label}`, icon: 'sparkle' })
    }
    return out
  })
  const modelValue = $derived(`${thread.provider}::${isClaude ? thread.model : thread.model || ''}`)
  const modelLabel = $derived.by(() => {
    if (!isClaude) {
      const v = vendorOf(thread.provider)
      const id = thread.model || acp?.currentModel || ''
      const name = acp?.models?.find((m) => m.id === id)?.name ?? knownProviderModels(thread.provider).find((m) => m.id === id)?.name
      return name ?? (thread.model ? thread.model : v.label)
    }
    return thread.model === 'default'
      ? (caps?.models?.find((m) => m.value === 'default')?.description?.split('·')[0]?.trim() ?? t('footer.modelDefault'))
      : (caps?.models?.find((m) => m.value === thread.model)?.displayName ?? MODELS.find((m) => m.id === thread.model)?.label ?? thread.model)
  })

  // `--dangerously-skip-permissions` aparece como o último "modo": é o mais
  // forte, e escolher qualquer outro modo o desliga
  const SKIP = '__skip'
  const modeOptions = $derived(
    isClaude
      ? [
          ...PERMISSION_MODES.map((m) => ({ value: m.id, label: m.label, group: t('footer.groupMode') })),
          { value: SKIP, label: t('footer.skipAll'), hint: '--dangerously-skip-permissions', group: t('footer.groupNoConfirm'), icon: 'alert' },
        ]
      : [{ value: '', label: t('footer.agentDefaultMode') }, ...(acp?.modes ?? []).map((m) => ({ value: m.id, label: m.name, hint: m.description ?? undefined }))],
  )
  const modeValue = $derived(isClaude && thread.skip_permissions ? SKIP : thread.permission_mode)
  const modeLabel = $derived(
    isClaude
      ? thread.skip_permissions
        ? t('footer.noPermissions')
        : thread.permission_mode !== 'default' ? PERMISSION_MODES.find((m) => m.id === thread.permission_mode)?.label : ''
      : (acp?.modes?.find((m) => m.id === (thread.permission_mode || acp?.currentMode))?.name ?? ''),
  )

  const agentOptions = $derived([
    { value: '', label: t('footer.defaultAgent'), group: '' },
    ...(caps?.agents ?? [])
      .filter((a) => a.loaded)
      .map((a) => ({ value: a.name, label: a.name, hint: a.model ?? undefined, group: a.builtin ? t('footer.groupBuiltin') : a.scope === 'plugin' ? t('footer.groupPlugins') : a.scope === 'project' ? t('footer.groupProject') : t('footer.groupAccount') })),
  ])

  async function setOpt(opts: { effort?: string | null; agent?: string | null; addDirs?: string[] }) {
    try {
      app.upsert(await api.setThreadOptions(thread.id, opts))
    } catch (e) {
      app.toast(errText(e), 'error')
    }
  }

  async function pickModel(value: string) {
    const [provider, ...rest] = value.split('::') as [ProviderId, ...string[]]
    const model = rest.join('::')
    try {
      if (provider === thread.provider) {
        app.upsert(await api.setThreadModel(thread.id, model))
        return
      }
      const fresh = !thread.session_id && !(app.transcripts[thread.id]?.items.length ?? 0)
      if (fresh) {
        app.upsert(await api.setThreadProvider(thread.id, provider, model || undefined))
      } else {
        // a conversa pertence a um provedor: outro provedor = thread nova
        await app.newThread(thread.project_id, thread.isolated, provider, model || undefined)
        app.toast(t('footer.newThreadWith', { name: vendorOf(provider).label }), 'info')
      }
    } catch (e) {
      app.toast(errText(e), 'error')
    }
  }

  async function setMode(mode: string) {
    try {
      if (mode === SKIP) {
        // vale a partir da próxima mensagem: a sessão reabre com a flag
        app.upsert(await api.setThreadOptions(thread.id, { skipPermissions: true }))
        if (thread.session_id) app.toast(t('footer.skipNextMessage'), 'info')
        return
      }
      if (thread.skip_permissions) app.upsert(await api.setThreadOptions(thread.id, { skipPermissions: false }))
      app.upsert(await api.setThreadPermissionMode(thread.id, mode))
    } catch (e) {
      app.toast(errText(e), 'error')
    }
  }

  async function addDir() {
    dirsOpen = false
    const dir = await open({ directory: true, multiple: false, title: t('footer.addDirDialog') })
    if (typeof dir === 'string' && !thread.add_dirs.includes(dir)) await setOpt({ addDirs: [...thread.add_dirs, dir] })
  }

  const n = (x: number) => x.toLocaleString(intlLocale())
  const short = (p: string) => p.replace(/^\/home\/[^/]+/, '~').replace(/^\/Users\/[^/]+/, '~')
</script>

<svelte:window onclick={() => (dirsOpen = false)} />

<footer class="foot">
  <div class="left">
    <button class="seg strong" title={thread.cwd} onclick={() => api.openPath(thread.cwd)}>
      <Icon name="layers" size={13} /> {project?.name ?? '—'}
    </button>
    <span class="sep">/</span>
    <span class="seg branch mono" title="Branch">{stats?.branch ?? thread.branch ?? ''}</span>
    {#if stats && (stats.additions || stats.deletions)}
      <button class="seg diff" title={t(stats.files === 1 ? 'footer.diffTitleOne' : 'footer.diffTitleOther', { n: stats.files })} onclick={() => app.showChanges()}>
        <span class="plus">+{n(stats.additions)}</span> <span class="minus">−{n(stats.deletions)}</span>
      </button>
    {/if}

    {#if isClaude}
    <div class="dirs">
      <button class="seg icon" class:on={thread.add_dirs.length > 0} title={t('footer.extraDirs')}
        onclick={(e) => { e.stopPropagation(); dirsOpen = !dirsOpen }}>
        <Icon name="folder" size={14} />{#if thread.add_dirs.length}<span class="count">+{thread.add_dirs.length}</span>{/if}
      </button>
      {#if dirsOpen}
        <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
        <div class="pop" onclick={(e) => e.stopPropagation()}>
          <div class="ptitle">{t('footer.dirsTitle')}</div>
          <div class="dir main mono"><Icon name="folder" size={12} /> {short(thread.cwd)}</div>
          {#each thread.add_dirs as d (d)}
            <div class="dir mono">
              <Icon name="folder" size={12} /> <span>{short(d)}</span>
              <button class="x" title={t('footer.remove')} onclick={() => setOpt({ addDirs: thread.add_dirs.filter((x) => x !== d) })}><Icon name="x" size={12} /></button>
            </div>
          {/each}
          <button class="btn add" onclick={addDir}><Icon name="plus" size={12} /> {t('footer.addDir')}</button>
          <div class="note">{t('footer.restartNote')}</div>
        </div>
      {/if}
    </div>

    <Menu options={agentOptions} value={thread.agent ?? ''} onSelect={(v) => setOpt({ agent: v || null })} searchable placeholder={t('footer.searchAgent')} title={t('footer.agentTitle')} width={300}>
      {#snippet trigger()}
        <Icon name="user" size={13} /> {thread.agent ?? t('footer.agent')}
      {/snippet}
    </Menu>
    {:else}
      <span class="seg prov" title={t('footer.agentVendor', { vendor: vendorOf(thread.provider).vendor })}>{vendorOf(thread.provider).label}</span>
    {/if}
  </div>

  <div class="right">
    {#if ctx}
      <span class="ring" title={t('footer.contextTitle', { used: fmtTokens(ctx.used), window: fmtTokens(ctx.window), pct: Math.round(pct * 100) })}>
        <svg width="16" height="16" viewBox="0 0 16 16">
          <circle cx="8" cy="8" r="6" fill="none" stroke="var(--border-strong)" stroke-width="2" />
          <circle cx="8" cy="8" r="6" fill="none" stroke={pct > 0.85 ? 'var(--red)' : pct > 0.6 ? 'var(--amber)' : 'var(--text-muted)'}
            stroke-width="2" stroke-dasharray="{(pct * 37.7).toFixed(1)} 37.7" transform="rotate(-90 8 8)" stroke-linecap="round" />
        </svg>
      </span>
    {/if}

    {#if isClaude}<McpChip {thread} {projectPath} />{/if}

    <span class="perm" class:danger={isClaude && thread.skip_permissions}>
      <Menu options={modeOptions} value={modeValue} onSelect={setMode} title={isClaude ? t('footer.permissions') : t('footer.agentMode')} width={340} align="right">
        {#snippet trigger()}
          <Icon name={isClaude && thread.skip_permissions ? 'alert' : 'shield'} size={13} /> {modeLabel}
        {/snippet}
      </Menu>
    </span>

    {#if isClaude}
      <Menu options={EFFORTS.map((e) => ({ value: e.id, label: e.label }))} value={thread.effort ?? ''} onSelect={(v) => setOpt({ effort: v || null })} title={t('footer.effort')} width={160} align="right">
        {#snippet trigger()}
          <Icon name="brain" size={13} /> {EFFORTS.find((e) => e.id === (thread.effort ?? ''))?.label}
        {/snippet}
      </Menu>
    {/if}

    <Menu options={modelOptions} value={modelValue} onSelect={pickModel} searchable placeholder={t('footer.selectModel')} title={t('footer.modelProvider')} width={340} align="right">
      {#snippet trigger()}
        <Icon name="sparkle" size={13} /> {modelLabel}
      {/snippet}
      {#snippet footer(close)}
        <button class="btn ghost wide" onclick={() => { close(); app.openSettings('defaults') }}><Icon name="sliders" size={13} /> {t('footer.defaultModelNew')}</button>
      {/snippet}
    </Menu>

    {#if busy}
      <button class="send stop" onclick={onStop} title={t('footer.stop')}><Icon name="stop" size={12} /></button>
    {:else}
      <button class="send" disabled={!canSend} onclick={onSend} title={t('footer.send')}><Icon name="send" size={14} /></button>
    {/if}
  </div>
</footer>

<style>
  .foot {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 40px;
    padding: 0 10px 0 14px;
    border-top: 1px solid var(--border);
    background: var(--bg);
    flex: none;
  }
  .left,
  .right {
    display: flex;
    align-items: center;
    gap: 2px;
    min-width: 0;
  }
  .left {
    flex: 1;
    overflow: hidden;
  }
  .seg {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 26px;
    padding: 0 6px;
    border: none;
    border-radius: 6px;
    background: none;
    color: var(--text-muted);
    font-size: 12.5px;
    white-space: nowrap;
  }
  button.seg:hover {
    background: var(--bg-hover);
    color: var(--text);
  }
  .seg.strong {
    color: var(--text);
  }
  .seg.on {
    color: var(--accent);
  }
  .branch {
    font-size: 12px;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    padding: 0 2px;
  }
  .sep {
    color: var(--text-faint);
  }
  .diff {
    font-family: var(--mono);
    font-size: 12px;
  }
  .plus {
    color: var(--add-fg);
  }
  .minus {
    color: var(--del-fg);
  }
  .count {
    font-size: 11px;
  }
  .dirs {
    position: relative;
  }
  .pop {
    position: absolute;
    bottom: 32px;
    left: 0;
    width: 340px;
    z-index: 50;
    background: var(--bg-elev);
    border: 1px solid var(--border-strong);
    border-radius: 10px;
    padding: 8px;
    box-shadow: 0 14px 40px rgba(0, 0, 0, 0.4);
  }
  .ptitle {
    font-size: 11.5px;
    color: var(--text-faint);
    margin-bottom: 6px;
  }
  .dir {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    padding: 3px 2px;
    color: var(--text-muted);
  }
  .dir span {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dir.main {
    color: var(--text);
  }
  .x {
    border: none;
    background: none;
    color: var(--text-faint);
    display: inline-flex;
    padding: 2px;
  }
  .x:hover {
    color: var(--red);
  }
  .add {
    margin-top: 6px;
    height: 26px;
  }
  .note {
    margin-top: 6px;
    font-size: 11px;
    color: var(--text-faint);
  }
  .perm.danger :global(.trigger) {
    color: var(--red);
    background: color-mix(in srgb, var(--red) 10%, transparent);
  }
  .prov {
    color: var(--text-muted);
    font-size: 12px;
  }
  .ring {
    display: inline-flex;
    padding: 0 6px;
  }
  .wide {
    width: 100%;
    justify-content: flex-start;
    color: var(--text-muted);
  }
  .send {
    width: 28px;
    height: 28px;
    margin-left: 4px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 1px solid var(--border-strong);
    border-radius: 7px;
    background: var(--bg-elev);
    color: var(--blue);
  }
  .send:hover:not(:disabled) {
    background: var(--bg-hover);
  }
  .send:disabled {
    color: var(--text-faint);
    cursor: default;
  }
  .send.stop {
    color: var(--text);
  }
</style>
