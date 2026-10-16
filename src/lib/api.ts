import { invoke } from '@tauri-apps/api/core'
import { t } from './i18n/index.svelte'

export type Status = 'idle' | 'running' | 'awaiting' | 'error'

export interface Project {
  id: string
  name: string
  path: string
  created_at: number
  /** tem `.git`; sem ele só há Conversa e Terminal */
  is_git: boolean
}

export interface Thread {
  id: string
  project_id: string
  title: string
  created_at: number
  updated_at: number
  cwd: string
  isolated: boolean
  branch: string | null
  base_ref: string | null
  session_id: string | null
  model: string
  permission_mode: string
  archived: boolean
  bookmarked: boolean
  unread: boolean
  workspace_id: string | null
  claude_dir: string | null
  effort: string | null
  agent: string | null
  add_dirs: string[]
  provider: ProviderId
  skip_permissions: boolean
  status: Status
}

export type ProviderId = 'claude' | 'codex' | 'gemini'

export const PROVIDERS: { id: ProviderId; label: string; vendor: string; short: string }[] = [
  { id: 'claude', label: 'Claude Code', vendor: 'Anthropic', short: 'C' },
  { id: 'codex', label: 'Codex', vendor: 'OpenAI', short: 'O' },
  { id: 'gemini', label: 'Gemini CLI', vendor: 'Google', short: 'G' },
]

export interface ProviderStatus {
  provider: ProviderId
  ok: boolean
  available: boolean
  error?: string
  launch?: { bin: string; args: string[]; how: 'installed' | 'npx' | 'env' }
  agent?: { name?: string; title?: string; version?: string } | null
  acp?: AcpSession | null
}

export interface AcpOption {
  id: string
  name: string
  description?: string | null
}

export interface AcpSession {
  modes: AcpOption[]
  currentMode: string | null
  models: AcpOption[]
  currentModel: string | null
}

export interface DiffStats {
  additions: number
  deletions: number
  files: number
  branch: string | null
}

export interface Profile {
  name: string | null
  email: string | null
  avatar: string | null
}

export const EFFORTS = [
  { id: '', get label() { return t('api.effortDefault') } },
  { id: 'low', label: 'Low' },
  { id: 'medium', label: 'Medium' },
  { id: 'high', label: 'High' },
  { id: 'xhigh', label: 'XHigh' },
  { id: 'max', label: 'Max' },
]

export interface Workspace {
  id: string
  name: string
  path: string
  claude_dir: string | null
  ignored_paths: string[]
  created_at: number
}

export interface RepoEntry {
  name: string
  path: string
  is_repo: boolean
}

export interface AuthInfo {
  loggedIn?: boolean
  email?: string
  orgName?: string
  subscriptionType?: string
  authMethod?: string
  error?: string
  [k: string]: unknown
}

export interface CapItem {
  kind: 'skill' | 'agent' | 'command'
  name: string
  description: string
  scope: 'global' | 'project' | 'plugin' | 'builtin' | 'other'
  plugin: string | null
  path: string
  model: string | null
  tools: string | null
  error: string | null
  loaded: boolean
  builtin: boolean
  argument_hint: string | null
}

export interface McpServer {
  name: string
  status: 'connected' | 'failed' | 'needs-auth' | 'pending' | 'disabled' | string
  scope: string | null
  error: string | null
  serverInfo: { name?: string; version?: string } | null
  transport: string | null
  url: string | null
  command: string | null
  tools: { name: string; description?: string }[]
}

export interface Capabilities {
  commands: CapItem[]
  agents: CapItem[]
  mcp: McpServer[]
  models: { value: string; displayName: string; description?: string }[] | null
  account: { email?: string; organization?: string; subscriptionType?: string } | null
  version: string | null
  claudeDir: string
  cwd: string
}

export interface Settings {
  default_model: string
  default_permission_mode: string
  send_with_modifier: boolean
  notify_on_finish: boolean
  sound_on_finish: boolean
  default_provider: ProviderId
  default_skip_permissions: boolean
  language: string
}

