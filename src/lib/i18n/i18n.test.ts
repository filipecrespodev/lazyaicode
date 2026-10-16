import { describe, expect, it } from 'vitest'
import { getLocale, setLocale, t } from './index.svelte'
import settings from './messages/settings'

describe('i18n', () => {
  it('defaults to English, switches locale and falls back to English/key', () => {
    expect(getLocale()).toBe('en')
    expect(t('settings.language')).toBe(settings.en.language)
    setLocale('pt')
    expect(t('settings.language')).toBe(settings.pt.language)
    setLocale('xx')
    expect(getLocale()).toBe('en')
    expect(t('nope.missing')).toBe('nope.missing')
  })

  it('every namespace has the same keys in all locales', async () => {
    const mods = import.meta.glob<{ default: Record<string, Record<string, string>> }>('./messages/*.ts', { eager: true })
    for (const [path, m] of Object.entries(mods)) {
      const en = Object.keys(m.default.en).sort()
      for (const l of ['pt', 'es', 'zh']) expect(Object.keys(m.default[l]).sort(), `${path} ${l}`).toEqual(en)
      for (const l of ['en', 'pt', 'es', 'zh'])
        for (const [k, v] of Object.entries(m.default[l])) {
          const ph = (s: string) => (s.match(/\{\w+\}/g) ?? []).sort().join()
          expect(ph(v), `${path} ${l}.${k} placeholders`).toBe(ph(m.default.en[k]))
        }
    }
  })
})
