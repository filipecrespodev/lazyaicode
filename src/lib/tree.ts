import type { Project, Thread } from './api'
import { t } from './i18n/index.svelte'

// Árvore da barra lateral: agrupa as threads (por projeto ou por data),
// decide o que começa aberto e achata o que está visível numa lista de
// linhas — a lista é o que a navegação por teclado percorre.

export type TreeMode = 'project' | 'recent'
export type Activity = 'awaiting' | 'running' | 'error' | 'unread' | null

export interface Group {
  key: string
  label: string
  kind: 'favorites' | 'project' | 'time' | 'idle'
  project?: Project
  threads: Thread[]
  children?: Group[]
  /** o que chama atenção dentro do grupo, para aparecer mesmo recolhido */
  activity: Activity
}

export type Row =
  | { type: 'group'; key: string; group: Group; depth: number; open: boolean }
  | { type: 'thread'; key: string; thread: Thread; depth: number; parent: string }
  | { type: 'more'; key: string; parent: string; hidden: number; depth: number }
  | { type: 'empty'; key: string; parent: string; depth: number }

const ACTIVITY_ORDER: Activity[] = ['awaiting', 'running', 'error', 'unread']

export function activityOf(threads: Thread[]): Activity {
  for (const a of ACTIVITY_ORDER) {
    if (threads.some((t) => (a === 'unread' ? t.unread && t.status === 'idle' : t.status === a))) return a
  }
  return null
}

const DAY = 86_400_000

function startOfDay(ms: number) {
  const d = new Date(ms)
  d.setHours(0, 0, 0, 0)
  return d.getTime()
}

/** Faixa de data de uma thread no modo Recentes. */
export function timeBucket(updated: number, now = Date.now()): { key: string; label: string } {
  const today = startOfDay(now)
  if (updated >= today) return { key: 'today', label: t('tree.today') }
  if (updated >= today - DAY) return { key: 'yesterday', label: t('tree.yesterday') }
  if (updated >= today - 6 * DAY) return { key: 'week', label: t('tree.last7Days') }
  if (updated >= today - 29 * DAY) return { key: 'month', label: t('tree.last30Days') }
  return { key: 'older', label: t('tree.older') }
}

/** Pastas em ordem alfabética, sem diferenciar maiúsculas; `repo2` antes de `repo10`. */
const byName = (a: Project, b: Project) => a.name.localeCompare(b.name, undefined, { sensitivity: 'base', numeric: true })

export function buildGroups(
  threads: Thread[],
  projects: Project[],
  opts: { mode: TreeMode; filter: string; archived: boolean; now?: number },
): Group[] {
  const q = opts.filter.trim().toLowerCase()
  const projectName = new Map(projects.map((p) => [p.id, p.name]))
  const visible = threads
    .filter((t) => t.archived === opts.archived)
    .filter((t) => !q || t.title.toLowerCase().includes(q) || (projectName.get(t.project_id) ?? '').toLowerCase().includes(q))
    .sort((a, b) => b.updated_at - a.updated_at)

  const groups: Group[] = []
  const favs = visible.filter((t) => t.bookmarked)
  if (favs.length && !opts.archived) groups.push({ key: 'fav', label: t('tree.favorites'), kind: 'favorites', threads: favs, activity: activityOf(favs) })

  if (opts.mode === 'recent') {
    const buckets = new Map<string, Group>()
    for (const t of visible) {
      const b = timeBucket(t.updated_at, opts.now)
      if (!buckets.has(b.key)) buckets.set(b.key, { key: `time:${b.key}`, label: b.label, kind: 'time', threads: [], activity: null })
      buckets.get(b.key)!.threads.push(t)
    }
    for (const g of buckets.values()) groups.push({ ...g, activity: activityOf(g.threads) })
    return groups
  }

  const byProject = new Map<string, Thread[]>()
  for (const t of visible) byProject.set(t.project_id, [...(byProject.get(t.project_id) ?? []), t])
  // todas as pastas do workspace, em ordem alfabética; as sem thread
  // começam recolhidas (ver isOpen). Filtrando, só entram as que casam.
  const shown = projects
    .filter((p) => byProject.has(p.id) || (!opts.archived && (!q || p.name.toLowerCase().includes(q))))
    .sort(byName)
  for (const p of shown) {
    const ts = byProject.get(p.id) ?? []
    groups.push({ key: `p:${p.id}`, label: p.name, kind: 'project', project: p, threads: ts, activity: activityOf(ts) })
  }
  return groups
}

/** Aberto por padrão: tudo, menos o grupo de repositórios sem thread. */
export function isOpen(g: Group, collapsed: Record<string, boolean>, filtering: boolean) {
  if (filtering) return true
  return !(collapsed[g.key] ?? (g.kind === 'idle' || (g.kind === 'project' && !g.threads.length)))
}

export function flatten(
  groups: Group[],
  collapsed: Record<string, boolean>,
  showAll: Record<string, boolean>,
  opts: { perGroup: number; filtering: boolean },
): Row[] {
  const rows: Row[] = []
  const walk = (g: Group, depth: number) => {
    const open = isOpen(g, collapsed, opts.filtering)
    rows.push({ type: 'group', key: g.key, group: g, depth, open })
    if (!open) return
    if (g.children) {
      for (const c of g.children) walk(c, depth + 1)
      return
    }
    if (!g.threads.length) {
      rows.push({ type: 'empty', key: `${g.key}:empty`, parent: g.key, depth: depth + 1 })
      return
    }
    const limit = showAll[g.key] || opts.filtering ? g.threads.length : opts.perGroup
    for (const t of g.threads.slice(0, limit)) rows.push({ type: 'thread', key: `${g.key}/${t.id}`, thread: t, depth: depth + 1, parent: g.key })
    if (g.threads.length > limit) rows.push({ type: 'more', key: `${g.key}:more`, parent: g.key, hidden: g.threads.length - limit, depth: depth + 1 })
  }
  for (const g of groups) walk(g, 0)
  return rows
}