export interface ClaudeDir {
  path: string
  label: string
  is_default: boolean
  has_credentials: boolean
}

export interface FileChange {
  path: string
  old_path: string | null
  status: string
  additions: number
  deletions: number
  patch: string
  binary: boolean
  truncated: boolean
  fingerprint: string
  seen: boolean
}

export interface Changes {
  base: string
  baseLabel: string
  files: FileChange[]
}

export interface BranchInfo {
  branch: string
  ahead: number
  behind: number
  dirty: boolean
  base_ref: string
}

export const MODELS = [
  { id: 'default', get label() { return t('api.defaultSettings') } },
  { id: 'opus', label: 'Opus' },
  { id: 'opus[1m]', label: 'Opus 1M' },
  { id: 'sonnet', label: 'Sonnet' },
  { id: 'sonnet[1m]', label: 'Sonnet 1M' },
  { id: 'haiku', label: 'Haiku' },
  { id: 'fable', label: 'Fable' },
]

export const PERMISSION_MODES = [
  { id: 'default', get label() { return t('api.defaultSettings') } },
  { id: 'manual', get label() { return t('api.modeManual') } },
  { id: 'acceptEdits', get label() { return t('api.modeAcceptEdits') } },
  { id: 'auto', get label() { return t('api.modeAuto') } },
  { id: 'plan', get label() { return t('api.modePlan') } },
  { id: 'bypassPermissions', get label() { return t('api.modeBypass') } },
]

