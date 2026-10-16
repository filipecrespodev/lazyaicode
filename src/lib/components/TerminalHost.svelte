<script lang="ts">
  import TerminalPane from './TerminalPane.svelte'

  // Terminais abertos por chave (thread ou repositório). Cada um sobrevive à
  // troca de aba e de thread: só o da chave atual fica visível.
  let { termKey, cwd, visible }: { termKey: string | null; cwd: string | null; visible: boolean } = $props()

  let terms = $state<Record<string, { cwd: string; n: number }>>({})

  $effect(() => {
    if (visible && termKey && cwd && !terms[termKey]) terms[termKey] = { cwd, n: Date.now() }
  })
</script>

<div class="termhost" class:gone={!visible}>
  {#each Object.entries(terms) as [key, t] (key + t.n)}
    <div class="termslot" class:parked={key !== termKey}>
      <TerminalPane cwd={t.cwd} termId={`${key}-${t.n}`} />
    </div>
  {/each}
</div>

<style>
  .termhost {
    flex: 1;
    position: relative;
    min-height: 0;
  }
  .termhost.gone {
    display: none;
  }
  .termslot {
    position: absolute;
    inset: 0;
  }
  .termslot.parked {
    visibility: hidden;
  }
</style>
