<script lang="ts">
  import Logo from './Logo.svelte'
  // Avatar das mensagens: foto do Gravatar do usuário (git config), iniciais
  // quando não há foto, ou o trevo do app para o agente.
  let { kind, src = null, name = null, size = 28 }: { kind: 'user' | 'agent'; src?: string | null; name?: string | null; size?: number } = $props()
  let failed = $state(false)
  const initials = $derived(
    (name ?? '?')
      .split(/\s+/)
      .filter(Boolean)
      .slice(0, 2)
      .map((p) => p[0]?.toUpperCase())
      .join('') || '?',
  )
</script>

<span class="av {kind}" style="width:{size}px;height:{size}px;font-size:{Math.round(size * 0.4)}px">
  {#if kind === 'agent'}
    <Logo {size} variant="small" />
  {:else if src && !failed}
    <img {src} alt={name ?? ''} onerror={() => (failed = true)} />
  {:else}
    {initials}
  {/if}
</span>

<style>
  .av {
    flex: none;
    border-radius: 50%;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    font-weight: 600;
    user-select: none;
  }
  .user {
    background: var(--bg-active);
    color: var(--text);
    border: 1px solid var(--border);
  }
  .agent {
    border-radius: 28%;
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
</style>
