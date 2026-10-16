<script lang="ts">
  import type { Item } from '../transcript'
  import { toolSummary } from '../transcript'
  import Icon from './Icon.svelte'
  import { t } from '../i18n/index.svelte'

  let { item }: { item: Extract<Item, { kind: 'tool' }> } = $props()
  let open = $state(false)

  const pending = $derived(!item.result)
  const summary = $derived(toolSummary(item.name, item.input))
  const isEdit = $derived(
    (item.name === 'Edit' || item.name === 'MultiEdit') && typeof item.input.old_string === 'string',
  )
  const resultText = $derived(item.result?.text ?? '')
  const preview = $derived(resultText.length > 6000 ? resultText.slice(0, 6000) + '\n…' : resultText)
</script>

<div class="tool" class:err={item.result?.isError}>
  <button class="head" onclick={() => (open = !open)}>
    <span class="chev" class:open><Icon name="chevron" size={11} /></span>
    <span class="name">{item.name}</span>
    <span class="sum mono">{summary || (item.partial ? '…' : '')}</span>
    {#if pending}<span class="spin"></span>
    {:else if item.result?.isError}<span class="state bad">{t('tool.error')}</span>
    {:else}<span class="state ok"><Icon name="check" size={12} /></span>{/if}
  </button>
  {#if open}
    <div class="body selectable">
      {#if isEdit}
        <pre class="mono diff"><span class="del">{String(item.input.old_string).split('\n').map((l) => '- ' + l).join('\n')}</span>
<span class="add">{String(item.input.new_string).split('\n').map((l) => '+ ' + l).join('\n')}</span></pre>
      {:else if item.name === 'Bash'}
        <pre class="mono">$ {item.input.command}</pre>
      {:else}
        <pre class="mono">{JSON.stringify(item.input, null, 2)}</pre>
      {/if}
      {#if item.result}
        <div class="label">{t('tool.result')}</div>
        <pre class="mono out">{preview || t('tool.empty')}</pre>
      {/if}
    </div>
  {/if}
</div>

<style>
  .tool {
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-panel);
    overflow: hidden;
  }
  .tool.err {
    border-color: color-mix(in srgb, var(--red) 40%, var(--border));
  }
  .head {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    background: none;
    border: none;
    text-align: left;
    color: var(--text-muted);
  }
  .head:hover {
    background: var(--bg-hover);
  }
  .chev {
    display: inline-flex;
    transition: transform 0.12s;
  }
  .chev.open {
    transform: rotate(90deg);
  }
  .name {
    color: var(--text);
    font-weight: 500;
  }
  .sum {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-muted);
  }
  .state.ok {
    color: var(--accent);
    display: inline-flex;
  }
  .state.bad {
    color: var(--red);
    font-size: 11px;
  }
  .spin {
    width: 11px;
    height: 11px;
    border: 2px solid var(--border-strong);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .body {
    border-top: 1px solid var(--border);
    padding: 8px 10px;
  }
  pre {
    margin: 0;
    white-space: pre-wrap;
    word-break: break-word;
    max-height: 360px;
    overflow: auto;
  }
  .label {
    font-size: 11px;
    color: var(--text-faint);
    margin: 8px 0 4px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }
  .out {
    color: var(--text-muted);
  }
  .del {
    color: var(--del-fg);
  }
  .add {
    color: var(--add-fg);
  }
</style>
