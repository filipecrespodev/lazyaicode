<script lang="ts">
  import { tick } from 'svelte'
  import { app } from '../stores/app.svelte'
  import Icon from './Icon.svelte'
  import { t } from '../i18n/index.svelte'

  // Selo da branch atual no cabeçalho da thread: ao clicar, troca para outra
  // branch local ou cria uma nova (a partir da atual) no checkout `path`.
  let {
    path,
    branch,
    disabled = false,
    onChanged,
  }: { path: string; branch: string | null; disabled?: boolean; onChanged: (name: string) => void } = $props()

  type Local = { name: string; isHead: boolean }
  let open = $state(false)
  let branches = $state<Local[] | null>(null)
  let error = $state('')
  let filter = $state('')
  let working = $state(false)
  /** posição fixa: o cabeçalho corta o que sai dele (overflow hidden) */
  let pos = $state({ x: 0, y: 0 })
  let input: HTMLInputElement | undefined = $state()

  const shown = $derived(
    (branches ?? []).filter((b) => !filter.trim() || b.name.toLowerCase().includes(filter.trim().toLowerCase())),
  )
  const exact = $derived((branches ?? []).some((b) => b.name === filter.trim()))

  const git = () => import('../../git/mount')

  async function toggle(e: MouseEvent) {
    e.stopPropagation()
    if (disabled) return
    open = !open
    if (!open) return
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect()
    pos = { x: Math.min(r.left, window.innerWidth - 312), y: r.bottom + 6 }
    filter = ''
    error = ''
    branches = null
    // autofocus não vale para um elemento inserido depois da carga da página
    tick().then(() => input?.focus())
    try {
      branches = (await (await git()).gitLocalBranches(path)).map((b) => ({ name: b.name, isHead: b.isHead }))
    } catch (err) {
      error = (err as Error).message
    }
  }

  async function run(name: string, action: 'checkout' | 'create') {
    working = true
    try {
      const m = await git()
      if (action === 'checkout') await m.gitCheckout(path, name)
      else await m.gitCreateBranch(path, name)
      app.toast(t(action === 'checkout' ? 'branchMenu.checkedOut' : 'branchMenu.created', { name }), 'ok')
      onChanged(name)
      open = false
    } catch (err) {
      app.toast((err as Error).message, 'error')
    } finally {
      working = false
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') open = false
    else if (e.key === 'Enter') {
      const name = filter.trim()
      if (!name) return
      const match = shown.find((b) => b.name === name)
      if (match) {
        if (!match.isHead) run(name, 'checkout')
      } else run(name, 'create')
    }
  }
</script>

<svelte:window onclick={() => (open = false)} />

<span class="wrap">
  <button class="branch mono" class:disabled onclick={toggle}
    title={disabled ? t('branchMenu.waitAgent') : t('branchMenu.currentTitle', { branch: branch ?? '(detached)' })}>
    <Icon name="branch" size={12} stroke={2.2} /> <span class="bname">{branch ?? '(detached)'}</span>
    <Icon name="down" size={11} stroke={2.2} />
  </button>

  {#if open}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div class="menu" style="left:{pos.x}px; top:{pos.y}px" onclick={(e) => e.stopPropagation()}>
      <input class="field" placeholder={t('branchMenu.placeholder')} bind:this={input} bind:value={filter} onkeydown={onKey} disabled={working} />
      <div class="list">
        {#if error}
          <div class="empty err">{error}</div>
        {:else if !branches}
          <div class="empty">{t('branchMenu.loading')}</div>
        {:else}
          {#each shown as b (b.name)}
            <button class="item mono" class:on={b.isHead} disabled={b.isHead || working} onclick={() => run(b.name, 'checkout')}>
              <span class="ck">{#if b.isHead}<Icon name="check" size={12} />{/if}</span>
              <span class="nm">{b.name}</span>
            </button>
          {:else}
            <div class="empty">{t('branchMenu.noMatch', { filter: filter.trim() })}</div>
          {/each}
        {/if}
      </div>
      {#if filter.trim() && !exact}
        {@const parts = t('branchMenu.create', { name: '\u0000', base: branch ?? 'HEAD' }).split('\u0000')}
        <button class="create" disabled={working} onclick={() => run(filter.trim(), 'create')}>
          <Icon name="plus" size={13} />
          <span>{parts[0]}<b class="mono">{filter.trim()}</b>{parts[1] ?? ''}</span>
        </button>
      {:else}
        <div class="hint">{t('branchMenu.hint')}</div>
      {/if}
    </div>
  {/if}
</span>

<style>
  .wrap {
    display: inline-flex;
    min-width: 0;
  }
  /* a branch atual em destaque: selo na cor de destaque do tema */
  .branch {
    display: inline-flex;
    gap: 4px;
    align-items: center;
    min-width: 0;
    font-size: 11.5px;
    font-weight: 600;
    color: var(--accent);
    background: var(--accent-soft);
    border: 1px solid color-mix(in srgb, var(--accent) 35%, transparent);
    border-radius: 999px;
    padding: 1px 6px 1px 6px;
    cursor: pointer;
  }
  .branch:hover:not(.disabled) {
    border-color: var(--accent);
  }
  .branch.disabled {
    cursor: default;
    opacity: 0.75;
  }
  .bname {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .menu {
    position: fixed;
    z-index: 40;
    width: 300px;
    background: var(--bg-elev);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.35);
    padding: 6px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    white-space: normal;
  }
  .field {
    height: 28px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg-input);
    padding: 0 8px;
    outline: none;
  }
  .field:focus {
    border-color: var(--border-strong);
  }
  .list {
    max-height: 240px;
    overflow-y: auto;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    text-align: left;
    background: none;
    border: none;
    border-radius: 4px;
    padding: 5px 6px;
    font-size: 12px;
    color: var(--text);
  }
  .item:hover:not(:disabled) {
    background: var(--bg-hover);
  }
  .item.on {
    color: var(--accent);
    font-weight: 600;
  }
  .item:disabled {
    cursor: default;
  }
  .ck {
    width: 14px;
    display: inline-flex;
  }
  .nm {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .create {
    display: flex;
    align-items: center;
    gap: 6px;
    background: var(--accent-soft);
    color: var(--accent);
    border: none;
    border-radius: 4px;
    padding: 6px 8px;
    font-size: 12px;
    text-align: left;
  }
  .create span {
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .create b {
    font-weight: 600;
  }
  .hint,
  .empty {
    font-size: 11.5px;
    color: var(--text-faint);
    padding: 4px 6px;
  }
  .empty.err {
    color: var(--red);
  }
</style>
