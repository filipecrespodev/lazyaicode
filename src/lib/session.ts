import { isPermissionGranted, requestPermission, sendNotification } from '@tauri-apps/plugin-notification'

export interface ContextUse {
  used: number
  window: number
  model: string
}

const WINDOWS_KEY = 'contextWindows'

function windows(): Record<string, number> {
  try {
    return JSON.parse(localStorage.getItem(WINDOWS_KEY) ?? '{}')
  } catch {
    return {}
  }
}

/** Guarda a janela de contexto de cada modelo (vem no `result`), para
 *  mostrar o uso também em threads abertas só pelo histórico. */
export function rememberWindow(model: string, window: number) {
  try {
    localStorage.setItem(WINDOWS_KEY, JSON.stringify({ ...windows(), [model]: window }))
  } catch {}
}

/** Tokens na janela = entrada + cache lido + cache criado da última chamada. */
export function usedTokens(u: any): number {
  if (!u) return 0
  return (u.input_tokens ?? 0) + (u.cache_read_input_tokens ?? 0) + (u.cache_creation_input_tokens ?? 0)
}

export function contextFromResult(r: any): ContextUse | null {
  const models = Object.entries(r?.modelUsage ?? {}) as [string, any][]
  if (!models.length) return null
  // o modelo principal é o de maior janela usada; subagentes também aparecem aqui
  const [model, mu] = models.sort((a, b) => (b[1].contextWindow ?? 0) - (a[1].contextWindow ?? 0))[0]
  const iters = r?.usage?.iterations
  const last = Array.isArray(iters) && iters.length ? iters[iters.length - 1] : r?.usage
  const used = usedTokens(last)
  if (!mu?.contextWindow || !used) return null
  return { used, window: mu.contextWindow, model }
}

export function contextFromUsage(usage: any, model: string): ContextUse | null {
  const window = windows()[model]
  const used = usedTokens(usage)
  return window && used ? { used, window, model } : null
}

const MODELS_KEY = 'providerModels'

/** Modelos que cada agente ACP anunciou na última sessão — para o seletor
 *  oferecer modelos do Codex/Gemini antes de abrir uma sessão com eles. */
export function rememberProviderModels(provider: string | undefined, models: { id: string; name: string }[] | undefined) {
  if (!provider || !models?.length) return
  try {
    const all = JSON.parse(localStorage.getItem(MODELS_KEY) ?? '{}')
    all[provider] = models.map((m) => ({ id: m.id, name: m.name }))
    localStorage.setItem(MODELS_KEY, JSON.stringify(all))
  } catch {}
}

export function knownProviderModels(provider: string): { id: string; name: string }[] {
  try {
    return JSON.parse(localStorage.getItem(MODELS_KEY) ?? '{}')[provider] ?? []
  } catch {
    return []
  }
}

export function fmtTokens(n: number) {
  return n >= 1_000_000 ? `${(n / 1_000_000).toFixed(1)}M` : n >= 1000 ? `${Math.round(n / 1000)}k` : String(n)
}

let audio: AudioContext | null = null

/** Dois toques curtos, sem arquivo de som. */
export function chime() {
  try {
    audio ??= new AudioContext()
    const t0 = audio.currentTime
    for (const [i, f] of [660, 880].entries()) {
      const o = audio.createOscillator()
      const g = audio.createGain()
      o.frequency.value = f
      g.gain.setValueAtTime(0.0001, t0 + i * 0.12)
      g.gain.exponentialRampToValueAtTime(0.15, t0 + i * 0.12 + 0.02)
      g.gain.exponentialRampToValueAtTime(0.0001, t0 + i * 0.12 + 0.25)
      o.connect(g).connect(audio.destination)
      o.start(t0 + i * 0.12)
      o.stop(t0 + i * 0.12 + 0.3)
    }
  } catch {}
}

let granted: boolean | null = null

export async function notifyFinished(title: string, body: string, notify: boolean, sound: boolean) {
  if (sound) chime()
  if (!notify) return
  try {
    if (granted === null) granted = (await isPermissionGranted()) || (await requestPermission()) === 'granted'
    if (granted) sendNotification({ title, body })
  } catch {}
}
