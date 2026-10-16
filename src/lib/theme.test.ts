import { describe, expect, it } from 'vitest'
import { THEMES, contrast, cssVars, resolveTheme } from './theme'

describe('themes', () => {
  it('every palette defines every color as a hex value', () => {
    expect(THEMES.length).toBeGreaterThanOrEqual(13)
    for (const t of THEMES) {
      for (const [k, v] of Object.entries(t.palette)) expect(v, `${t.id}.${k}`).toMatch(/^#[0-9a-f]{6}$/i)
    }
    expect(new Set(THEMES.map((t) => t.id)).size).toBe(THEMES.length)
  })

  it('"system" follows the OS between the default dark and light themes, unknown ids too', () => {
    expect(resolveTheme('system', true).id).toBe('dark')
    expect(resolveTheme('system', false).id).toBe('light')
    expect(resolveTheme('não-existe', false).id).toBe('light')
    expect(resolveTheme('nord', false).id).toBe('nord')
  })

  it('produces every variable the stylesheet uses, with diff colors derived from the palette', () => {
    const v = cssVars(resolveTheme('dracula', true))
    for (const k of ['--bg', '--bg-panel', '--bg-elev', '--bg-hover', '--bg-active', '--bg-input', '--border', '--border-strong',
      '--text', '--text-muted', '--text-faint', '--accent', '--accent-soft', '--blue', '--amber', '--red',
      '--add-bg', '--add-fg', '--del-bg', '--del-fg']) expect(v[k], k).toBeTruthy()
    expect(v['--add-bg']).toContain('#50fa7b')
  })

  it('primary button text stays readable (WCAG AA, 4.5:1) on every theme accent', () => {
    for (const t of THEMES) {
      const on = cssVars(t)['--on-accent']
      expect(contrast(t.palette.accent, on), t.id).toBeGreaterThanOrEqual(4.5)
    }
  })
})
