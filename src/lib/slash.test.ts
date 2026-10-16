import { describe, expect, it } from 'vitest'
import type { CapItem } from './api'
import { rankCommands, slashQuery } from './slash'

const it_ = (name: string, scope: CapItem['scope'], description = '', extra: Partial<CapItem> = {}): CapItem => ({
  kind: 'skill', name, description, scope, plugin: null, path: '', model: null, tools: null, error: null,
  loaded: true, builtin: scope === 'builtin', argument_hint: null, ...extra,
})

describe('slash menu', () => {
  it('opens only while the caret is inside a leading command', () => {
    expect(slashQuery('/rev', 4)).toBe('rev')
    expect(slashQuery('/', 1)).toBe('')
    expect(slashQuery('/trevo-skills:pm', 16)).toBe('trevo-skills:pm')
    expect(slashQuery('/review agora', 13)).toBeNull()
    expect(slashQuery('texto /rev', 10)).toBeNull()
  })

  it('ranks prefix matches first, including the part after the plugin name, and hides unloaded items', () => {
    const items = [
      it_('compact', 'builtin', 'Compacta a conversa'),
      it_('trevo-skills:trevo-pm', 'plugin', 'Board do Azure'),
      it_('pm-report', 'global'),
      it_('deploy', 'project', 'faz deploy e avisa o pm'),
      it_('pm-quebrado', 'project', '', { loaded: false }),
    ]
    expect(rankCommands(items, 'pm').map((i) => i.name)).toEqual(['pm-report', 'trevo-skills:trevo-pm', 'deploy'])
    expect(rankCommands(items, 'trevo').map((i) => i.name)).toEqual(['trevo-skills:trevo-pm'])
    // sem termo: projeto, global, plugin e por fim os embutidos
    expect(rankCommands(items, '').map((i) => i.scope)).toEqual(['project', 'global', 'plugin', 'builtin'])
  })
})
