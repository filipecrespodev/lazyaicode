<script lang="ts" generics="T extends string">
  import type { Snippet } from 'svelte'
  import Icon from './Icon.svelte'
  import { t } from '../i18n/index.svelte'

  // Botão do rodapé que abre uma lista para cima (como no Delta): grupos,
  // marca na opção atual e busca opcional (seletor de modelo).
  type Option = { value: T; label: string; hint?: string; group?: string; icon?: string }
  let {
    options,
    value,
    onSelect,
    trigger,
    title = '',
    searchable = false,
    placeholder,
    width = 300,
    align = 'left',
    footer,
  }: {
    options: Option[]
    value: T | null
    onSelect: (v: T) => void
    trigger: Snippet
    title?: string
    searchable?: boolean
    placeholder?: string
    width?: number
    align?: 'left' | 'right'
    footer?: Snippet<[() => void]>
  } = $props()

  let open = $state(false)
  let q = $state('')
  let idx = $state(0)

  const filtered = $derived(
    options.filter((o) => !q || `${o.label} ${o.hint ?? ''} ${o.value}`.toLowerCase().includes(q.toLowerCase())),
  )
  const groups = $derived([...new Set(filtered.map((o) => o.group ?? ''))])

  function toggle(e: MouseEvent) {
    e.stopPropagation()
    open = !open
    q = ''
    idx = Math.max(0, filtered.findIndex((o) => o.value === value))
  }

  function pick(o: Option) {
    open = false
    onSelect(o.value)
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') open = false
    else if (e.key === 'ArrowDown') { idx = Math.min(idx + 1, filtered.length - 1); e.preventDefault() }
    else if (e.key === 'ArrowUp') { idx = Math.max(idx - 1, 0); e.preventDefault() }
    else if (e.key === 'Enter' && filtered[idx]) { pick(filtered[idx]); e.preventDefault() }
  }
</script>

<svelte:window onclick={() => (open = false)} />

<div class="menu">
  <button class="trigger" class:on={open} {title} onclick={toggle}>{@render trigger()}</button>
  {#if open}
    <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
    <div class="pop" style="width:{width}px; {align === 'right' ? 'right:0' : 'left:0'}" onclick={(e) => e.stopPropagation()} onkeydown={onKey}>
      {#if searchable}
        <!-- svelte-ignore a11y_autofocus -->
        <input autofocus placeholder={placeholder ?? t('menu.search')} bind:value={q} oninput={() => (idx = 0)} />
      {/if}
      <div class="list">
        {#each groups as g (g)}
          {#if g}<div class="group">{g}</div>{/if}
          {#each filtered.filter((o) => (o.group ?? '') === g) as o (g + o.value)}
            {@const i = filtered.indexOf(o)}
            <button class="opt" class:hl={i === idx} onmouseenter={() => (idx = i)} onclick={() => pick(o)}>
              {#if o.icon}<Icon name={o.icon} size={13} />{/if}
              <span class="l">{o.label}{#if o.hint}<span class="h">{o.hint}</span>{/if}</span>
              {#if o.value === value}<span class="ck"><Icon name="check" size={13} /></span>{/if}
            </button>
          {/each}
        {:else}
          <div class="none">{t('menu.nothingFound')}</div>
        {/each}
      </div>
      {#if footer}<div class="foot">{@render footer(() => (open = false))}</div>{/if}
    </div>
  {/if}
</div>

<style>
  .menu {
    position: relative;
    display: inline-flex;
  }
  .trigger {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 26px;
    padding: 0 7px;
    border: none;
    border-radius: 6px;
    background: none;
    color: var(--text-muted);
    font-size: 12.5px;
    white-space: nowrap;
  }
  .trigger:hover,
  .trigger.on {
    background: var(--bg-hover);
    color: var(--text);
  }
  .pop {
    position: absolute;
    bottom: 32px;
    z-index: 50;
    max-height: 420px;
    display: flex;
    flex-direction: column;
    background: var(--bg-elev);
    border: 1px solid var(--border-strong);
    border-radius: 10px;
    box-shadow: 0 14px 40px rgba(0, 0, 0, 0.4);
    overflow: hidden;
  }
  input {
    border: none;
    border-bottom: 1px solid var(--border);
    background: transparent;
    padding: 10px 12px;
    outline: none;
    font-size: 13px;
    user-select: text;
  }
  .list {
    overflow-y: auto;
    padding: 4px;
  }
  .group {
    font-size: 11px;
    color: var(--text-faint);
    padding: 8px 8px 4px;
  }
  .opt {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 8px;
    border: none;
    border-radius: 6px;
    background: none;
    text-align: left;
    color: var(--text);
  }
  .opt.hl {
    background: var(--bg-active);
  }
  .l {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .h {
    color: var(--text-faint);
    font-size: 11.5px;
    margin-left: 6px;
  }
  .ck {
    color: var(--blue);
    display: inline-flex;
  }
  .none {
    padding: 12px;
    color: var(--text-faint);
    text-align: center;
  }
  .foot {
    border-top: 1px solid var(--border);
    padding: 4px;
  }
</style>
