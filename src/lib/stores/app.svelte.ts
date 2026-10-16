import { listen } from '@tauri-apps/api/event'
import {
  api,
  errText,
  type AuthInfo,
  type Capabilities,
  type DiffStats,
  type McpServer,
  type Profile,
  type ProviderId,
  type AcpSession,
  type Project,
  type Settings,
  type Status,
  type Thread,
  type Workspace,
} from '../api'
import { addError, addLocalUser, apply, emptyState, type TranscriptState } from '../transcript'
import { dataUrl } from '../images'
import { setLocale, t as tx } from '../i18n/index.svelte'
import {
  notifyFinished,
  rememberWindow,
  rememberProviderModels,
  contextFromResult,
  contextFromUsage,
  type ContextUse,
} from '../session'

type Toast = { id: number; text: string; kind: 'info' | 'error' | 'ok' }
/** Abas da thread/repositório na ordem dos atalhos Ctrl+1…6. As mudanças
 * locais ficam no Histórico (linha de mudanças → staging e commit). */
export const MAIN_TABS = ['chat', 'history', 'tree', 'reflog', 'stats', 'terminal'] as const
export type MainTab = (typeof MAIN_TABS)[number]
/** Abas que são painéis Git (ilhas React). */
export type GitMode = Exclude<MainTab, 'chat' | 'terminal'>
export const isGitTab = (t: MainTab): t is GitMode => t !== 'chat' && t !== 'terminal'
/** Abas de uma pasta sem `.git`. */
export const PLAIN_TABS: readonly MainTab[] = ['chat', 'terminal']

export type SettingsTab =
  | 'workspaces' | 'appearance' | 'skills' | 'agents' | 'mcp' | 'accounts' | 'providers' | 'defaults' | 'git' | 'integrations' | 'shortcuts'

/** Chave de conta usada nos mapas de login: `''` = `~/.claude`. */
export function accountKey(dir: string | null | undefined) {
  return dir ?? ''
}

const OPEN_WS_KEY = 'openWorkspaces'

function readList(key: string): string[] {
  try {
    const v = JSON.parse(localStorage.getItem(key) ?? '[]')
    return Array.isArray(v) ? v.filter((x) => typeof x === 'string') : []
  } catch {
    return []
  }
}

function readFlag(key: string, fallback: boolean) {
  try {
    const v = localStorage.getItem(key)
    return v === null ? fallback : v === '1'
  } catch {
    return fallback
  }
}

class AppState {
  workspaces = $state<Workspace[]>([])
  activeWorkspaceId = $state<string | null>(null)
  /** workspaces abertos como abas no topo, na ordem das abas (os demais ficam
   *  só na escolha de workspaces); lembrados entre aberturas do app */
  openWorkspaceIds = $state<string[]>(readList(OPEN_WS_KEY))
  /** tela de escolha de quais workspaces abrir (ao abrir o app ou pelo "+") */
  pickingWorkspaces = $state(false)
  /** já há um workspace carregado na tela (não é a escolha da abertura do app) */
  workspaceShown = $state(false)
  projects = $state<Project[]>([])
  threads = $state<Thread[]>([])
  selectedId = $state<string | null>(null)
  transcripts = $state<Record<string, TranscriptState>>({})
  settings = $state<Settings | null>(null)
  claudeBinary = $state<string | null>(null)
  defaultClaudeDir = $state('')
  suggestedClaudeDir = $state('')
  /** login por conta (diretório Claude) */
  auth = $state<Record<string, AuthInfo | null>>({})
  /** aba aberta na thread (ou na tela do repositório) */
  tab = $state<MainTab>('chat')
  /** repositório aberto na tela do repositório (sem thread selecionada) */
  repoProjectId = $state<string | null>(null)
  sidebarOpen = $state(readFlag('sidebarOpen', true))
  paletteOpen = $state(false)
  settingsOpen = $state(false)
  settingsTab = $state<SettingsTab>('workspaces')
  /** workspace em edição na aba Workspaces das configurações */
  settingsWorkspaceId = $state<string | null>(null)
  newWorkspaceOpen = $state(false)
  showArchived = $state(false)
  toasts = $state<Toast[]>([])
  /** incrementa a cada fim de turno: o painel de mudanças observa para recarregar */
  changesTick = $state(0)
  creating = $state(false)
  loading = $state(false)
  /** skills/agentes/MCP por `workspace|pasta` (pasta vazia = só global) */
  capabilities = $state<Record<string, Capabilities | undefined>>({})
  capabilitiesLoading = $state<Record<string, boolean>>({})
  capabilitiesError = $state<Record<string, string>>({})
  /** MCPs da sessão aberta de cada thread (do `system init` e do /mcp) */
  threadMcp = $state<Record<string, McpServer[] | { name: string; status: string }[]>>({})
  profile = $state<Profile | null>(null)
  /** contexto usado / janela, por thread */
  context = $state<Record<string, ContextUse>>({})
  diffStats = $state<Record<string, DiffStats>>({})
  /** sessão ACP aberta por thread: modos, modelos e comandos do agente */
  acpSession = $state<Record<string, AcpSession & { commands?: { name: string; description: string; hint?: string }[] }>>({})

