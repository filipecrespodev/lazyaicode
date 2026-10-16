import { t } from './i18n/index.svelte'

// Converte as mensagens do Claude CLI (stream ao vivo ou histórico .jsonl)
// numa lista de itens para renderizar. Uma única função para as duas origens:
// o histórico é só o stream sem os eventos parciais.

export type Item =
  /** `images`: data URLs das imagens anexadas à mensagem */
  | { kind: 'user'; key: string; text: string; images?: string[] }
  | { kind: 'text'; key: string; msgId: string; text: string; final: boolean }
  | { kind: 'thinking'; key: string; msgId: string; text: string; final: boolean }
  | {
      kind: 'tool'
      key: string
      id: string
      name: string
      input: Record<string, unknown>
      partial: string
      result?: { text: string; isError: boolean }
    }
  | {
      kind: 'permission'
      key: string
      requestId: string
      toolName: string
      toolUseId?: string
      input: Record<string, unknown>
      reason?: string
      /** regras que o CLI sugere para "não perguntar de novo" (`permission_suggestions`) */
      suggestions?: unknown[]
      /** opções do agente ACP (allow_once, allow_always, …) */
      options?: { kind?: string }[]
      decided?: 'allow' | 'always' | 'deny' | 'cancelled'
    }
  | { kind: 'result'; key: string; durationMs: number; costUsd: number; isError: boolean; text?: string }
  | { kind: 'error'; key: string; text: string }
  | { kind: 'notice'; key: string; text: string }

export interface TranscriptState {
  items: Item[]
  /** bloco do stream (`index`) → chave do item, por mensagem em curso */
  blocks: Record<number, string>
  currentMsg: string
  seq: number
}

export function emptyState(): TranscriptState {
  return { items: [], blocks: {}, currentMsg: '', seq: 0 }
}

type Json = Record<string, any>

function nextKey(s: TranscriptState, p: string) {
  s.seq += 1
  return `${p}-${s.seq}`
}

function contentText(c: unknown): string {
  if (typeof c === 'string') return c
  if (Array.isArray(c))
    return c
      .map((b: Json) => (b?.type === 'text' ? b.text : b?.type === 'image' ? t('transcript.image') : ''))
      .filter(Boolean)
      .join('\n')
  return ''
}

/** Imagens (base64) de uma mensagem do usuário, como data URLs. */
function contentImages(c: unknown): string[] {
  if (!Array.isArray(c)) return []
  return c
    .filter((b: Json) => b?.type === 'image' && b.source?.type === 'base64' && b.source.data)
    .map((b: Json) => `data:${b.source.media_type};base64,${b.source.data}`)
}

/** Retira os blocos que o CLI injeta e que não são do usuário. */
export function cleanUserText(t: string): string {
  return t
    .replace(/<system-reminder>[\s\S]*?<\/system-reminder>/g, '')
    .replace(/<local-command-caveat>[\s\S]*?<\/local-command-caveat>/g, '')
    .trim()
}

function findTool(s: TranscriptState, id: string) {
  return s.items.find((i) => i.kind === 'tool' && i.id === id) as Extract<Item, { kind: 'tool' }> | undefined
}

function upsertFinalBlock(s: TranscriptState, msgId: string, kind: 'text' | 'thinking', text: string) {
  const open = s.items.find(
    (i) => (i.kind === 'text' || i.kind === 'thinking') && i.kind === kind && i.msgId === msgId && !i.final,
  ) as Extract<Item, { kind: 'text' | 'thinking' }> | undefined
  if (open) {
    // o texto final é a fonte de verdade; o parcial só serviu para a animação
    if (text) open.text = text
    open.final = true
    return
  }
  if (!text) return
  s.items.push({ kind, key: nextKey(s, kind), msgId, text, final: true })
}

