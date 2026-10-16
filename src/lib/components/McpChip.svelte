<script lang="ts">
  import { app } from '../stores/app.svelte'
  import { api, errText, type McpServer, type Thread } from '../api'
  import Icon from './Icon.svelte'
  import { t } from '../i18n/index.svelte'

  const STATUS: Record<string, string> = {
    connected: 'capabilities.statusConnected',
    failed: 'capabilities.statusFailed',
    'needs-auth': 'capabilities.statusNeedsAuth',
    pending: 'capabilities.statusPending',
    disabled: 'capabilities.statusDisabled',
  }

  // Chip "MCP" do composer: servidores da sessão desta thread, como o /mcp
  // do Claude Code — ligar/desligar e reconectar valem só para a sessão.
  let { thread, projectPath }: { thread: Thread; projectPath: string | null } = $props()
  let open = $state(false)
  let busy = $state<string | null>(null)

  const live = $derived(app.threadMcp[thread.id] as McpServer[] | undefined)
  const fallback = $derived(app.capabilities[app.capKey(projectPath)]?.mcp)
  const servers = $derived<{ name: string; status: string; tools?: unknown[] }[]>(live ?? fallback ?? [])
  const ok = $derived(servers.filter((s) => s.status === 'connected').length)
  const failing = $derived(servers.some((s) => s.status === 'failed' || s.status === 'needs-auth'))

  async function toggleOpen(e: MouseEvent) {
    e.stopPropagation()
    open = !open
    if (open && !live) app.loadCapabilities(projectPath)
  }

  async function act(action: 'status' | 'toggle' | 'reconnect', s?: { name: string; status: string }) {
    busy = s?.name ?? '*'
    try {
      app.threadMcp[thread.id] = await api.threadMcp(thread.id, action, s?.name, s ? s.status === 'disabled' : undefined)
    } catch (e) {
      app.toast(errText(e), 'error')
    } finally {
      busy = null
    }
  }
</script>

<svelte:window onclick={() => (open = false)} />

<div class="wrap">
  <button class="chip" class:bad={failing} onclick={toggleOpen} title={t('mcpChip.title')}>
    <span class="d" class:ok={ok > 0 && !failing} class:bad={failing}></span> MCP {servers.length ? `${ok}/${servers.length}` : ''}
  </button>
  {#if open}
    <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
    <div class="pop" onclick={(e) => e.stopPropagation()}>
      <div class="head">
        <b>MCP</b>
        <span class="muted">{t(live ? 'mcpChip.threadSession' : 'mcpChip.configured')}</span>
        <button class="btn ghost icon" title={t('mcpChip.refresh')} disabled={!!busy} onclick={() => (live ? act('status') : app.loadCapabilities(projectPath, true))}>
          <Icon name="refresh" size={12} />
        </button>
      </div>
      {#each servers as s (s.name)}
        <div class="row">
          <span class="d st-{s.status}"></span>
          <span class="n">{s.name}</span>
          <span class="muted">{STATUS[s.status] ? t(STATUS[s.status]) : s.status}</span>
          {#if live}
            <button class="btn ghost mini" disabled={!!busy} title={t('mcpChip.reconnect')} onclick={() => act('reconnect', s)}><Icon name="refresh" size={11} /></button>
            <label class="sw" title={t('mcpChip.toggle')}>
              <input type="checkbox" checked={s.status !== 'disabled'} disabled={!!busy} onchange={() => act('toggle', s)} />
            </label>
          {/if}
        </div>
      {:else}
        <div class="muted pad">{t('mcpChip.none')}</div>
      {/each}
      {#if !live}<div class="note">{t('mcpChip.note')}</div>{/if}
      <button class="link" onclick={() => { open = false; app.openSettings('mcp') }}>{t('mcpChip.details')}</button>
    </div>
  {/if}
</div>

<style>
  .wrap {
    position: relative;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 26px;
    padding: 0 8px;
    border: none;
    border-radius: var(--radius-sm);
    background: none;
    color: var(--text-muted);
    font-size: 12px;
  }
  .chip:hover {
    background: var(--bg-hover);
    color: var(--text);
  }
  .d {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--text-faint);
    flex: none;
  }
  .d.ok,
  .st-connected {
    background: var(--accent);
  }
  .d.bad,
  .st-failed {
    background: var(--red);
  }
  .st-needs-auth,
  .st-pending {
    background: var(--amber);
  }
  .pop {
    position: absolute;
    bottom: 32px;
    left: 0;
    width: 320px;
    z-index: 40;
    background: var(--bg-elev);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    padding: 8px;
    box-shadow: 0 12px 30px rgba(0, 0, 0, 0.35);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 6px;
  }
  .head .btn {
    margin-left: auto;
    width: 24px;
    height: 24px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 2px;
  }
  .n {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .muted {
    color: var(--text-faint);
    font-size: 11.5px;
  }
  .mini {
    height: 22px;
    width: 22px;
    padding: 0;
    justify-content: center;
  }
  .note {
    font-size: 11.5px;
    color: var(--text-muted);
    margin: 6px 0;
  }
  .pad {
    padding: 8px 0;
  }
  .link {
    background: none;
    border: none;
    padding: 0;
    margin-top: 4px;
    color: var(--text-muted);
    font-size: 11.5px;
    text-decoration: underline;
  }
</style>
