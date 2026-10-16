<script lang="ts">
  import full from '../brand/logo.svg?raw'
  import small from '../brand/logo-small.svg?raw'
  import mark from '../brand/mark.svg?raw'

  // Logo do LazyAICode. Até 40px usa a versão simplificada (sem o "z z",
  // traço mais grosso), que continua legível; `mark` é só o desenho, na cor
  // do texto em volta.
  let { size = 24, variant = 'auto' }: { size?: number; variant?: 'auto' | 'full' | 'small' | 'mark' } = $props()
  const svg = $derived(variant === 'mark' ? mark : variant === 'full' ? full : variant === 'small' ? small : size <= 40 ? small : full)
</script>

<span class="logo" style="width:{size}px;height:{size}px">{@html svg}</span>

<style>
  .logo {
    display: inline-flex;
    flex: none;
  }
  .logo :global(svg) {
    width: 100%;
    height: 100%;
    display: block;
  }
</style>
