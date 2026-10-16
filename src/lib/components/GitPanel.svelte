<script lang="ts">
  import { onDestroy } from 'svelte'
  import type { GitMode } from '../stores/app.svelte'
  import { t } from '../i18n/index.svelte'

  // Ilha React (src/git) com um painel Git de um checkout: staging, grafo,
  // árvore, reflog ou estatísticas.
  // O bundle do git só carrega na primeira vez que um painel aparece.
  let { path, mode, active }: { path: string; mode: GitMode; active: boolean } = $props()

  type Handle = { update: (p: { path: string; mode: GitMode; active: boolean }) => void; unmount: () => void }
  let el: HTMLDivElement | undefined = $state()
  let handle: Handle | null = null
  let loading = false
  let failed = $state<string | null>(null)

  $effect(() => {
    const props = { path, mode, active }
    if (handle) handle.update(props)
    else if (active && el && !loading) mount(props)
  })

  async function mount(props: { path: string; mode: GitMode; active: boolean }) {
    loading = true
    try {
      const { mountRepoPanel } = await import('../../git/mount')
      handle = mountRepoPanel(el!, props)
      handle.update({ path, mode, active })
    } catch (e) {
      failed = String(e)
    }
  }

  onDestroy(() => handle?.unmount())
</script>

<div class="git-panel">
  {#if failed}<div class="err">{t('gitPanel.loadPanelError', { error: failed })}</div>{/if}
  <div class="host" bind:this={el}></div>
</div>

<style>
  .git-panel {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .host {
    flex: 1;
    min-height: 0;
  }
  .err {
    padding: 16px;
    color: var(--red);
  }
</style>
