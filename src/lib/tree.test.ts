import { describe, expect, it } from 'vitest'
import type { Project, Thread } from './api'
import { activityOf, buildGroups, flatten, timeBucket } from './tree'

const NOW = new Date('2026-10-15T15:00:00').getTime()
const H = 3_600_000
const p = (id: string, name: string): Project => ({ id, name, path: `/x/${name}`, created_at: 0, is_git: true })
const t = (id: string, project: string, ago: number, extra: Partial<Thread> = {}): Thread =>
  ({
    id, project_id: project, title: `thread ${id}`, created_at: 0, updated_at: NOW - ago, cwd: '/x', isolated: false,
    branch: null, base_ref: null, session_id: null, model: '', permission_mode: '', archived: false, bookmarked: false,
    unread: false, workspace_id: 'w', claude_dir: null, effort: null, agent: null, add_dirs: [], provider: 'claude',
    status: 'idle', ...extra,
  }) as Thread

const projects = [p('a', 'api'), p('w', 'web'), p('z', 'zeta'), p('b', 'billing')]
const threads = [
  t('1', 'a', 1 * H),
  t('2', 'w', 0.5 * H, { status: 'running' }),
  t('3', 'a', 30 * H, { bookmarked: true }),
  t('4', 'w', 200 * H, { archived: true }),
  t('5', 'a', 50 * 24 * H, { unread: true }),
]

describe('sidebar tree', () => {
  it('lists every folder in alphabetical order; folders without threads start collapsed', () => {
    const g = buildGroups(threads, projects, { mode: 'project', filter: '', archived: false, now: NOW })
    expect(g.map((x) => x.key)).toEqual(['fav', 'p:a', 'p:b', 'p:w', 'p:z'])
    const rows = flatten(g, {}, {}, { perGroup: 8, filtering: false })
    expect(rows.find((r) => r.key === 'p:b')).toMatchObject({ type: 'group', open: false })
    expect(rows.some((r) => r.key === 'p:b:empty')).toBe(false)
    const opened = flatten(g, { 'p:b': false }, {}, { perGroup: 8, filtering: false })
    expect(opened.some((r) => r.key === 'p:b:empty')).toBe(true)
  })

  it('filtering keeps only folders whose name or threads match; archived view only folders with threads', () => {
    expect(buildGroups(threads, projects, { mode: 'project', filter: 'zet', archived: false, now: NOW }).map((x) => x.key)).toEqual(['p:z'])
    expect(buildGroups(threads, projects, { mode: 'project', filter: '', archived: true, now: NOW }).map((x) => x.key)).toEqual(['p:w'])
  })

  it('collapsing a project hides its threads but keeps its activity visible', () => {
    const g = buildGroups(threads, projects, { mode: 'project', filter: '', archived: false, now: NOW })
    const rows = flatten(g, { 'p:w': true }, {}, { perGroup: 8, filtering: false })
    expect(rows.some((r) => r.type === 'thread' && r.thread.id === '2')).toBe(false)
    expect(g.find((x) => x.key === 'p:w')!.activity).toBe('running')
  })

  it('limits each group and reports how many are hidden', () => {
    const many = Array.from({ length: 12 }, (_, i) => t(`m${i}`, 'a', i * H))
    const g = buildGroups(many, projects, { mode: 'project', filter: '', archived: false, now: NOW })
    const rows = flatten(g, {}, {}, { perGroup: 8, filtering: false })
    expect(rows.find((r) => r.type === 'more')).toMatchObject({ hidden: 4 })
    expect(flatten(g, {}, { 'p:a': true }, { perGroup: 8, filtering: false }).some((r) => r.type === 'more')).toBe(false)
  })

  it('filters by title or project name and opens everything while filtering', () => {
    const g = buildGroups(threads, projects, { mode: 'project', filter: 'web', archived: false, now: NOW })
    expect(g.map((x) => x.key)).toEqual(['p:w'])
    const rows = flatten(g, { 'p:w': true }, {}, { perGroup: 8, filtering: true })
    expect(rows.filter((r) => r.type === 'thread')).toHaveLength(1)
  })

  it('groups by date in recent mode', () => {
    expect(timeBucket(NOW - H, NOW).label).toBe('Today')
    expect(timeBucket(NOW - 24 * H, NOW).label).toBe('Yesterday')
    expect(timeBucket(NOW - 50 * 24 * H, NOW).label).toBe('Older')
    const g = buildGroups(threads, projects, { mode: 'recent', filter: '', archived: false, now: NOW })
    expect(g.map((x) => x.label)).toEqual(['Favorites', 'Today', 'Yesterday', 'Older'])
  })

  it('ranks what needs attention first', () => {
    expect(activityOf([t('x', 'a', 0, { unread: true }), t('y', 'a', 0, { status: 'awaiting' })])).toBe('awaiting')
    expect(activityOf([t('x', 'a', 0)])).toBeNull()
  })
})
