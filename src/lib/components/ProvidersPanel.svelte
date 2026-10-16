<script lang="ts">
  import { onMount } from 'svelte'
  import { app } from '../stores/app.svelte'
  import { api, errText, PROVIDERS, type ProviderId, type ProviderStatus } from '../api'
  import Icon from './Icon.svelte'
  import { t } from '../i18n/index.svelte'

  // OpenAI (Codex) e Google (Gemini) rodam pelos próprios agentes oficiais,
  // com o login que já existe na máquina — como o Claude Code.
  type Method = { id: string; name: string; description?: string; type?: string; vars?: { name: string }[] }
  const ACP: ProviderId[] = ['codex', 'gemini']
  const INSTALL: Record<string, { pkg: string; login: string; env: string; siteKey: string }> = {
    codex: { pkg: '@openai/codex', login: 'codex login', env: 'OPENAI_API_KEY', siteKey: 'providers.codexSite' },
    gemini: { pkg: '@google/gemini-cli', login: 'gemini', env: 'GEMINI_API_KEY', siteKey: 'providers.geminiSite' },
  }

  let status = $state<Record<string, ProviderStatus | null>>({})
  let checking = $state<Record<string, boolean>>({})
  let methods = $state<Record<string, Method[] | null>>({})
  let logging = $state<string | null>(null)

  async function check(p: ProviderId, force = false) {
    checking[p] = true
    try {
      status[p] = await api.providerStatus(p, force)
    } catch (e) {
      status[p] = { provider: p, ok: false, available: false, error: errText(e) }
    } finally {
      checking[p] = false
    }
  }

  async function loadMethods(p: ProviderId) {
    methods[p] = null
    try {
      methods[p] = (await api.providerAuthMethods(p)).methods ?? []
    } catch (e) {
      app.toast(errText(e), 'error')
    }
  }

  async function login(p: ProviderId, m: Method) {
    logging = `${p}:${m.id}`
    app.toast(t('providers.finishLogin'), 'info')
    try {
      await api.providerLogin(p, m.id)
      app.toast(t('providers.loginDone'), 'ok')
      await check(p, true)
    } catch (e) {
      app.toast(errText(e), 'error')
    } finally {
      logging = null
    }
  }

  async function saveDefault(v: ProviderId) {
    if (!app.settings) return
    app.settings.default_provider = v
    await api.saveSettings(app.settings)
  }

  onMount(() => {
    for (const p of ACP) check(p)
  })
</script>

<p class="help">{@html t('providers.help')}</p>

<div class="def">
  <span>{t('providers.defaultProvider')}</span>
  <select class="field" value={app.settings?.default_provider ?? 'claude'} onchange={(e) => saveDefault(e.currentTarget.value as ProviderId)}>
    {#each PROVIDERS as p (p.id)}<option value={p.id}>{p.vendor} · {p.label}</option>{/each}
  </select>
</div>

{#each ACP as p (p)}
  {@const meta = PROVIDERS.find((x) => x.id === p)!}
  {@const st = status[p]}
  {@const inst = INSTALL[p]}
  <section class="card">
    <header>
      <span class="logo prov-{p}">{meta.short}</span>
      <div class="ttl">
        <b>{meta.vendor}</b> · {meta.label}
        {#if st?.agent?.version}<span class="muted mono">v{st.agent.version}</span>{/if}
      </div>
      {#if checking[p]}
        <span class="badge">{t('providers.checking')}</span>
      {:else if st?.ok}
        <span class="badge accent"><Icon name="check" size={11} /> {t('providers.connected')}</span>
      {:else if st && !st.available}
        <span class="badge bad">{t('providers.unavailable')}</span>
      {:else if st}
        <span class="badge warn">{t('providers.needsLogin')}</span>
      {/if}
      <button class="btn ghost icon" title={t('providers.checkAgain')} disabled={checking[p]} onclick={() => check(p, true)}><Icon name="refresh" size={13} /></button>
    </header>

    {#if checking[p] && !st}
      <p class="muted">{t('providers.opening')}</p>
    {/if}
    {#if st?.launch}
      <div class="row muted mono">
        {t(st.launch.how === 'installed' ? 'providers.installed' : st.launch.how === 'env' ? 'providers.fromEnv' : 'providers.viaNpx')}
        {st.launch.how === 'npx' ? st.launch.args.filter((a) => a !== '-y').join(' ') : st.launch.bin}
      </div>
    {/if}
    {#if st && !st.ok && st.error}<div class="err mono">{st.error}</div>{/if}

    {#if st && !st.ok}
      <div class="how">
        <div>{t('providers.loginWith', { site: t(inst.siteKey) })}</div>
        {#if methods[p] === undefined}
          <button class="btn" onclick={() => loadMethods(p)}><Icon name="key" size={13} /> {t('providers.showLoginMethods')}</button>
        {:else if methods[p] === null}
          <span class="muted">{t('providers.loading')}</span>
        {:else}
          <div class="methods">
            {#each methods[p] ?? [] as m (m.id)}
              {#if m.type === 'env_var'}
                {@const ev = t('providers.envVar', { vars: '\u0000' }).split('\u0000')}
                <div class="muted">{ev[0]}<code>{m.vars?.map((v) => v.name).join(' / ')}</code>{ev[1] ?? ''}</div>
              {:else}
                <button class="btn primary" disabled={!!logging} onclick={() => login(p, m)} title={m.description ?? ''}>
                  {logging === `${p}:${m.id}` ? t('providers.waitingBrowser') : m.name}
                </button>
              {/if}
            {/each}
          </div>
        {/if}
        <!-- pacotes e comandos são constantes do INSTALL acima -->
        <div class="muted small">
          {@html t('providers.terminalHint', {
            npx: `<code>npx ${inst.pkg}</code>`,
            install: `<code>npm i -g ${inst.pkg}</code>`,
            login: `<code>${inst.login}</code>`,
            env: `<code>${inst.env}</code>`,
          })}
        </div>
      </div>
    {/if}
    {#if st?.ok && st.acp?.models?.length}
      <div class="row muted">{t('providers.models', { models: st.acp.models.map((m) => m.name).join(', ') })}</div>
    {/if}
  </section>
{/each}

<style>
  .help {
    color: var(--text-muted);
    margin: 0 0 12px;
  }
  .def {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 14px;
    color: var(--text-muted);
  }
  .field {
    height: 30px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg-input);
    padding: 0 8px;
  }
  .card {
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 10px 12px;
    margin-bottom: 10px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  header {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .logo {
    width: 26px;
    height: 26px;
    border-radius: 7px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-weight: 700;
    border: 1px solid;
  }
  .ttl {
    flex: 1;
    display: flex;
    gap: 8px;
    align-items: baseline;
  }
  .badge.bad {
    color: var(--red);
  }
  .badge.warn {
    color: var(--amber);
  }
  .row {
    font-size: 11.5px;
  }
  .muted {
    color: var(--text-faint);
  }
  .small {
    font-size: 11.5px;
  }
  .err {
    color: var(--red);
    font-size: 11.5px;
    user-select: text;
    white-space: pre-wrap;
  }
  .how {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 8px 10px;
    background: var(--bg-panel);
    border-radius: var(--radius-sm);
  }
  .methods {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    align-items: center;
  }
  code {
    font-size: 11px;
    background: var(--bg-input);
    padding: 0 4px;
    border-radius: 4px;
  }
</style>
