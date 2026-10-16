<script lang="ts">
  import { onDestroy, onMount } from 'svelte'
  import { Terminal } from '@xterm/xterm'
  import { FitAddon } from '@xterm/addon-fit'
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'
  import { api, errText } from '../api'
  import { t } from '../i18n/index.svelte'
  import { loadFonts, terminalFontFamily } from '../fonts'

  let { cwd, termId }: { cwd: string; termId: string } = $props()
  let el: HTMLDivElement
  let term: Terminal
  let fit: FitAddon
  let unlisten: UnlistenFn | undefined
  let ro: ResizeObserver | undefined
  let exited = $state(false)

  function decode(b64: string) {
    const bin = atob(b64)
    const bytes = new Uint8Array(bin.length)
    for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i)
    return bytes
  }

  onMount(async () => {
    const fonts = loadFonts()
    // a fonte de ícones embutida carrega sob demanda: garante antes de o
    // xterm medir as células, senão os ícones do prompt saem como quadrados
    await document.fonts?.load(`${fonts.terminalSize}px "LazyAICode Symbols"`, '\ue0b0').catch(() => {})
    term = new Terminal({
      fontFamily: terminalFontFamily(fonts),
      fontSize: fonts.terminalSize,
      cursorBlink: true,
      allowProposedApi: true,
      theme: termTheme(),
    })
    window.addEventListener('appearance-changed', onAppearance)
    window.addEventListener('fonts-changed', onFonts)
    fit = new FitAddon()
    term.loadAddon(fit)
    term.open(el)
    fit.fit()
    unlisten = await listen<{ termId: string; data: string | null }>('term-data', ({ payload }) => {
      if (payload.termId !== termId) return
      if (payload.data === null) exited = true
      else term.write(decode(payload.data))
    })
    term.onData((d) => api.termWrite(termId, d).catch(() => {}))
    term.onResize(({ cols, rows }) => api.termResize(termId, cols, rows).catch(() => {}))
    try {
      await api.termSpawn(termId, cwd, term.cols, term.rows)
    } catch (e) {
      term.write(`\r\n${t('terminalPane.openError', { error: errText(e) })}\r\n`)
    }
    ro = new ResizeObserver(() => {
      try {
        fit.fit()
      } catch {}
    })
    ro.observe(el)
    term.focus()
  })

  // cores do terminal acompanham o tema do app
  function termTheme() {
    const css = getComputedStyle(document.documentElement)
    const v = (k: string) => css.getPropertyValue(k).trim()
    return { background: v('--bg-input'), foreground: v('--text'), cursor: v('--accent'), selectionBackground: v('--bg-active') }
  }

  function onAppearance() {
    if (term) term.options.theme = termTheme()
  }

  // fonte do terminal (Aparência → Fontes)
  function onFonts() {
    if (!term) return
    const fonts = loadFonts()
    term.options.fontFamily = terminalFontFamily(fonts)
    term.options.fontSize = fonts.terminalSize
    try {
      fit.fit()
    } catch {}
  }

  onDestroy(() => {
    window.removeEventListener('appearance-changed', onAppearance)
    window.removeEventListener('fonts-changed', onFonts)
    unlisten?.()
    ro?.disconnect()
    api.termKill(termId).catch(() => {})
    term?.dispose()
  })
</script>

<div class="wrap">
  <div class="term" bind:this={el}></div>
  {#if exited}<div class="exit">{t('terminalPane.exited')}</div>{/if}
</div>

<style>
  .wrap {
    position: relative;
    height: 100%;
    background: var(--bg-input);
    padding: 6px 0 0 8px;
  }
  .term {
    height: 100%;
  }
  .exit {
    position: absolute;
    right: 12px;
    bottom: 8px;
    font-size: 11px;
    color: var(--text-faint);
  }
</style>
