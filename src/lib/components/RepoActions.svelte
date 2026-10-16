<script lang="ts">
  import { onDestroy, onMount } from 'svelte'

  // Fetch, Push e Stash do repositório (ilha React, src/git) na barra de abas.
  let { path }: { path: string } = $props()

  let el: HTMLDivElement
  let handle: { update: (p: string) => void; unmount: () => void } | null = null

  onMount(async () => {
    const { mountRepoActions } = await import('../../git/mount')
    handle = mountRepoActions(el, path)
  })

  $effect(() => {
    handle?.update(path)
  })

  onDestroy(() => handle?.unmount())
</script>

<div class="actions" bind:this={el}></div>

<style>
  .actions {
    display: flex;
    align-items: center;
    flex: none;
    margin-right: 4px;
  }
</style>
