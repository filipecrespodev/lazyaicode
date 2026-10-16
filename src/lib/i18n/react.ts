import { useSyncExternalStore } from 'react'
import { getLocale, onLocaleChange, t } from './index.svelte'

/** Re-renderiza o componente ao trocar de idioma e devolve `t`. */
export function useT(): typeof t {
  useSyncExternalStore(onLocaleChange, getLocale)
  return t
}

export { t, getLocale, intlLocale } from './index.svelte'
