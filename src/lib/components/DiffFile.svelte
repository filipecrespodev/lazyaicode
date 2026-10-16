<script lang="ts">
  import type { FileChange } from '../api'
  import { parsePatch } from '../diff'
  import Icon from './Icon.svelte'
  import { t } from '../i18n/index.svelte'

  let {
    file,
    collapsed,
    onToggle,
    onSeen,
  }: { file: FileChange; collapsed: boolean; onToggle: () => void; onSeen: (seen: boolean) => void } = $props()

  const lines = $derived(collapsed ? [] : parsePatch(file.patch))
  const statusLabel: Record<string, string> = { M: 'M', A: 'A', D: 'D', R: 'R' }
</script>

<div class="file" class:seen={file.seen}>
  <div class="fhead">
    <button class="chev" class:open={!collapsed} onclick={onToggle}><Icon name="chevron" size={12} /></button>
    <span class="st st-{file.status}">{statusLabel[file.status] ?? file.status}</span>
    <button class="path mono" onclick={onToggle} title={file.path}>
      {#if file.old_path}<span class="old">{file.old_path} → </span>{/if}{file.path}
    </button>
    <span class="stat"><span class="plus">+{file.additions}</span> <span class="minus">−{file.deletions}</span></span>
    <label class="seen-box" title={t('diffFile.markSeen')}>
      <input type="checkbox" checked={file.seen} onchange={(e) => onSeen(e.currentTarget.checked)} /> {t('diffFile.seen')}
    </label>
  </div>
  {#if !collapsed}
    {#if file.binary}
      <div class="note">{t('diffFile.binary')}</div>
    {:else}
      <div class="body mono selectable">
        {#each lines as l}
          {#if l.t === 'hunk'}
            <div class="hunk">{l.text}</div>
          {:else}
            <div class="ln {l.t}">
              <span class="n">{l.a ?? ''}</span><span class="n">{l.b ?? ''}</span><span class="sig">{l.t === 'add' ? '+' : l.t === 'del' ? '−' : ' '}</span><span class="code">{l.text}</span>
            </div>
          {/if}
        {/each}
        {#if file.truncated}<div class="note">{t('diffFile.truncated')}</div>{/if}
      </div>
    {/if}
  {/if}
</div>

<style>
  .file {
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
    background: var(--bg-elev);
  }
  .fhead {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 10px;
    background: var(--bg-panel);
    position: sticky;
    top: 0;
    z-index: 1;
  }
  .file.seen .fhead {
    opacity: 0.7;
  }
  .chev {
    background: none;
    border: none;
    padding: 0;
    display: inline-flex;
    color: var(--text-muted);
    transition: transform 0.12s;
  }
  .chev.open {
    transform: rotate(90deg);
  }
  .st {
    font-family: var(--mono);
    font-size: 11px;
    font-weight: 700;
    width: 14px;
    text-align: center;
  }
  .st-M {
    color: var(--amber);
  }
  .st-A {
    color: var(--accent);
  }
  .st-D {
    color: var(--red);
  }
  .st-R {
    color: var(--blue);
  }
  .path {
    flex: 1;
    min-width: 0;
    text-align: left;
    background: none;
    border: none;
    padding: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl;
  }
  .old {
    color: var(--text-faint);
  }
  .stat {
    font-family: var(--mono);
    font-size: 11px;
  }
  .plus {
    color: var(--add-fg);
  }
  .minus {
    color: var(--del-fg);
  }
  .seen-box {
    display: inline-flex;
    gap: 4px;
    align-items: center;
    font-size: 11.5px;
    color: var(--text-muted);
  }
  .body {
    font-size: var(--code-size, 12px);
    line-height: 1.55;
    overflow-x: auto;
  }
  .hunk {
    color: var(--text-faint);
    background: var(--bg-panel);
    padding: 2px 10px;
    white-space: pre;
  }
  .ln {
    display: flex;
    white-space: pre;
    min-width: max-content;
  }
  .ln.add {
    background: var(--add-bg);
  }
  .ln.del {
    background: var(--del-bg);
  }
  .n {
    width: 44px;
    flex: none;
    text-align: right;
    padding-right: 8px;
    color: var(--text-faint);
    user-select: none;
  }
  .sig {
    width: 14px;
    flex: none;
    user-select: none;
    color: var(--text-faint);
  }
  .add .sig,
  .add .code {
    color: var(--add-fg);
  }
  .del .sig,
  .del .code {
    color: var(--del-fg);
  }
  .code {
    padding-right: 16px;
  }
  .note {
    padding: 8px 12px;
    color: var(--text-muted);
    font-size: 12px;
  }
</style>
