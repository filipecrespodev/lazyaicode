<script lang="ts">
  import { onDestroy, onMount } from 'svelte'
  import { t } from '../i18n/index.svelte'

  // Seção de configurações do git (React, src/git) dentro das Configurações.
  let { section }: { section: 'git' | 'integrations' } = $props()

  let el: HTMLDivElement
  let handle: { unmount: () => void } | null = null
  let failed = $state<string | null>(null)

  onMount(async () => {
    try {
      const { mountGitSettings } = await import('../../git/mount')
      handle = mountGitSettings(el, section)
    } catch (e) {
      failed = String(e)
    }
  })

  onDestroy(() => handle?.unmount())
</script>

{#if failed}<p class="err">{t('gitPanel.loadError', { error: failed })}</p>{/if}
<div bind:this={el}></div>

<style>
  .err {
    color: var(--red);
  }
</style>
