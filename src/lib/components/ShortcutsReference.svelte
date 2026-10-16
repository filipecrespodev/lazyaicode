<script lang="ts">
  import { app } from '../stores/app.svelte'
  import { t } from '../i18n/index.svelte'
  import ShortcutsList from './ShortcutsList.svelte'

  // Lista de atalhos aberta pelo showShortcuts (Ctrl+/ por padrão).
  let { onclose }: { onclose: () => void } = $props()

  function edit() {
    onclose()
    app.openSettings('shortcuts')
  }
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onclose()} />

<div class="overlay" role="presentation" onclick={onclose}>
  <div class="box" role="dialog" aria-label={t('shortcuts.title')} tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
    <h3>{t('shortcuts.title')}</h3>
    <ShortcutsList editable={false} />
    <p class="hint">
      {t('shortcuts.changeIn')} <button class="link" onclick={edit}>{t('shortcuts.openSettingsLink')}</button>
    </p>
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
    align-items: flex-start;
    padding-top: 10vh;
  }
  .box {
    width: min(520px, 92vw);
    max-height: 75vh;
    overflow-y: auto;
    padding: 16px;
    background: var(--bg-elev);
    border: 1px solid var(--border-strong);
    border-radius: 12px;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.45);
  }
  h3 {
    margin: 0 0 12px;
    font-size: 14px;
  }
  .hint {
    margin: 12px 0 0;
    font-size: 12px;
    color: var(--text-faint);
  }
  .link {
    border: none;
    background: none;
    padding: 0;
    font-size: 12px;
    color: var(--accent);
  }
  .link:hover {
    text-decoration: underline;
  }
</style>
