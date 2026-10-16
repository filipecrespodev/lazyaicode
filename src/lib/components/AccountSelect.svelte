<script lang="ts">
  import { onMount } from 'svelte'
  import { api, type ClaudeDir } from '../api'
  import { app } from '../stores/app.svelte'
  import { t } from '../i18n/index.svelte'

  // Escolha da conta Claude (diretório de configuração) de um workspace.
  let { value = $bindable(), onchange }: { value: string | null; onchange?: (v: string | null) => void } = $props()
  let dirs = $state<ClaudeDir[]>([])

  onMount(async () => {
    dirs = await api.listClaudeDirs()
  })

  function key(d: ClaudeDir) {
    return d.is_default ? '' : d.path
  }
</script>

<select
  class="acct-select"
  value={value ?? ''}
  onchange={(e) => {
    const v = e.currentTarget.value || null
    value = v
    onchange?.(v)
  }}
>
  {#if value && !dirs.some((d) => d.path === value)}<option value={value}>{app.accountLabel(value)}</option>{/if}
  {#each dirs as d (d.path)}
    <option value={key(d)}>{d.label}{d.has_credentials ? '' : t('account.noCredentials')}</option>
  {/each}
</select>

<style>
  .acct-select {
    height: 30px;
    width: 100%;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg-input);
    padding: 0 8px;
    font-family: var(--mono);
    font-size: 12px;
  }
</style>