export const api = {
  getSettings: () =>
    invoke<{
      settings: Settings
      claudeBinary: string | null
      dataDir: string
      defaultClaudeDir: string
      suggestedClaudeDir: string
    }>('get_settings'),
  saveSettings: (settings: Settings) => invoke<void>('save_settings', { settings }),
  listClaudeDirs: () => invoke<ClaudeDir[]>('list_claude_dirs'),
  authStatus: (claudeDir: string | null) => invoke<AuthInfo>('auth_status', { claudeDir }),
  /** famílias de fonte instaladas (fc-list / WPF) e o que o padrão do sistema resolve */
  listSystemFonts: () => invoke<{ families: string[]; defaultText: string; defaultCode: string }>('list_system_fonts'),

  listWorkspaces: () => invoke<{ workspaces: Workspace[]; active: string | null }>('list_workspaces'),
  createWorkspace: (path: string, name: string | null, claudeDir: string | null) =>
    invoke<Workspace>('create_workspace', { path, name, claudeDir }),
  updateWorkspace: (
    id: string,
    patch: { name?: string; claudeDir?: string | null; ignoredPaths?: string[] },
  ) => invoke<Workspace>('update_workspace', { id, ...patch }),
  removeWorkspace: (id: string) => invoke<void>('remove_workspace', { id }),
  setActiveWorkspace: (id: string) => invoke<void>('set_active_workspace', { id }),
  workspaceRepos: (id: string) => invoke<RepoEntry[]>('workspace_repos', { id }),

  getCapabilities: (workspaceId: string, path: string | null, force = false) =>
    invoke<Capabilities>('get_capabilities', { workspaceId, path, force }),
  threadMcp: (id: string, action: 'status' | 'toggle' | 'reconnect', server?: string, enabled?: boolean) =>
    invoke<McpServer[]>('thread_mcp', { id, action, server, enabled }),
  trashItem: (workspaceId: string, path: string) => invoke<void>('trash_item', { workspaceId, path }),

  listProjects: (workspaceId: string) => invoke<Project[]>('list_projects', { workspaceId }),
  listThreads: (workspaceId: string) => invoke<Thread[]>('list_threads', { workspaceId }),
  syncSessions: (workspaceId: string) => invoke<number>('sync_sessions', { workspaceId }),
  createThread: (
    workspaceId: string,
    projectId: string,
    isolated: boolean,
    opts: { model?: string; permissionMode?: string; provider?: ProviderId; skipPermissions?: boolean } = {},
  ) => invoke<Thread>('create_thread', { workspaceId, projectId, isolated, ...opts }),
  setThreadProvider: (id: string, provider: ProviderId, model?: string) =>
    invoke<Thread>('set_thread_provider', { id, provider, model }),
  providerStatus: (provider: ProviderId, force = false) => invoke<ProviderStatus>('provider_status', { provider, force }),
  providerAuthMethods: (provider: ProviderId) =>
    invoke<{ methods: { id: string; name: string; description?: string; type?: string; vars?: { name: string }[] }[] }>(
      'provider_auth_methods',
      { provider },
    ),
  providerLogin: (provider: ProviderId, method: string) => invoke<void>('provider_login', { provider, method }),
  updateThreadMeta: (id: string, meta: { title?: string; bookmarked?: boolean; unread?: boolean }) =>
    invoke<Thread>('update_thread_meta', { id, ...meta }),
  archiveThread: (id: string, archived: boolean) => invoke<Thread>('archive_thread', { id, archived }),
  deleteThread: (id: string) => invoke<void>('delete_thread', { id }),
  loadTranscript: (id: string) => invoke<unknown[]>('load_transcript', { id }),

  sendMessage: (id: string, text: string, images: { mime: string; data: string }[] = []) =>
    invoke<void>('send_message', { id, text, images }),
  readImageFile: (path: string) => invoke<ArrayBuffer>('read_image_file', { path }),
  interrupt: (id: string) => invoke<void>('interrupt', { id }),
  respondPermission: (
    id: string,
    requestId: string,
    allow: boolean,
    input: unknown,
    toolUseId?: string,
    message?: string,
    updatedPermissions?: unknown[],
  ) => invoke<void>('respond_permission', { id, requestId, allow, input, toolUseId, message, updatedPermissions }),
  setThreadModel: (id: string, model: string) => invoke<Thread>('set_thread_model', { id, model }),
  setThreadPermissionMode: (id: string, mode: string) => invoke<Thread>('set_thread_permission_mode', { id, mode }),

  setThreadOptions: (
    id: string,
    opts: { effort?: string | null; agent?: string | null; addDirs?: string[]; skipPermissions?: boolean },
  ) =>
    invoke<Thread>('set_thread_options', { id, ...opts }),
  diffStats: (id: string) => invoke<DiffStats>('diff_stats', { id }),
  userProfile: () => invoke<Profile>('user_profile'),
  getChanges: (id: string, base: string) => invoke<Changes>('get_changes', { id, base }),
  setSeen: (id: string, entries: [string, string | null][]) => invoke<void>('set_seen', { id, entries }),
  branchInfo: (id: string) => invoke<BranchInfo>('branch_info', { id }),
  landChanges: (id: string, mode: 'merge' | 'apply') => invoke<string>('land_changes', { id, mode }),
  openPath: (path: string) => invoke<void>('open_path', { path }),

  termSpawn: (termId: string, cwd: string, cols: number, rows: number) =>
    invoke<void>('term_spawn', { termId, cwd, cols, rows }),
  termWrite: (termId: string, data: string) => invoke<void>('term_write', { termId, data }),
  termResize: (termId: string, cols: number, rows: number) => invoke<void>('term_resize', { termId, cols, rows }),
  termKill: (termId: string) => invoke<void>('term_kill', { termId }),

  // WSL (app do Windows): pastas lidas pelo agente dentro da distro
  listWslDistros: () => invoke<string[]>('list_wsl_distros'),
  wslHome: (distro: string) => invoke<string>('wsl_home', { distro }),
  listWslDirs: (path: string) => invoke<WslDirEntry[]>('list_wsl_dirs', { path }),
}

export interface WslDirEntry {
  name: string
  /** caminho UNC (`\\wsl.localhost\<distro>\...`) */
  path: string
  isRepo: boolean
}

export function errText(e: unknown): string {
  return typeof e === 'string' ? e : e instanceof Error ? e.message : JSON.stringify(e)
}
