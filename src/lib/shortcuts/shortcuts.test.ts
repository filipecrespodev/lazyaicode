import { describe, expect, it } from 'vitest'
import {
  ACTIONS,
  FIXED,
  GROUPS,
  actionFor,
  bindingsFor,
  eventToBinding,
  findConflict,
  formatBinding,
  isUsableBinding,
  matches,
} from './shortcuts'
import { t } from '../i18n/index.svelte'

const ev = (key: string, mods: Partial<{ ctrlKey: boolean; metaKey: boolean; shiftKey: boolean; altKey: boolean }> = {}) => ({
  key,
  ctrlKey: false,
  metaKey: false,
  shiftKey: false,
  altKey: false,
  ...mods,
})

describe('eventToBinding / matches', () => {
  it('treats Ctrl and Meta as Mod and normalizes letters', () => {
    expect(eventToBinding(ev('p', { ctrlKey: true }))).toBe('Mod+P')
    expect(eventToBinding(ev('p', { metaKey: true }))).toBe('Mod+P')
    expect(eventToBinding(ev('Z', { ctrlKey: true, shiftKey: true }))).toBe('Mod+Shift+Z')
    expect(eventToBinding(ev(' ', { altKey: true }))).toBe('Alt+Space')
    expect(eventToBinding(ev('D', { altKey: true, shiftKey: true }))).toBe('Shift+Alt+D')
  })
  it('ignores bare modifiers', () => {
    expect(eventToBinding(ev('Control', { ctrlKey: true }))).toBeNull()
  })
  it('matches exactly, extra modifiers included', () => {
    expect(matches(ev('z', { ctrlKey: true }), 'Mod+Z')).toBe(true)
    expect(matches(ev('Z', { ctrlKey: true, shiftKey: true }), 'Mod+Z')).toBe(false)
  })
})

describe('bindings', () => {
  it('uses defaults until overridden, and an override replaces all of them', () => {
    expect(bindingsFor('redo', {})).toEqual(['Mod+Shift+Z', 'Mod+Y'])
    expect(bindingsFor('redo', { redo: 'Alt+R' })).toEqual(['Alt+R'])
  })
  it('finds conflicts with other actions only', () => {
    expect(findConflict('newThread', 'Mod+K', {})?.id).toBe('commandPalette')
    expect(findConflict('commandPalette', 'Mod+K', {})).toBeNull()
    expect(findConflict('newThread', 'Mod+K', { commandPalette: 'Mod+J' })).toBeNull()
  })
  it('treats the fixed numbered keys as taken', () => {
    expect(findConflict('newThread', 'Mod+3', {})?.id).toBe('tabs')
    expect(findConflict('newThread', 'Alt+9', {})?.id).toBe('workspaceN')
    expect(findConflict('newThread', 'Mod+9', {})).toBeNull()
  })
  it('has unique default bindings, none of them reserved or untypable', () => {
    const all = ACTIONS.flatMap((a) => a.defaults)
    expect(new Set(all).size).toBe(all.length)
    for (const b of all) {
      expect(isUsableBinding(b), b).toBe(true)
      expect(FIXED.some((f) => f.reserves?.test(b)), b).toBe(false)
    }
  })
  it('only allows bindings that can’t be typed as text', () => {
    expect(isUsableBinding('Mod+K')).toBe(true)
    expect(isUsableBinding('Shift+Alt+D')).toBe(true)
    expect(isUsableBinding('F5')).toBe(true)
    expect(isUsableBinding('K')).toBe(false)
    expect(isUsableBinding('Shift+K')).toBe(false)
  })
  it('formats for the platform', () => {
    expect(formatBinding('Mod+Shift+Z', false)).toBe('Ctrl+Shift+Z')
    expect(formatBinding('Mod+Shift+Z', true)).toBe('⌘⇧Z')
    expect(formatBinding('Alt+ArrowUp', false)).toBe('Alt+↑')
    expect(formatBinding('Escape', false)).toBe('Esc')
  })
})

describe('app actions', () => {
  // as teclas que o app usava antes do registro continuam as mesmas
  const cases: [string, ReturnType<typeof ev>][] = [
    ['commandPalette', ev('k', { ctrlKey: true })],
    ['toggleSidebar', ev('b', { ctrlKey: true })],
    ['openSettings', ev(',', { ctrlKey: true })],
    ['showShortcuts', ev('/', { ctrlKey: true })],
    ['newThread', ev('n', { ctrlKey: true })],
    ['toggleBookmark', ev('d', { ctrlKey: true })],
    ['toggleChanges', ev('D', { altKey: true, shiftKey: true })],
    ['toggleTerminal', ev('`', { ctrlKey: true })],
    ['prevThread', ev('ArrowUp', { altKey: true })],
    ['nextThread', ev('ArrowDown', { altKey: true })],
    ['nextWorkspace', ev('Tab', { ctrlKey: true })],
    ['prevWorkspace', ev('Tab', { ctrlKey: true, shiftKey: true })],
    ['toggleActivityLog', ev('l', { ctrlKey: true })],
    ['searchCommits', ev('f', { ctrlKey: true })],
    ['undo', ev('z', { ctrlKey: true })],
    ['redo', ev('Z', { ctrlKey: true, shiftKey: true })],
    ['redo', ev('y', { ctrlKey: true })],
  ]
  it.each(cases)('%s fires with its default keys', (id, e) => {
    expect(actionFor(e, {})).toBe(id)
  })
  it('follows overrides: the new keys fire it, the old ones no longer do', () => {
    const o = { commandPalette: 'Mod+Shift+P' }
    expect(actionFor(ev('P', { ctrlKey: true, shiftKey: true }), o)).toBe('commandPalette')
    expect(actionFor(ev('k', { ctrlKey: true }), o)).toBeNull()
  })
  it('ignores plain typing and the fixed keys', () => {
    expect(actionFor(ev('k'), {})).toBeNull()
    expect(actionFor(ev('/'), {})).toBeNull()
    expect(actionFor(ev('1', { ctrlKey: true }), {})).toBeNull()
    expect(actionFor(ev('Escape'), {})).toBeNull()
  })
  it('every action and fixed shortcut has a group and a translated label', () => {
    for (const a of [...ACTIONS, ...FIXED]) {
      expect(GROUPS).toContain(a.group)
      expect(a.label, a.id).not.toBe(`shortcuts.${a.id}`)
    }
    for (const g of GROUPS) expect(t(`shortcuts.group_${g}`)).not.toBe(`shortcuts.group_${g}`)
  })
})