  activeWorkspace = $derived(this.workspaces.find((w) => w.id === this.activeWorkspaceId) ?? null)
  openWorkspaces = $derived(
    this.openWorkspaceIds.map((id) => this.workspaces.find((w) => w.id === id)).filter((w): w is Workspace => !!w),
  )
  closedWorkspaces = $derived(this.workspaces.filter((w) => !this.openWorkspaceIds.includes(w.id)))
  selected = $derived(this.threads.find((t) => t.id === this.selectedId) ?? null)
  selectedProject = $derived(
    this.selected ? (this.projects.find((p) => p.id === this.selected!.project_id) ?? null) : null,
  )

  private toastSeq = 0
  /** thread aberta por workspace, para voltar a ela ao trocar de aba */
  private lastSelected: Record<string, string | null> = {}

  toast(text: string, kind: Toast['kind'] = 'info') {
    const id = ++this.toastSeq
    this.toasts.push({ id, text, kind })
    setTimeout(() => (this.toasts = this.toasts.filter((t) => t.id !== id)), kind === 'error' ? 8000 : 3500)
  }

  async init() {
    await this.refreshSettings()
    const { workspaces, active } = await api.listWorkspaces()
    this.workspaces = workspaces
    for (const w of workspaces) this.refreshAuth(w.claude_dir)
    this.activeWorkspaceId = active ?? null
    this.setOpenWorkspaces(this.openWorkspaceIds.filter((id) => workspaces.some((w) => w.id === id)))
    // com mais de um, a pessoa escolhe quais abrir (já marcados os da última vez)
    if (workspaces.length > 1) this.pickingWorkspaces = true
    else if (workspaces[0]) await this.switchWorkspace(workspaces[0].id)

    api.userProfile().then((p) => (this.profile = p)).catch(() => {})
    await listen<{ threadId: string; msg: any }>('agent-event', ({ payload }) => {
      const m = payload.msg
      if (m?.type === 'result') {
        const c = contextFromResult(m)
        if (c) {
          this.context[payload.threadId] = c
          rememberWindow(c.model, c.window)
        }
        this.onFinished(payload.threadId, m)
      } else if (m?.type === 'control_request' && m.request?.subtype === 'can_use_tool') {
        this.onNeedsPermission(payload.threadId, m.request.tool_name)
      }
      if (m?.type === 'system' && m.subtype === 'init' && Array.isArray(m.mcp_servers))
        this.threadMcp[payload.threadId] = m.mcp_servers
      if (m?.type === 'system' && m.subtype === 'init' && m.acp) {
        this.acpSession[payload.threadId] = { ...this.acpSession[payload.threadId], ...m.acp }
        rememberProviderModels(m.provider, m.acp.models)
      }
      if (m?.type === 'lazy_commands') {
        const cur = this.acpSession[payload.threadId] ?? { modes: [], currentMode: null, models: [], currentModel: null }
        this.acpSession[payload.threadId] = { ...cur, commands: m.commands }
      }
      if (m?.type === 'lazy_config' && this.acpSession[payload.threadId]) {
        const cur = this.acpSession[payload.threadId]
        if (m.currentModeId) cur.currentMode = m.currentModeId
        for (const c of m.configOptions ?? []) {
          if (c.category === 'mode') cur.currentMode = c.currentValue
          if (c.category === 'model') cur.currentModel = c.currentValue
        }
      }
      if (m?.type === 'lazy_usage' && m.size) {
        this.context[payload.threadId] = { used: m.used, window: m.size, model: m.model || '' }
      }
      apply(this.transcriptFor(payload.threadId), payload.msg)
      if (payload.msg?.type === 'result') this.changesTick++
    })
    await listen<{ threadId: string; status: Status }>('thread-status', ({ payload }) => {
      const t = this.threads.find((x) => x.id === payload.threadId)
      if (t) t.status = payload.status
    })
    await listen<Thread>('thread-updated', ({ payload }) => {
      if (payload.workspace_id === this.activeWorkspaceId) this.upsert(payload)
    })
    await listen<{ threadId: string; code: number | null; stderr: string }>('agent-exit', ({ payload }) => {
      if (payload.stderr) addError(this.transcriptFor(payload.threadId), payload.stderr.trim().slice(-2000))
      this.refreshThreads()
    })
    window.addEventListener('focus', () => this.syncSessions())
  }

