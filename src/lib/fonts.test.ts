import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  DEFAULT_FONTS, applyFonts, fontStack, fontVars, loadFonts, normalizeFonts, rootFontSize, saveFonts, terminalFontFamily,
} from './fonts'

function fakeRoot() {
  const props = new Map<string, string>()
  const root = {
    style: {
      fontSize: '',
      setProperty: (k: string, v: string) => void props.set(k, v),
      removeProperty: (k: string) => void props.delete(k),
    },
    dataset: {} as Record<string, string>,
  }
  return { root: root as unknown as HTMLElement, props }
}

afterEach(() => vi.unstubAllGlobals())

describe('fonts', () => {
  it('quotes a bare family name and keeps a comma list as a stack', () => {
    expect(fontStack('', 'monospace')).toBeNull()
    expect(fontStack('  ', 'monospace')).toBeNull()
    expect(fontStack('Fira Code', 'monospace')).toBe('"Fira Code", monospace')
    expect(fontStack('Iosevka, Menlo', 'monospace')).toBe('Iosevka, Menlo, monospace')
  })

  it('defaults leave the stylesheet in charge (every variable removed)', () => {
    expect(Object.values(fontVars(DEFAULT_FONTS)).every((v) => v === null)).toBe(true)
    expect(rootFontSize(DEFAULT_FONTS)).toBe('')
  })

  it('custom fonts feed both the shell (--sans/--mono) and the Git view (Tailwind --font-*)', () => {
    const v = fontVars({ ...DEFAULT_FONTS, textFamily: 'Ubuntu', textSize: 15, codeFamily: 'Fira Code', codeSize: 14 })
    expect(v['--sans']).toMatch(/^"Ubuntu", /)
    expect(v['--font-sans']).toBe(v['--sans'])
    expect(v['--mono']).toMatch(/^"Fira Code", /)
    expect(v['--font-mono']).toBe(v['--mono'])
    expect(v['--text-size']).toBe('15px')
    expect(v['--code-size']).toBe('14px')
    // rem da visão Git escala na mesma proporção do texto (13px ↔ 16px)
    expect(rootFontSize({ ...DEFAULT_FONTS, textSize: 26 })).toBe('32px')
  })

  it('terminal: chosen font first, then the bundled icons and installed Nerd/mono fonts', () => {
    const auto = terminalFontFamily(DEFAULT_FONTS)
    expect(auto.startsWith('"LazyAICode Symbols", ')).toBe(true)
    expect(auto.endsWith(', monospace')).toBe(true)
    expect(auto).toContain('"MesloLGS NF"')
    expect(terminalFontFamily({ ...DEFAULT_FONTS, terminalFamily: 'Hack Nerd Font' })).toMatch(/^"Hack Nerd Font", "LazyAICode Symbols", /)
    expect(terminalFontFamily({ ...DEFAULT_FONTS, terminalFamily: 'Iosevka, Menlo' })).toMatch(/^Iosevka, Menlo, "LazyAICode Symbols", /)
    // fonte do terminal independe da do código
    expect(terminalFontFamily({ ...DEFAULT_FONTS, codeFamily: 'Fira Code' })).toBe(auto)
  })

  it('normalizes saved values: missing or out-of-range fields fall back', () => {
    expect(normalizeFonts(null)).toEqual(DEFAULT_FONTS)
    expect(normalizeFonts({ textSize: 99, codeSize: 2, codeFamily: ' Hack ' })).toEqual({
      ...DEFAULT_FONTS, textSize: 20, codeSize: 8, codeFamily: 'Hack',
    })
    expect(normalizeFonts({ textSize: 'x' as unknown as number })).toEqual(DEFAULT_FONTS)
  })

  it('applies onto the root and resets back to the defaults', () => {
    const { root, props } = fakeRoot()
    applyFonts({ ...DEFAULT_FONTS, textFamily: 'Ubuntu', textSize: 14, codeSize: 13 }, root)
    expect(props.get('--sans')).toMatch(/Ubuntu/)
    expect(props.has('--mono')).toBe(false)
    expect(props.get('--code-size')).toBe('13px')
    expect(root.dataset.codeSize).toBe('custom')
    expect(root.style.fontSize).not.toBe('')
    applyFonts(DEFAULT_FONTS, root)
    expect(props.size).toBe(0)
    expect(root.dataset.codeSize).toBeUndefined()
    expect(root.style.fontSize).toBe('')
  })

  it('round-trips through localStorage and survives it being unavailable', () => {
    expect(loadFonts()).toEqual(DEFAULT_FONTS)
    const store = new Map<string, string>()
    vi.stubGlobal('localStorage', { getItem: (k: string) => store.get(k) ?? null, setItem: (k: string, v: string) => store.set(k, v) })
    const f = { textFamily: 'Ubuntu', textSize: 14, codeFamily: 'Hack', codeSize: 11, terminalFamily: 'MesloLGS NF', terminalSize: 15 }
    saveFonts(f)
    expect(loadFonts()).toEqual(f)
    store.set('ui.fonts', '{not json')
    expect(loadFonts()).toEqual(DEFAULT_FONTS)
  })
})
