// Idioma da interface. Um só módulo para as duas UIs: no Svelte, `t()` lê um
// `$state` e o template re-renderiza ao trocar de idioma; no React, `useT()`
// assina as mudanças. Os textos ficam em `messages/<ns>.ts`, um arquivo por
// área, cada um com as quatro línguas; a chave final é `<ns>.<chave>`.

export const LOCALES = [
  { id: 'en', label: 'English', intl: 'en-US' },
  { id: 'pt', label: 'Português', intl: 'pt-BR' },
  { id: 'es', label: 'Español', intl: 'es-ES' },
  { id: 'zh', label: '中文', intl: 'zh-CN' },
] as const

export type Locale = (typeof LOCALES)[number]['id']
export const DEFAULT_LOCALE: Locale = 'en'

type Dict = Record<string, string>
export type Messages = Record<Locale, Dict>

const dicts: Record<Locale, Dict> = { en: {}, pt: {}, es: {}, zh: {} }
const modules = import.meta.glob<{ default: Messages }>('./messages/*.ts', { eager: true })
for (const [path, mod] of Object.entries(modules)) {
  const ns = path.slice(path.lastIndexOf('/') + 1, -3)
  for (const l of Object.keys(dicts) as Locale[])
    for (const [k, v] of Object.entries(mod.default[l] ?? {})) dicts[l][`${ns}.${k}`] = v
}

const KEY = 'lazyaicode.locale'

function isLocale(x: unknown): x is Locale {
  return LOCALES.some((l) => l.id === x)
}

function initial(): Locale {
  try {
    const v = localStorage.getItem(KEY)
    if (isLocale(v)) return v
  } catch {}
  return DEFAULT_LOCALE
}

const state = $state<{ locale: Locale }>({ locale: initial() })
const listeners = new Set<() => void>()

if (typeof document !== 'undefined') document.documentElement.lang = state.locale

export function getLocale(): Locale {
  return state.locale
}

/** Locale para `Intl` (`pt-BR`, `zh-CN`…). */
export function intlLocale(): string {
  return LOCALES.find((l) => l.id === state.locale)!.intl
}

export function setLocale(l: string | null | undefined) {
  const next = isLocale(l) ? l : DEFAULT_LOCALE
  if (next === state.locale) return
  state.locale = next
  try {
    localStorage.setItem(KEY, next)
  } catch {}
  if (typeof document !== 'undefined') document.documentElement.lang = next
  for (const f of listeners) f()
}

export function onLocaleChange(f: () => void): () => void {
  listeners.add(f)
  return () => listeners.delete(f)
}

/**
 * Texto traduzido. `{nome}` no texto é trocado por `params.nome`. Sem a chave
 * no idioma atual, cai no inglês e, por fim, na própria chave.
 */
export function t(key: string, params?: Record<string, string | number>): string {
  const s = dicts[state.locale][key] ?? dicts.en[key] ?? key
  return params ? s.replace(/\{(\w+)\}/g, (m, k) => (k in params ? String(params[k]) : m)) : s
}
