<script lang="ts">
  // Navega por uma distro WSL pelo agente que roda dentro dela (o diálogo
  // nativo varre \\wsl.localhost pelo 9P, lento) e devolve a pasta escolhida
  // como caminho UNC (`\\wsl.localhost\<distro>\...`).
  import { onMount } from 'svelte'
  import { api, errText, type WslDirEntry } from '../api'
  import { wslParent } from '../wsl'
  import Icon from './Icon.svelte'
  import { t } from '../i18n/index.svelte'

  let { onpick, onclose }: { onpick: (path: string) => void; onclose: () => void } = $props()

  let distros = $state<string[] | null>(null)
  let current = $state<string | null>(null)
  let dirs = $state<WslDirEntry[]>([])
  let busy = $state<string | null>(null)
  let error = $state<string | null>(null)

  // erros do cliente git chegam como { kind, details }
  function message(e: unknown): string {
    const d = (e as { details?: unknown })?.details
    if (typeof d === 'string') return d
    if (d && typeof d === 'object' && 'message' in d) return String((d as { message: unknown }).message)
    return errText(e)
  }

  onMount(async () => {
    try {
      distros = await api.listWslDistros()
    } catch (e) {
      distros = []
      error = message(e)
    }
  })

  async function go(path: string) {
    error = null
    busy = t('wsl.loading')
    try {
      dirs = await api.listWslDirs(path)
      current = path
    } catch (e) {
      error = message(e)
    } finally {
      busy = null
    }
  }

  async function openDistro(distro: string) {
    error = null
    // a primeira chamada sobe a distro e o agente (pode levar alguns segundos)
    busy = t('wsl.starting', { distro })
    try {
      const home = await api.wslHome(distro)
      busy = null
      await go(home)
    } catch (e) {
      error = message(e)
    } finally {
      busy = null
    }
  }

  const parent = $derived(current ? wslParent(current) : null)
</script>

<div class="overlay" role="presentation" onclick={onclose}>
  <div class="modal" role="dialog" tabindex="-1" onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.key === 'Escape' && (e.stopPropagation(), onclose())}>
    <header>
      <h3>{t('wsl.title')}</h3>
      <button class="btn ghost icon" title={t('wsl.cancel')} onclick={onclose}><Icon name="x" /></button>
    </header>
    <div class="body">
      {#if !current}
        <span class="label">{t('wsl.distros')}</span>
        {#if distros === null}
          <p class="hint">{t('wsl.loading')}</p>
        {:else if distros.length === 0}
          <p class="hint">{t('wsl.noDistros')}</p>
        {/if}
        <div class="list">
          {#each distros ?? [] as d (d)}
            <button class="item" disabled={!!busy} onclick={() => openDistro(d)}>
              <Icon name="terminal" size={14} /> {d}
            </button>
          {/each}
        </div>
      {:else}
        <div class="path mono" title={current}>{current}</div>
        <div class="list">
          {#if parent}
            <button class="item" disabled={!!busy} title={t('wsl.up')} onclick={() => go(parent)}>
              <span class="flip"><Icon name="down" size={14} /></span> ..
            </button>
          {:else}
            <button class="item" disabled={!!busy} onclick={() => ((current = null), (dirs = []))}>
              <span class="flip"><Icon name="chevron" size={14} /></span> {t('wsl.back')}
            </button>
          {/if}
          {#each dirs as d (d.path)}
            <button class="item" disabled={!!busy} onclick={() => go(d.path)}>
              {#if d.isRepo}
                <span class="repo" title={t('wsl.repo')}><Icon name="branch" size={14} /></span>
              {:else}
                <Icon name="folder" size={14} />
              {/if}
              {d.name}
            </button>
          {/each}
          {#if !busy && dirs.length === 0}
            <p class="hint">{t('wsl.empty')}</p>
          {/if}
        </div>
      {/if}
      {#if busy}<p class="hint">{busy}</p>{/if}
      {#if error}<p class="error">{error}</p>{/if}
    </div>
    <footer>
      <button class="btn" onclick={onclose}>{t('wsl.cancel')}</button>
      <button class="btn primary" disabled={!current || !!busy} onclick={() => current && onpick(current)}>
        {t('wsl.select')}
      </button>
    </footer>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.4);
    z-index: 120;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .modal {
    width: min(520px, 94vw);
    background: var(--bg-elev);
    border: 1px solid var(--border-strong);
    border-radius: 12px;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.45);
  }
  header,
  footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 16px;
  }
  header {
    border-bottom: 1px solid var(--border);
  }
  footer {
    border-top: 1px solid var(--border);
    justify-content: flex-end;
    gap: 8px;
  }
  h3 {
    margin: 0;
    font-size: 15px;
  }
  .body {
    padding: 12px 16px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .label {
    color: var(--text-muted);
    font-size: 12px;
  }
  .path {
    color: var(--text-muted);
    font-size: 12px;
    word-break: break-all;
    user-select: text;
  }
  .list {
    max-height: 300px;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    text-align: left;
    padding: 5px 8px;
    border-radius: var(--radius-sm);
    background: transparent;
    border: 0;
    color: inherit;
    cursor: pointer;
  }
  .item:hover:not(:disabled) {
    background: var(--bg-hover);
  }
  .item:disabled {
    opacity: 0.6;
    cursor: default;
  }
  .repo {
    display: inline-flex;
    color: var(--accent);
  }
  .flip {
    display: inline-flex;
    transform: rotate(180deg);
  }
  .hint {
    color: var(--text-muted);
    font-size: 12px;
    margin: 4px 0;
  }
  .error {
    color: var(--red);
    font-size: 12px;
    margin: 4px 0;
    word-break: break-word;
  }
</style>
