import { describe, expect, it } from 'vitest'
import { apply, emptyState, toolSummary, cleanUserText } from './transcript'

const se = (event: object) => ({ type: 'stream_event', event })

describe('transcript', () => {
  it('shows images attached to a user message, without the [imagem] placeholder', () => {
    const s = emptyState()
    const img = { type: 'image', source: { type: 'base64', media_type: 'image/png', data: 'AAAA' } }
    apply(s, { type: 'user', uuid: 'u1', message: { role: 'user', content: [img, { type: 'text', text: 'o que é isso?' }] } })
    expect(s.items).toEqual([{ kind: 'user', key: 'u1', text: 'o que é isso?', images: ['data:image/png;base64,AAAA'] }])
  })

  it('keeps a user message that has only images', () => {
    const s = emptyState()
    const img = { type: 'image', source: { type: 'base64', media_type: 'image/jpeg', data: 'BBBB' } }
    apply(s, { type: 'user', uuid: 'u2', message: { role: 'user', content: [img] } })
    expect(s.items).toMatchObject([{ kind: 'user', text: '', images: ['data:image/jpeg;base64,BBBB'] }])
  })

  it('streams text deltas and lets the final assistant message settle the text', () => {
    const s = emptyState()
    apply(s, se({ type: 'message_start', message: { id: 'm1' } }))
    apply(s, se({ type: 'content_block_start', index: 0, content_block: { type: 'text', text: '' } }))
    apply(s, se({ type: 'content_block_delta', index: 0, delta: { type: 'text_delta', text: 'Ol' } }))
    apply(s, se({ type: 'content_block_delta', index: 0, delta: { type: 'text_delta', text: 'á' } }))
    expect(s.items).toMatchObject([{ kind: 'text', text: 'Olá', final: false }])
    apply(s, { type: 'assistant', message: { id: 'm1', content: [{ type: 'text', text: 'Olá!' }] } })
    expect(s.items).toHaveLength(1)
    expect(s.items[0]).toMatchObject({ text: 'Olá!', final: true })
  })

  it('does not duplicate blocks when one message arrives split across several assistant events', () => {
    const s = emptyState()
    apply(s, se({ type: 'message_start', message: { id: 'm2' } }))
    apply(s, se({ type: 'content_block_start', index: 0, content_block: { type: 'thinking', thinking: '' } }))
    apply(s, se({ type: 'content_block_start', index: 1, content_block: { type: 'text', text: '' } }))
    apply(s, { type: 'assistant', message: { id: 'm2', content: [{ type: 'thinking', thinking: 'hmm' }] } })
    apply(s, { type: 'assistant', message: { id: 'm2', content: [{ type: 'text', text: 'feito' }] } })
    expect(s.items.map((i) => i.kind)).toEqual(['thinking', 'text'])
  })

  it('attaches tool results to their call and builds the call from partial json', () => {
    const s = emptyState()
    apply(s, se({ type: 'message_start', message: { id: 'm3' } }))
    apply(s, se({ type: 'content_block_start', index: 0, content_block: { type: 'tool_use', id: 't1', name: 'Bash' } }))
    apply(s, se({ type: 'content_block_delta', index: 0, delta: { type: 'input_json_delta', partial_json: '{"comm' } }))
    apply(s, { type: 'assistant', message: { id: 'm3', content: [{ type: 'tool_use', id: 't1', name: 'Bash', input: { command: 'ls' } }] } })
    apply(s, {
      type: 'user',
      message: { role: 'user', content: [{ type: 'tool_result', tool_use_id: 't1', content: 'a.txt', is_error: false }] },
    })
    expect(s.items).toHaveLength(1)
    expect(s.items[0]).toMatchObject({ kind: 'tool', input: { command: 'ls' }, result: { text: 'a.txt' } })
  })

  it('ignores subagent traffic in the main transcript', () => {
    const s = emptyState()
    apply(s, { type: 'assistant', parent_tool_use_id: 'x', message: { id: 'sub', content: [{ type: 'text', text: 'oi' }] } })
    expect(s.items).toHaveLength(0)
  })

  it('turns can_use_tool into a permission card and cancels it when the turn ends', () => {
    const s = emptyState()
    apply(s, {
      type: 'control_request',
      request_id: 'r1',
      request: { subtype: 'can_use_tool', tool_name: 'Bash', input: { command: 'rm -rf x' }, tool_use_id: 't9' },
    })
    expect(s.items[0]).toMatchObject({ kind: 'permission', requestId: 'r1', toolName: 'Bash' })
    apply(s, { type: 'result', duration_ms: 10, total_cost_usd: 0.01 })
    expect(s.items[0]).toMatchObject({ decided: 'cancelled' })
    expect(s.items[1]).toMatchObject({ kind: 'result', isError: false })
  })

  it('renders history user messages given as plain strings, without injected reminders', () => {
    const s = emptyState()
    apply(s, { type: 'user', message: { role: 'user', content: 'faça X<system-reminder>ruído</system-reminder>' } })
    expect(s.items[0]).toMatchObject({ kind: 'user', text: 'faça X' })
    expect(cleanUserText('<system-reminder>a</system-reminder>')).toBe('')
  })

  it('shows agent notices and uses the title ACP agents send for their tools', () => {
    const s = emptyState()
    apply(s, { type: 'lazy_notice', text: 'sessão nova' })
    expect(s.items[0]).toMatchObject({ kind: 'notice', text: 'sessão nova' })
    expect(toolSummary('Execute', { title: 'npm test', cmd: 'x' })).toBe('npm test')
  })

  it('summarises tool calls by their most telling argument', () => {
    expect(toolSummary('Read', { file_path: '/a/b.ts' })).toBe('/a/b.ts')
    expect(toolSummary('Bash', { command: 'ls', description: 'Lista' })).toBe('Lista')
    expect(toolSummary('TodoWrite', { todos: [1, 2] })).toBe('2 tasks')
  })
})