  private attention(threadId: string) {
    return typeof document === 'undefined' || !document.hasFocus() || this.selectedId !== threadId
  }

  private threadTitle(id: string) {
    return this.threads.find((t) => t.id === id)?.title ?? tx('app.thread')
  }

  /** Avisa quando um prompt termina numa thread que você não está olhando. */
  onFinished(threadId: string, result: any) {
    this.refreshDiffStats(threadId)
    if (!this.settings || !this.attention(threadId)) return
    const tr = this.transcripts[threadId]
    const lastText = tr ? [...tr.items].reverse().find((i) => i.kind === 'text') : undefined
    const body = result?.is_error
      ? result?.result
        ? tx('app.finishedWithErrorDetail', { detail: String(result.result).slice(0, 120) })
        : tx('app.finishedWithError')
      : lastText && 'text' in lastText
        ? lastText.text.replace(/[#*`_>]/g, '').trim().slice(0, 160)
        : tx('app.done')
    notifyFinished(`✓ ${this.threadTitle(threadId)}`, body, this.settings.notify_on_finish, this.settings.sound_on_finish)
  }

  onNeedsPermission(threadId: string, tool: string) {
    if (!this.settings || !this.attention(threadId)) return
    notifyFinished(`⏸ ${this.threadTitle(threadId)}`, tx('app.needsPermission', { tool }), this.settings.notify_on_finish, this.settings.sound_on_finish)
  }

  async refreshDiffStats(id: string) {
    try {
      this.diffStats[id] = await api.diffStats(id)
    } catch {
      delete this.diffStats[id]
    }
  }

  transcriptFor(id: string): TranscriptState {
    if (!this.transcripts[id]) this.transcripts[id] = emptyState()
    return this.transcripts[id]
  }

  upsert(t: Thread) {
    t.provider ??= 'claude'
    const i = this.threads.findIndex((x) => x.id === t.id)
    if (i >= 0) this.threads[i] = t
    else this.threads.push(t)
  }

  async refreshSettings() {
    const s = await api.getSettings()
    this.settings = s.settings
    setLocale(s.settings.language)
    this.claudeBinary = s.claudeBinary
    this.defaultClaudeDir = s.defaultClaudeDir
    this.suggestedClaudeDir = s.suggestedClaudeDir
  }

  async refreshAuth(dir: string | null) {
    const key = accountKey(dir)
    this.auth[key] = null
    try {
      this.auth[key] = await api.authStatus(dir)
    } catch (e) {
      this.auth[key] = { loggedIn: false, error: errText(e) }
    }
  }

  authOf(w: Workspace | null): AuthInfo | null | undefined {
    return w ? this.auth[accountKey(w.claude_dir)] : undefined
  }

  /** Nome curto de uma conta, para rótulos: `~/.claude-trevo`. */
  accountLabel(dir: string | null | undefined) {
    return (dir || this.defaultClaudeDir).replace(/^\/home\/[^/]+/, '~').replace(/^\/Users\/[^/]+/, '~')
  }

  private setOpenWorkspaces(ids: string[]) {
    this.openWorkspaceIds = ids
    try {
      localStorage.setItem(OPEN_WS_KEY, JSON.stringify(ids))
    } catch {}
  }

  /** Abre os workspaces escolhidos na tela de escolha; fica ativo o que já
   *  estava (se continuar aberto) ou o primeiro. */
  async openWorkspacesFrom(ids: string[]) {
    if (!ids.length) return
    this.setOpenWorkspaces(ids)
    this.pickingWorkspaces = false
    const active = this.activeWorkspaceId
    if (this.workspaceShown && active && ids.includes(active)) return
    await this.switchWorkspace(active && ids.includes(active) ? active : ids[0])
  }

  /** Fecha a aba do workspace (as threads dele seguem rodando). Fechando a
   *  última, volta para a escolha de workspaces. */
  async closeWorkspace(id: string) {
    const i = this.openWorkspaceIds.indexOf(id)
    if (i < 0) return
    const rest = this.openWorkspaceIds.filter((x) => x !== id)
    this.setOpenWorkspaces(rest)
    if (id !== this.activeWorkspaceId) return
    const next = rest[Math.min(i, rest.length - 1)]
    if (next) await this.switchWorkspace(next)
    else {
      this.lastSelected[id] = this.selectedId
      this.selectedId = null
      this.repoProjectId = null
      this.threads = []
      this.projects = []
      this.workspaceShown = false
      this.pickingWorkspaces = true
    }
  }

  async switchWorkspace(id: string) {
    if (!this.openWorkspaceIds.includes(id)) this.setOpenWorkspaces([...this.openWorkspaceIds, id])
    this.pickingWorkspaces = false
    this.workspaceShown = true
    if (this.activeWorkspaceId) this.lastSelected[this.activeWorkspaceId] = this.selectedId
    this.activeWorkspaceId = id
    this.selectedId = null
    this.repoProjectId = null
    this.projects = []
    this.threads = []
    api.setActiveWorkspace(id).catch(() => {})
    this.loading = true
    try {
      await this.refreshAll()
      const back = this.lastSelected[id]
      if (back && this.threads.some((t) => t.id === back)) await this.select(back)
    } catch (e) {
      this.toast(errText(e), 'error')
    } finally {
      this.loading = false
    }
    await this.syncSessions(true)
  }

  async createWorkspace(path: string, name: string | null, claudeDir: string | null) {
    try {
      const w = await api.createWorkspace(path, name, claudeDir)
      this.workspaces.push(w)
      this.refreshAuth(w.claude_dir)
      await this.switchWorkspace(w.id)
      return w
    } catch (e) {
      this.toast(errText(e), 'error')
      return null
    }
  }

  async updateWorkspace(id: string, patch: { name?: string; claudeDir?: string | null; ignoredPaths?: string[] }) {
    try {
      const w = await api.updateWorkspace(id, patch)
      const i = this.workspaces.findIndex((x) => x.id === id)
      if (i >= 0) this.workspaces[i] = w
      if (patch.claudeDir !== undefined) this.refreshAuth(w.claude_dir)
      if (id === this.activeWorkspaceId && (patch.claudeDir !== undefined || patch.ignoredPaths)) {
        await this.refreshAll()
        await this.syncSessions(true)
      }
      return w
    } catch (e) {
      this.toast(errText(e), 'error')
      return null
    }
  }

  async removeWorkspace(id: string) {
    try {
      await api.removeWorkspace(id)
      this.workspaces = this.workspaces.filter((w) => w.id !== id)
      const rest = this.openWorkspaceIds.filter((x) => x !== id)
      this.setOpenWorkspaces(rest)
      if (this.activeWorkspaceId === id) {
        this.activeWorkspaceId = null
        this.threads = []
        this.projects = []
        this.selectedId = null
        const next = rest[0] ?? this.workspaces[0]?.id
        if (next) await this.switchWorkspace(next)
      }
    } catch (e) {
      this.toast(errText(e), 'error')
    }
  }

  toggleSidebar(open = !this.sidebarOpen) {
    this.sidebarOpen = open
    try {
      localStorage.setItem('sidebarOpen', open ? '1' : '0')
    } catch {}
  }

  openSettings(tab: SettingsTab = 'workspaces', workspaceId?: string) {
    this.settingsTab = tab
    this.settingsWorkspaceId = workspaceId ?? this.activeWorkspaceId
    this.settingsOpen = true
  }

  capKey(path: string | null, ws = this.activeWorkspaceId) {
    return `${ws}|${path ?? ''}`
  }

  /** Skills, agentes e MCP de uma pasta do workspace ativo (cache de 2 min no backend). */
  async loadCapabilities(path: string | null, force = false) {
    const ws = this.activeWorkspaceId
    if (!ws) return
    const key = this.capKey(path, ws)
    if (this.capabilitiesLoading[key] || (this.capabilities[key] && !force)) return this.capabilities[key]
    this.capabilitiesLoading[key] = true
    delete this.capabilitiesError[key]
    try {
      const c = await api.getCapabilities(ws, path, force)
      this.capabilities[key] = c
      return c
    } catch (e) {
      this.capabilitiesError[key] = errText(e)
    } finally {
      this.capabilitiesLoading[key] = false
    }
  }

  private lastSync = 0

  /**
   * Importa as sessões do Claude Code feitas fora do app (terminal, IDE) na
   * conta e pasta do workspace ativo, e atualiza as que continuaram lá.
   * Transcrições em cache de threads paradas que avançaram são relidas.
   */
  async syncSessions(force = false) {
    const ws = this.activeWorkspaceId
    if (!ws || (!force && Date.now() - this.lastSync < 5000)) return
    this.lastSync = Date.now()
    const before = new Map(this.threads.map((t) => [t.id, t.updated_at]))
    try {
      const added = await api.syncSessions(ws)
      if (ws !== this.activeWorkspaceId) return
      await this.refreshAll()
      for (const t of this.threads) {
        const prev = before.get(t.id)
        if (prev !== undefined && prev !== t.updated_at && t.status === 'idle' && t.id !== this.selectedId)
          delete this.transcripts[t.id]
      }
      if (added && before.size) this.toast(tx(added === 1 ? 'app.importedOne' : 'app.importedOther', { n: added }), 'info')
    } catch (e) {
      this.toast(tx('app.importFailed', { error: errText(e) }), 'error')
    }
  }

  async refreshThreads() {
    if (this.activeWorkspaceId)
      this.threads = (await api.listThreads(this.activeWorkspaceId)).map((x) => ({ ...x, provider: x.provider ?? 'claude' }))
  }

  async refreshAll() {
    const ws = this.activeWorkspaceId
    if (!ws) return
    const [p, t] = await Promise.all([api.listProjects(ws), api.listThreads(ws)])
    if (ws !== this.activeWorkspaceId) return
    this.projects = p
    // threads gravadas antes dos provedores não têm o campo: são do Claude
    this.threads = t.map((x) => ({ ...x, provider: x.provider ?? 'claude' }))
  }

  repoProject = $derived(this.projects.find((p) => p.id === this.repoProjectId) ?? null)
  /** A pasta aberta (thread ou tela do repositório) é um repositório git. */
  isGit = $derived((this.selectedProject ?? this.repoProject)?.is_git ?? true)
  /** Abas visíveis, na ordem de Ctrl+1…N. */
  tabs = $derived<readonly MainTab[]>(this.isGit ? MAIN_TABS : PLAIN_TABS)

  /** Histórico com a linha de mudanças locais selecionada (staging e commit). */
  showChanges() {
    const path = this.selected?.cwd ?? this.repoProject?.path
    if (!path || !this.isGit) return
    this.tab = 'history'
    import('../../git/mount').then((m) => m.focusLocalChanges(path))
  }

  /** Abre a tela do repositório (checkout principal da pasta). */
  openRepo(projectId: string) {
    this.selectedId = null
    this.repoProjectId = projectId
    if (this.tab === 'chat' && this.isGit) this.tab = 'history'
  }

  async select(id: string | null) {
    if (id) this.repoProjectId = null
    this.selectedId = id
    if (!id) return
    this.refreshDiffStats(id)
    const t = this.threads.find((x) => x.id === id)
    if (t?.unread) {
      t.unread = false
      api.updateThreadMeta(id, { unread: false }).catch(() => {})
    }
    if (!this.transcripts[id]) {
      const st = emptyState()
      if (t?.session_id) {
        try {
          let lastUsage: { usage: any; model: string } | null = null
          for (const m of await api.loadTranscript(id)) {
            apply(st, m as any)
            const mm = (m as any).message
            if ((m as any).type === 'assistant' && mm?.usage) lastUsage = { usage: mm.usage, model: mm.model }
          }
          const c = lastUsage && contextFromUsage(lastUsage.usage, lastUsage.model)
          if (c && !this.context[id]) this.context[id] = c
        } catch (e) {
          addError(st, tx('app.historyFailed', { error: errText(e) }))
        }
      }
      this.transcripts[id] = st
    }
  }

  async newThread(projectId: string, isolated?: boolean, provider?: ProviderId, model?: string, skipPermissions?: boolean) {
    if (this.creating || !this.activeWorkspaceId) return
    this.creating = true
    try {
      // por padrão a thread roda na pasta do projeto, na branch selecionada;
      // se vale um worktree, quem decide é o próprio agente
      const t = await api.createThread(this.activeWorkspaceId, projectId, isolated ?? false, {
        provider: provider ?? this.settings?.default_provider ?? 'claude',
        model,
        skipPermissions,
      })
      this.upsert(t)
      await this.select(t.id)
      return t
    } catch (e) {
      this.toast(errText(e), 'error')
    } finally {
      this.creating = false
    }
  }

  async send(text: string, images: { mime: string; data: string }[] = []) {
    const t = this.selected
    if (!t || (!text.trim() && !images.length)) return
    addLocalUser(this.transcriptFor(t.id), text, images.map(dataUrl))
    t.status = 'running'
    try {
      await api.sendMessage(t.id, text, images.map(({ mime, data }) => ({ mime, data })))
    } catch (e) {
      t.status = 'error'
      addError(this.transcriptFor(t.id), errText(e))
    }
  }

  async interrupt() {
    if (this.selected) await api.interrupt(this.selected.id).catch((e) => this.toast(errText(e), 'error'))
  }

  async archive(id: string, archived = true) {
    try {
      this.upsert(await api.archiveThread(id, archived))
      if (archived && this.selectedId === id) this.selectedId = null
    } catch (e) {
      this.toast(errText(e), 'error')
    }
  }

  async remove(id: string) {
    try {
      await api.deleteThread(id)
      this.threads = this.threads.filter((t) => t.id !== id)
      delete this.transcripts[id]
      if (this.selectedId === id) this.selectedId = null
    } catch (e) {
      this.toast(errText(e), 'error')
    }
  }
}

export const app = new AppState()

