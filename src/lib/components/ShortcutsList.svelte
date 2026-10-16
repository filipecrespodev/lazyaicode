<script lang="ts">
  import { t } from '../i18n/index.svelte'
  import {
    ACTIONS,
    FIXED,
    GROUPS,
    bindingsFor,
    eventToBinding,
    findConflict,
    formatBinding,
    groupLabel,
    isUsableBinding,
  } from '../shortcuts/shortcuts'
  import { getOverrides, resetBinding, setBinding } from '../shortcuts/store.svelte'

  // Todas as ações com as combinações atuais, por grupo: aba Atalhos das
  // Configurações (editable) e a lista do Ctrl+/.
  let { editable }: { editable: boolean } = $props()

  let recording = $state<string | null>(null)
  /** Aviso da última tentativa (combinação inválida ou já usada), por ação. */
  let warning = $state<{ id: string; text: string } | null>(null)

  const overrides = $derived(getOverrides())

  $effect(() => {
    const id = recording
    if (!id) return
    const onKeyDown = (e: KeyboardEvent) => {
      e.preventDefault()
      e.stopPropagation()
      if (e.key === 'Escape') {
        recording = null
        return
      }
      const binding = eventToBinding(e)
      if (!binding) return
      if (!isUsableBinding(binding)) {
        warning = { id, text: t('shortcuts.unusable') }
        return
      }
      const conflict = findConflict(id, binding, getOverrides())
      if (conflict) {
        warning = { id, text: t('shortcuts.conflict', { keys: formatBinding(binding), action: conflict.label }) }
        return
      }
      warning = null
      setBinding(id, binding)
      recording = null
    }
    // fase de captura: roda antes dos atalhos do app (e do Esc que fecha a janela)
    window.addEventListener('keydown', onKeyDown, true)
    return () => window.removeEventListener('keydown', onKeyDown, true)
  })

  function toggleRecording(id: string) {
    warning = null
    recording = recording === id ? null : id
  }
</script>

<div class="groups">
  {#each GROUPS as group (group)}
    <section>
      <h4>{groupLabel(group)}</h4>
      <ul>
        {#each ACTIONS.filter((a) => a.group === group) as a (a.id)}
          <li>
            <span class="name">{a.label}</span>
            {#if editable}
              <button class="bind" class:rec={recording === a.id} onclick={() => toggleRecording(a.id)}>
                {recording === a.id ? t('shortcuts.pressKeys') : bindingsFor(a.id, overrides).map((b) => formatBinding(b)).join(' / ')}
              </button>
              <button class="reset" class:hidden={!overrides[a.id]} title={t('shortcuts.resetTitle')} onclick={() => resetBinding(a.id)}>
                {t('shortcuts.reset')}
              </button>
            {:else}
              <span class="keys">
                {#each bindingsFor(a.id, overrides) as b (b)}<kbd class="kbd">{formatBinding(b)}</kbd>{/each}
              </span>
            {/if}
          </li>
          {#if warning?.id === a.id}<li class="warn">{warning.text}</li>{/if}
        {/each}
        {#each FIXED.filter((f) => f.group === group) as f (f.id)}
          <li class="fixed">
            <span class="name">{f.label}</span>
            <span class="keys" title={editable ? t('shortcuts.fixed') : undefined}>
              {#each f.keys as k (k)}<kbd class="kbd">{formatBinding(k)}</kbd>{/each}
            </span>
            {#if editable}<span class="reset hidden">{t('shortcuts.reset')}</span>{/if}
          </li>
        {/each}
      </ul>
    </section>
  {/each}
</div>

<style>
  .groups {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  h4 {
    margin: 0 0 4px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-faint);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 0;
    border-bottom: 1px solid var(--border);
  }
  li:last-child {
    border-bottom: none;
  }
  .name {
    flex: 1;
    min-width: 0;
    color: var(--text-muted);
  }
  .fixed .name {
    color: var(--text-faint);
  }
  .keys {
    display: inline-flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 4px;
  }
  .bind {
    min-width: 120px;
    padding: 2px 8px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg-panel);
    color: var(--text);
    font-family: var(--mono);
    font-size: 11px;
  }
  .bind:hover {
    background: var(--bg-hover);
  }
  .bind.rec {
    border-color: var(--accent);
    color: var(--accent);
    font-family: var(--sans);
  }
  .reset {
    border: none;
    background: none;
    padding: 0;
    font-size: 11px;
    color: var(--text-faint);
  }
  .reset:hover {
    color: var(--text);
  }
  .reset.hidden {
    visibility: hidden;
  }
  .warn {
    border-bottom: none;
    padding: 0 0 6px;
    font-size: 12px;
    color: var(--amber);
  }
</style>
