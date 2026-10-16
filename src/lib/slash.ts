import type { CapItem } from './api'

/** Texto digitado depois da `/` quando o composer começa com um comando e o
 *  cursor ainda está nele (`/rev|`); `null` quando o menu não deve abrir. */
export function slashQuery(text: string, caret: number): string | null {
  const m = /^\/([\w:.\-]*)$/.exec(text.slice(0, caret))
  return m ? m[1] : null
}

const SCOPE_RANK: Record<string, number> = { project: 0, global: 1, plugin: 2, other: 3, builtin: 4 }

/** Itens carregados que casam com a busca: nome começando com o termo (também
 *  depois do `plugin:`) primeiro, depois nome contendo, depois descrição. */
export function rankCommands(items: CapItem[], q: string, limit = 60): CapItem[] {
  const query = q.toLowerCase()
  const score = (i: CapItem) => {
    if (!query) return 1
    const name = i.name.toLowerCase()
    const short = name.includes(':') ? name.slice(name.indexOf(':') + 1) : name
    if (name.startsWith(query) || short.startsWith(query)) return 4
    if (name.includes(query)) return 3
    if (i.description.toLowerCase().includes(query)) return 1
    return 0
  }
  return items
    .filter((i) => i.loaded && i.kind !== 'agent')
    .map((i) => ({ i, s: score(i) }))
    .filter((x) => x.s > 0)
    .sort((a, b) => b.s - a.s || (SCOPE_RANK[a.i.scope] ?? 9) - (SCOPE_RANK[b.i.scope] ?? 9) || a.i.name.localeCompare(b.i.name))
    .slice(0, limit)
    .map((x) => x.i)
}