export function apply(s: TranscriptState, msg: Json): void {
  switch (msg.type) {
    case 'stream_event': {
      if (msg.parent_tool_use_id) return
      const ev = msg.event ?? {}
      if (ev.type === 'message_start') {
        s.currentMsg = ev.message?.id ?? nextKey(s, 'm')
        s.blocks = {}
      } else if (ev.type === 'content_block_start') {
        const b = ev.content_block ?? {}
        if (b.type === 'text' || b.type === 'thinking') {
          const key = nextKey(s, b.type)
          s.items.push({ kind: b.type, key, msgId: s.currentMsg, text: b.text ?? b.thinking ?? '', final: false })
          s.blocks[ev.index] = key
        } else if (b.type === 'tool_use') {
          if (!findTool(s, b.id))
            s.items.push({ kind: 'tool', key: nextKey(s, 'tool'), id: b.id, name: b.name, input: {}, partial: '' })
          s.blocks[ev.index] = b.id
        }
      } else if (ev.type === 'content_block_delta') {
        const key = s.blocks[ev.index]
        const d = ev.delta ?? {}
        if (!key) return
        if (d.type === 'text_delta' || d.type === 'thinking_delta') {
          const it = s.items.find((i) => i.key === key) as Extract<Item, { kind: 'text' | 'thinking' }> | undefined
          if (it) it.text += d.text ?? d.thinking ?? ''
        } else if (d.type === 'input_json_delta') {
          const t = findTool(s, key)
          if (t) t.partial += d.partial_json ?? ''
        }
      }
      return
    }
    case 'assistant': {
      if (msg.parent_tool_use_id) return
      const m = msg.message ?? {}
      const msgId: string = m.id ?? msg.uuid ?? nextKey(s, 'm')
      for (const b of m.content ?? []) {
        if (b.type === 'text') upsertFinalBlock(s, msgId, 'text', b.text ?? '')
        else if (b.type === 'thinking') upsertFinalBlock(s, msgId, 'thinking', b.thinking ?? '')
        else if (b.type === 'tool_use') {
          const t = findTool(s, b.id)
          if (t) {
            t.input = b.input ?? {}
            t.partial = ''
          } else s.items.push({ kind: 'tool', key: nextKey(s, 'tool'), id: b.id, name: b.name, input: b.input ?? {}, partial: '' })
        }
      }
      return
    }
    case 'user': {
      if (msg.parent_tool_use_id) return
      const c = msg.message?.content
      if (Array.isArray(c) && c.some((b: Json) => b?.type === 'tool_result')) {
        for (const b of c) {
          if (b?.type !== 'tool_result') continue
          const t = findTool(s, b.tool_use_id)
          if (t) t.result = { text: contentText(b.content), isError: !!b.is_error }
        }
        return
      }
      const images = contentImages(c)
      // com as imagens à mostra, o marcador "[imagem]" do texto sobra
      const text = cleanUserText(images.length ? contentText(c.filter((b: Json) => b?.type !== 'image')) : contentText(c))
      if (text || images.length)
        s.items.push({ kind: 'user', key: msg.uuid ?? nextKey(s, 'user'), text, ...(images.length ? { images } : {}) })
      return
    }
    case 'control_request': {
      const r = msg.request ?? {}
      if (r.subtype !== 'can_use_tool') return
      s.items.push({
        kind: 'permission',
        key: nextKey(s, 'perm'),
        requestId: msg.request_id,
        toolName: r.tool_name,
        toolUseId: r.tool_use_id,
        input: r.input ?? {},
        reason: r.decision_reason ?? r.blocked_path,
        ...(Array.isArray(r.permission_suggestions) && r.permission_suggestions.length ? { suggestions: r.permission_suggestions } : {}),
        ...(Array.isArray(r.options) ? { options: r.options } : {}),
      })
      return
    }
    case 'control_cancel_request': {
      const p = s.items.find((i) => i.kind === 'permission' && i.requestId === msg.request_id)
      if (p && p.kind === 'permission' && !p.decided) p.decided = 'cancelled'
      return
    }
    case 'lazy_notice': {
      s.items.push({ kind: 'notice', key: nextKey(s, 'note'), text: String(msg.text ?? '') })
      return
    }
    case 'result': {
      // permissões pendentes morrem com o turno
      for (const i of s.items) if (i.kind === 'permission' && !i.decided) i.decided = 'cancelled'
      for (const i of s.items) if ((i.kind === 'text' || i.kind === 'thinking') && !i.final) i.final = true
      const isError = !!msg.is_error
      s.items.push({
        kind: 'result',
        key: nextKey(s, 'res'),
        durationMs: msg.duration_ms ?? 0,
        costUsd: msg.total_cost_usd ?? 0,
        isError,
        text: isError ? (msg.result ?? (msg.errors ?? []).join('\n')) : undefined,
      })
      return
    }
  }
}

export function addLocalUser(s: TranscriptState, text: string, images: string[] = []) {
  s.items.push({ kind: 'user', key: nextKey(s, 'local'), text, ...(images.length ? { images } : {}) })
}

export function addError(s: TranscriptState, text: string) {
  s.items.push({ kind: 'error', key: nextKey(s, 'err'), text })
}

/** Uma linha que resume a chamada de ferramenta no cabeçalho do card. */
export function toolSummary(name: string, input: Record<string, any>): string {
  const pick = (k: string) => (typeof input?.[k] === 'string' ? (input[k] as string) : '')
  // agentes ACP (Codex, Gemini) mandam um título pronto para exibir
  const title = pick('title')
  switch (name) {
    case 'Bash':
      return pick('description') || pick('command')
    case 'Read':
    case 'Write':
    case 'Edit':
    case 'MultiEdit':
    case 'NotebookEdit':
      return pick('file_path') || pick('notebook_path')
    case 'Grep':
    case 'Glob':
      return pick('pattern')
    case 'WebFetch':
      return pick('url')
    case 'WebSearch':
      return pick('query')
    case 'Task':
    case 'Agent':
      return pick('description')
    case 'TodoWrite':
      {
      const n = (input?.todos ?? []).length
      return t(n === 1 ? 'transcript.taskOne' : 'transcript.taskOther', { n })
    }
    default:
      return title || (Object.values(input ?? {}).find((v) => typeof v === 'string')?.toString().slice(0, 120) ?? '')
  }
}
