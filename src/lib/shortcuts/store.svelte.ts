import { getConfig, setConfig } from '../../git/lib/config-store'
import { shortcutLabel, type Overrides } from './shortcuts'

/*
 * Combinações escolhidas pelo usuário, compartilhadas pelas duas UIs: no
 * Svelte, `getOverrides()` lê um `$state` (re-renderiza ao mudar); no React,
 * `useShortcutOverrides()` assina `onOverridesChange`. Persistidas em
 * "shortcuts" no layout.json (mesma chave do gitHydra).
 */

const KEY = 'shortcuts'

let overrides = $state.raw<Overrides>({})
const listeners = new Set<() => void>()
let loading: Promise<void> | null = null

export function getOverrides(): Overrides {
  return overrides
}

export function onOverridesChange(f: () => void): () => void {
  listeners.add(f)
  return () => listeners.delete(f)
}

function apply(next: Overrides) {
  overrides = next
  for (const f of listeners) f()
}

/** Lê as combinações salvas (uma vez; as próximas chamadas reaproveitam). */
export function loadOverrides(): Promise<void> {
  loading ??= getConfig<Overrides>(KEY, {})
    .then((saved) => apply({ ...saved, ...overrides }))
    .catch(() => {})
  return loading
}

function save(next: Overrides) {
  apply(next)
  setConfig(KEY, next).catch(() => {})
}

export const setBinding = (id: string, binding: string) => save({ ...overrides, [id]: binding })

export const resetBinding = (id: string) => save(Object.fromEntries(Object.entries(overrides).filter(([k]) => k !== id)))

export const resetAllBindings = () => save({})

/** Combinação atual da ação, para dicas como "Configurações (Ctrl+,)". Reativa no Svelte. */
export const keyHint = (id: string) => shortcutLabel(id, overrides)
