import DOMPurify from 'dompurify'
import { marked } from 'marked'
import { intlLocale, t } from './i18n/index.svelte'

marked.setOptions({ gfm: true, breaks: false })

const cache = new Map<string, string>()

/** Markdown do agente → HTML sanitizado (o texto vem de um modelo, não é confiável). */
export function renderMarkdown(src: string): string {
  const hit = cache.get(src)
  if (hit) return hit
  const html = DOMPurify.sanitize(marked.parse(src, { async: false }) as string)
  if (cache.size > 500) cache.clear()
  cache.set(src, html)
  return html
}

export function relTime(ms: number): string {
  const s = Math.round((Date.now() - ms) / 1000)
  if (s < 60) return t('time.now')
  if (s < 3600) return t('time.minutes', { n: Math.floor(s / 60) })
  if (s < 86400) return t('time.hours', { n: Math.floor(s / 3600) })
  if (s < 86400 * 30) return t('time.days', { n: Math.floor(s / 86400) })
  return new Date(ms).toLocaleDateString(intlLocale())
}
