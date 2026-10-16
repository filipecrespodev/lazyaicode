<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog'
  import { app } from '../stores/app.svelte'
  import Icon from './Icon.svelte'
  import AccountSelect from './AccountSelect.svelte'
  import WslFolderPicker from './WslFolderPicker.svelte'
  import { t } from '../i18n/index.svelte'
  import { IS_WINDOWS, baseName } from '../wsl'

  let path = $state('')
  let name = $state('')
  // sugere a conta do ambiente; `null` = ~/.claude
  let account = $state<string | null>(app.suggestedClaudeDir === app.defaultClaudeDir ? null : app.suggestedClaudeDir || null)
  let busy = $state(false)
  let wslOpen = $state(false)

  function setPath(dir: string) {
    path = dir
    if (!name) name = baseName(dir)
  }

  async function pick() {
    const dir = await open({ directory: true, multiple: false, title: t('newWorkspace.pickTitle') })
    if (typeof dir !== 'string') return
    setPath(dir)
  }

  async function create() {
    if (!path) return
    busy = true
    const w = await app.createWorkspace(path, name.trim() || null, account)
    busy = false
    if (w) app.newWorkspaceOpen = false
  }
</script>

<div class="overlay" role="presentation" onclick={() => (app.newWorkspaceOpen = false)}>
  <div class="modal" role="dialog" tabindex="-1" onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.key === 'Escape' && !wslOpen && (app.newWorkspaceOpen = false)}>
    <header>
      <h3>{t('newWorkspace.title')}</h3>
      <button class="btn ghost icon" title={t('newWorkspace.close')} onclick={() => (app.newWorkspaceOpen = false)}><Icon name="x" /></button>
    </header>
    <div class="body">
      <p class="help">{t('newWorkspace.help')}</p>
      <label for="ws-path">{t('newWorkspace.folder')}</label>
      <div class="row">
        <input id="ws-path" class="field mono" placeholder={t('newWorkspace.pathPlaceholder')} bind:value={path} />
        <button class="btn" onclick={pick}><Icon name="folder" size={14} /> {t('newWorkspace.choose')}</button>
        {#if IS_WINDOWS}
          <button class="btn" title={t('wsl.openFromWslTitle')} onclick={() => (wslOpen = true)}>
            <Icon name="terminal" size={14} /> {t('wsl.openFromWsl')}
          </button>
        {/if}
      </div>
      <label for="ws-name">{t('newWorkspace.name')}</label>
      <input id="ws-name" class="field" placeholder={t('newWorkspace.namePlaceholder')} bind:value={name} />
      <span class="label">{t('newWorkspace.account')}</span>
      <AccountSelect bind:value={account} />
      <p class="hint">{t('newWorkspace.accountHint')}</p>
    </div>
    <footer>
      <button class="btn" onclick={() => (app.newWorkspaceOpen = false)}>{t('newWorkspace.cancel')}</button>
      <button class="btn primary" disabled={!path || busy} onclick={create}>{t('newWorkspace.create')}</button>
    </footer>
  </div>
</div>

{#if wslOpen}
  <WslFolderPicker
    onclose={() => (wslOpen = false)}
    onpick={(dir) => {
      wslOpen = false
      setPath(dir)
    }}
  />
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.4);
    z-index: 110;
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
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .help,
  .hint {
    color: var(--text-muted);
    margin: 0 0 6px;
  }
  .hint {
    font-size: 12px;
  }
  label,
  .label {
    color: var(--text-muted);
    font-size: 12px;
    margin-top: 6px;
  }
  .row {
    display: flex;
    gap: 6px;
  }
  .field {
    flex: 1;
    height: 30px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg-input);
    padding: 0 8px;
    outline: none;
    user-select: text;
  }
  .field:focus {
    border-color: var(--border-strong);
  }
</style>
