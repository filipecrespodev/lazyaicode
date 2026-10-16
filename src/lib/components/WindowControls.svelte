<script lang="ts">
  import { onMount } from 'svelte'
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import Icon from './Icon.svelte'
  import { t } from '../i18n/index.svelte'

  // Minimizar / maximizar / fechar da janela sem decoração nativa
  // (a barra do GNOME está desligada em tauri.conf.json).
  let maximized = $state(false)
  const win = (() => {
    try {
      return getCurrentWindow()
    } catch {
      return null
    }
  })()

  onMount(() => {
    if (!win) return
    const sync = () => win.isMaximized().then((m) => (maximized = m)).catch(() => {})
    sync()
    const off = win.onResized(sync)
    return () => {
      off.then((f) => f())
    }
  })
</script>

<div class="wc">
  <button title={t('windowControls.minimize')} onclick={() => win?.minimize()}><Icon name="minus" size={14} /></button>
  <button title={t(maximized ? 'windowControls.restore' : 'windowControls.maximize')} onclick={() => win?.toggleMaximize()}>
    <Icon name={maximized ? 'restore' : 'square'} size={12} />
  </button>
  <button class="close" title={t('windowControls.close')} onclick={() => win?.close()}><Icon name="x" size={15} /></button>
</div>

<style>
  .wc {
    display: flex;
    align-self: stretch;
  }
  button {
    width: 44px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: none;
    color: var(--text-muted);
  }
  button:hover {
    background: var(--bg-hover);
    color: var(--text);
  }
  .close:hover {
    background: #d63c3c;
    color: #fff;
  }
</style>
