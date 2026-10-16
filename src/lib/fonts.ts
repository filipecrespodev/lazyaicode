// Fontes (como no gitHydra): família e tamanho do texto da interface e do
// código. Viram variáveis CSS no <html> — --sans/--mono e --text-size/--code-size
// do app.css (shell Svelte), --font-sans/--font-mono do Tailwind (visão Git) —
// e o terminal se atualiza pelo evento "fonts-changed". Fica no localStorage
// para ser aplicado antes da primeira pintura, como a aparência.

export interface FontSettings {
  /** família do texto da interface; vazio = padrão do app */
  textFamily: string
  /** tamanho base do texto, em px */
  textSize: number
  /** família do código (diffs, blocos de código, terminal); vazio = padrão */
  codeFamily: string
  /** tamanho do código, em px */
  codeSize: number
  /** família do terminal; vazio = Nerd Font instalada ou monoespaçada comum */
  terminalFamily: string
  /** tamanho do terminal, em px */
  terminalSize: number
}

export const DEFAULT_FONTS: FontSettings = { textFamily: '', textSize: 13, codeFamily: '', codeSize: 12, terminalFamily: '', terminalSize: 13 }
export const TEXT_SIZE = { min: 10, max: 20 }
export const CODE_SIZE = { min: 8, max: 24 }
export const TERMINAL_SIZE = { min: 8, max: 28 }

const KEY = 'ui.fonts'
const SANS_FALLBACK = "'Inter', -apple-system, 'Segoe UI', system-ui, sans-serif"
const MONO_FALLBACK = "'JetBrains Mono', 'Fira Code', 'SF Mono', ui-monospace, Menlo, monospace"
/*
 * Terminal (como no gitHydra): a fonte escolhida e, atrás dela, a que estiver
 * instalada destas — Nerd Fonts primeiro, porque prompts como Powerlevel10k,
 * oh-my-posh e starship precisam dos ícones delas; depois monoespaçadas comuns.
 * "LazyAICode Symbols" (app.css) é a Symbols Nerd Font embutida, só na faixa
 * de ícones: os prompts mostram os ícones mesmo sem Nerd Font (comum no WSL),
 * e as letras continuam vindo da fonte do usuário.
 */
const TERMINAL_FALLBACK = [
  'LazyAICode Symbols',
  'Symbols Nerd Font Mono',
  'Symbols Nerd Font',
  'MesloLGS NF',
  'MesloLGS Nerd Font',
  'FiraCode Nerd Font',
  'JetBrainsMono Nerd Font',
  'Hack Nerd Font',
  'CaskaydiaCove Nerd Font',
  'JetBrains Mono',
  'Fira Code',
  'Cascadia Code',
  'Consolas',
  'Liberation Mono',
  'DejaVu Sans Mono',
  'Menlo',
  'monospace',
]

const clamp = (n: unknown, { min, max }: { min: number; max: number }, fallback: number) =>
  typeof n === 'number' && Number.isFinite(n) ? Math.min(max, Math.max(min, n)) : fallback

/** Corrige o que veio salvo: campos faltando ou fora da faixa voltam ao padrão. */
export function normalizeFonts(saved: Partial<FontSettings> | null | undefined): FontSettings {
  const s = saved ?? {}
  return {
    textFamily: typeof s.textFamily === 'string' ? s.textFamily.trim() : '',
    textSize: clamp(s.textSize, TEXT_SIZE, DEFAULT_FONTS.textSize),
    codeFamily: typeof s.codeFamily === 'string' ? s.codeFamily.trim() : '',
    codeSize: clamp(s.codeSize, CODE_SIZE, DEFAULT_FONTS.codeSize),
    terminalFamily: typeof s.terminalFamily === 'string' ? s.terminalFamily.trim() : '',
    terminalSize: clamp(s.terminalSize, TERMINAL_SIZE, DEFAULT_FONTS.terminalSize),
  }
}

/** Um nome vira família entre aspas; uma lista com vírgulas é usada como pilha. */
export function fontStack(family: string, fallback: string): string | null {
  const f = family.trim()
  if (!f) return null
  return `${f.includes(',') ? f : `"${f.replace(/"/g, '')}"`}, ${fallback}`
}

/** Variáveis CSS das fontes; `null` = remover (vale o padrão do CSS). */
export function fontVars(f: FontSettings): Record<string, string | null> {
  const sans = fontStack(f.textFamily, SANS_FALLBACK)
  const mono = fontStack(f.codeFamily, MONO_FALLBACK)
  const custom = (n: number, d: number) => (n === d ? null : `${n}px`)
  return {
    '--sans': sans,
    '--mono': mono,
    '--font-sans': sans,
    '--font-mono': mono,
    '--text-size': custom(f.textSize, DEFAULT_FONTS.textSize),
    '--code-size': custom(f.codeSize, DEFAULT_FONTS.codeSize),
  }
}

/** A visão Git é em rem (Tailwind, 16px na raiz): escala junto com o texto. */
export function rootFontSize(f: FontSettings): string {
  return f.textSize === DEFAULT_FONTS.textSize ? '' : `${(f.textSize * 16) / DEFAULT_FONTS.textSize}px`
}

/** Pilha de fontes do xterm: a escolhida (ou lista com vírgulas) e as reservas. */
export function terminalFontFamily(f: FontSettings): string {
  const custom = f.terminalFamily.trim()
  const chosen = custom ? (custom.includes(',') ? [custom] : [`"${custom.replace(/"/g, '')}"`]) : []
  return [...chosen, ...TERMINAL_FALLBACK.map((n) => (n.includes(' ') ? `"${n}"` : n))].join(', ')
}

export function loadFonts(): FontSettings {
  try {
    const saved = localStorage.getItem(KEY)
    return normalizeFonts(saved ? JSON.parse(saved) : null)
  } catch {
    return { ...DEFAULT_FONTS }
  }
}

export function saveFonts(f: FontSettings) {
  try {
    localStorage.setItem(KEY, JSON.stringify(f))
  } catch {}
}

/** Aplica no <html> e avisa quem pinta fora do CSS (o terminal). */
export function applyFonts(f: FontSettings, root: HTMLElement = document.documentElement) {
  for (const [k, v] of Object.entries(fontVars(f))) {
    if (v) root.style.setProperty(k, v)
    else root.style.removeProperty(k)
  }
  root.style.fontSize = rootFontSize(f)
  if (f.codeSize === DEFAULT_FONTS.codeSize) delete root.dataset.codeSize
  else root.dataset.codeSize = 'custom'
  if (typeof window !== 'undefined') window.dispatchEvent(new CustomEvent('fonts-changed', { detail: f }))
}
