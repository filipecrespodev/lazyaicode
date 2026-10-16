import { t } from '../i18n/index.svelte'

/*
 * Atalhos de teclado como dados, para as duas UIs (Svelte e os painéis git em
 * React). Uma combinação é "Mod+Shift+Alt+Tecla" (Mod = Ctrl, ou ⌘ no macOS).
 * Cada ação tem combinações padrão; o usuário pode trocar por uma sua
 * (`Overrides`, salvas em "shortcuts" no layout.json — ver ./store.svelte.ts).
 * Teclas numeradas e de contexto (árvore, Esc) são fixas, mas aparecem na lista.
 */

export const GROUPS = ['general', 'navigation', 'threads', 'git'] as const
export type ShortcutGroup = (typeof GROUPS)[number]

export const groupLabel = (g: ShortcutGroup) => t(`shortcuts.group_${g}`)

export type ShortcutEntry = {
  id: string
  /** Traduzido ao ler, no idioma atual. */
  label: string
  group: ShortcutGroup
}

export type ShortcutAction = ShortcutEntry & {
  /** Every binding that triggers it until the user picks their own. */
  defaults: string[]
}

/** Atalho fixo (não remapeável): só listado. `keys` são exibidas com `formatBinding`. */
export type FixedShortcut = ShortcutEntry & {
  keys: string[]
  /** Combinações que ele ocupa: nenhuma ação pode usá-las. */
  reserves?: RegExp
}

const action = (id: string, group: ShortcutGroup, defaults: string[]): ShortcutAction => ({
  id,
  get label() {
    return t(`shortcuts.${id}`)
  },
  group,
  defaults,
})

const fixed = (id: string, group: ShortcutGroup, keys: string[], reserves?: RegExp): FixedShortcut => ({
  id,
  get label() {
    return t(`shortcuts.${id}`)
  },
  group,
  keys,
  reserves,
})

export const ACTIONS: ShortcutAction[] = [
  action('commandPalette', 'general', ['Mod+K']),
  action('toggleSidebar', 'general', ['Mod+B']),
  action('openSettings', 'general', ['Mod+,']),
  action('showShortcuts', 'general', ['Mod+/']),
  action('toggleChanges', 'navigation', ['Shift+Alt+D']),
  action('toggleTerminal', 'navigation', ['Mod+`']),
  action('prevThread', 'navigation', ['Alt+ArrowUp']),
  action('nextThread', 'navigation', ['Alt+ArrowDown']),
  action('nextWorkspace', 'navigation', ['Mod+Tab']),
  action('prevWorkspace', 'navigation', ['Mod+Shift+Tab']),
  action('newThread', 'threads', ['Mod+N']),
  action('toggleBookmark', 'threads', ['Mod+D']),
  action('toggleActivityLog', 'git', ['Mod+L']),
  action('searchCommits', 'git', ['Mod+F']),
  action('undo', 'git', ['Mod+Z']),
  action('redo', 'git', ['Mod+Shift+Z', 'Mod+Y']),
]

export const FIXED: FixedShortcut[] = [
  fixed('tabs', 'navigation', ['Mod+1…6'], /^Mod\+[1-6]$/),
  fixed('workspaceN', 'navigation', ['Alt+1…9'], /^Alt\+[1-9]$/),
  fixed('tree', 'threads', ['↑ ↓', '← →', 'Enter']),
  fixed('filterTree', 'threads', ['/']),
  fixed('interrupt', 'threads', ['Escape']),
]

/** What the user changed: action id → its single replacement binding. */
export type Overrides = Record<string, string>

type KeyEventLike = { key: string; ctrlKey: boolean; metaKey: boolean; shiftKey: boolean; altKey: boolean }

const MODIFIER_KEYS = new Set(['Control', 'Meta', 'Shift', 'Alt', 'AltGraph'])

/** The binding an event would trigger, or null for a bare modifier press. */
export function eventToBinding(e: KeyEventLike): string | null {
  if (MODIFIER_KEYS.has(e.key)) return null
  const key = e.key === ' ' ? 'Space' : e.key.length === 1 ? e.key.toUpperCase() : e.key
  return [(e.ctrlKey || e.metaKey) && 'Mod', e.shiftKey && 'Shift', e.altKey && 'Alt', key].filter(Boolean).join('+')
}

export const matches = (e: KeyEventLike, binding: string) => eventToBinding(e) === binding

/** Whether a recorded binding is allowed: it needs Ctrl/⌘ or Alt (or be a function key), so typing never triggers it. */
export function isUsableBinding(binding: string): boolean {
  return /^(Mod|(Shift\+)?Alt)\+/.test(binding) || /^(Shift\+)?F\d{1,2}$/.test(binding)
}

export function bindingsFor(id: string, overrides: Overrides): string[] {
  if (overrides[id]) return [overrides[id]]
  return ACTIONS.find((a) => a.id === id)?.defaults ?? []
}

/** A ação cuja combinação atual é a do evento (as combinações não se repetem). */
export function actionFor(e: KeyEventLike, overrides: Overrides): string | null {
  const b = eventToBinding(e)
  if (!b) return null
  return ACTIONS.find((a) => bindingsFor(a.id, overrides).includes(b))?.id ?? null
}

/** Another action (or a fixed shortcut) already using `binding`, if any. */
export function findConflict(id: string, binding: string, overrides: Overrides): ShortcutEntry | null {
  return (
    ACTIONS.find((a) => a.id !== id && bindingsFor(a.id, overrides).includes(binding)) ??
    FIXED.find((f) => f.reserves?.test(binding)) ??
    null
  )
}

const KEY_NAMES: Record<string, string> = {
  ArrowUp: '↑',
  ArrowDown: '↓',
  ArrowLeft: '←',
  ArrowRight: '→',
  Escape: 'Esc',
}

const isMac = () => /Mac|iPhone|iPad/.test(globalThis.navigator?.platform ?? '')

export function formatBinding(binding: string, mac = isMac()): string {
  return binding
    .split('+')
    .map((part) =>
      part === 'Mod' ? (mac ? '⌘' : 'Ctrl') : part === 'Alt' && mac ? '⌥' : part === 'Shift' && mac ? '⇧' : (KEY_NAMES[part] ?? part),
    )
    .join(mac ? '' : '+')
}

/** Combinações atuais da ação, formatadas ("Ctrl+Shift+Z / Ctrl+Y"). */
export const shortcutLabel = (id: string, overrides: Overrides) =>
  bindingsFor(id, overrides)
    .map((b) => formatBinding(b))
    .join(' / ')
