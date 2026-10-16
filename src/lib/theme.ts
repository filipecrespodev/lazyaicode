// Aparência: temas (paletas, como no gitHydra) e opções de layout. Tudo vira
// variáveis CSS no <html>, as mesmas que o app inteiro já usa — diff,
// terminal e botões mudam juntos. Fica no localStorage para ser aplicado
// antes da primeira pintura (sem piscar o tema padrão ao abrir).

import { t } from './i18n/index.svelte'

export type Mode = 'dark' | 'light'

export interface Palette {
  bg: string
  panel: string
  elev: string
  hover: string
  active: string
  input: string
  border: string
  borderStrong: string
  text: string
  muted: string
  faint: string
  accent: string
  blue: string
  amber: string
  red: string
}

export interface Theme {
  id: string
  label: string
  mode: Mode
  palette: Palette
}

const p = (
  bg: string, panel: string, elev: string, hover: string, active: string, input: string,
  border: string, borderStrong: string, text: string, muted: string, faint: string,
  accent: string, blue: string, amber: string, red: string,
): Palette => ({ bg, panel, elev, hover, active, input, border, borderStrong, text, muted, faint, accent, blue, amber, red })

export const THEMES: Theme[] = [
  { id: 'dark', get label() { return t('appearance.themeDark') }, mode: 'dark', palette: p('#16171b', '#1a1c20', '#1d1f24', '#25282e', '#2c3038', '#121317', '#2b2e35', '#3a3e47', '#dcdfe4', '#8b919c', '#5f6570', '#3fb47a', '#5aa7f0', '#e5b04a', '#ec6a6a') },
  { id: 'tokyo', label: 'Tokyo Night', mode: 'dark', palette: p('#1a1b26', '#16161e', '#1f2335', '#24283b', '#2f354f', '#16161e', '#2a2e42', '#3b4261', '#c0caf5', '#a9b1d6', '#565f89', '#9ece6a', '#7aa2f7', '#e0af68', '#f7768e') },
  { id: 'dracula', label: 'Dracula', mode: 'dark', palette: p('#282a36', '#21222c', '#2e303e', '#343746', '#44475a', '#21222c', '#3a3c4e', '#4d5068', '#f8f8f2', '#c4c6d4', '#6272a4', '#50fa7b', '#8be9fd', '#f1fa8c', '#ff5555') },
  { id: 'one-dark', label: 'One Dark', mode: 'dark', palette: p('#282c34', '#21252b', '#2c313a', '#2c313a', '#3a3f4b', '#1e2227', '#333842', '#3e4451', '#abb2bf', '#9da5b4', '#5c6370', '#98c379', '#61afef', '#e5c07b', '#e06c75') },
  { id: 'nord', label: 'Nord', mode: 'dark', palette: p('#2e3440', '#292e39', '#3b4252', '#3b4252', '#434c5e', '#272c36', '#3b4252', '#4c566a', '#eceff4', '#d8dee9', '#7b88a1', '#a3be8c', '#88c0d0', '#ebcb8b', '#bf616a') },
  { id: 'gruvbox', label: 'Gruvbox Dark', mode: 'dark', palette: p('#282828', '#1d2021', '#32302f', '#3c3836', '#504945', '#1d2021', '#3c3836', '#504945', '#ebdbb2', '#d5c4a1', '#928374', '#b8bb26', '#83a598', '#fabd2f', '#fb4934') },
  { id: 'solarized', label: 'Solarized Dark', mode: 'dark', palette: p('#002b36', '#00252f', '#073642', '#073642', '#0d4452', '#00212b', '#0a3a47', '#24525e', '#93a1a1', '#839496', '#586e75', '#859900', '#268bd2', '#b58900', '#dc322f') },
  { id: 'catppuccin', label: 'Catppuccin Mocha', mode: 'dark', palette: p('#1e1e2e', '#181825', '#232334', '#2a2b3c', '#313244', '#181825', '#313244', '#45475a', '#cdd6f4', '#bac2de', '#6c7086', '#a6e3a1', '#89b4fa', '#f9e2af', '#f38ba8') },
  { id: 'rose-pine', label: 'Rosé Pine', mode: 'dark', palette: p('#191724', '#1f1d2e', '#26233a', '#26233a', '#393552', '#16141f', '#26233a', '#403d52', '#e0def4', '#908caa', '#6e6a86', '#9ccfd8', '#c4a7e7', '#f6c177', '#eb6f92') },
  { id: 'light', get label() { return t('appearance.themeLight') }, mode: 'light', palette: p('#fbfbfc', '#f4f5f7', '#ffffff', '#eceef1', '#e3e6ea', '#ffffff', '#e1e4e8', '#cfd4da', '#1f2328', '#5d6570', '#8d949e', '#1f9a5b', '#0969da', '#9a6700', '#cf222e') },
  { id: 'catppuccin-latte', label: 'Catppuccin Latte', mode: 'light', palette: p('#eff1f5', '#e6e9ef', '#f6f7fa', '#dce0e8', '#ccd0da', '#ffffff', '#ccd0da', '#bcc0cc', '#4c4f69', '#5c5f77', '#8c8fa1', '#40a02b', '#1e66f5', '#df8e1d', '#d20f39') },
  { id: 'rose-pine-dawn', label: 'Rosé Pine Dawn', mode: 'light', palette: p('#faf4ed', '#f2e9e1', '#fffaf3', '#f2e9e1', '#e4dcd5', '#fffaf3', '#dfdad9', '#cecacd', '#575279', '#797593', '#9893a5', '#56949f', '#907aa9', '#ea9d34', '#b4637a') },
  { id: 'solarized-light', label: 'Solarized Light', mode: 'light', palette: p('#fdf6e3', '#eee8d5', '#fffcf0', '#eee8d5', '#e4ddc8', '#fffcf0', '#e4ddc8', '#d3cbb4', '#586e75', '#657b83', '#93a1a1', '#859900', '#268bd2', '#b58900', '#dc322f') },
]

export type Density = 'compact' | 'normal' | 'comfortable'
export type ContentWidth = 'narrow' | 'normal' | 'wide' | 'full'

export interface Appearance {
  /** id de um tema, ou "system" (segue o claro/escuro do sistema) */
  theme: string
  density: Density
  /** escala da interface, em % */
  scale: number
  contentWidth: ContentWidth
}

export const DEFAULT_APPEARANCE: Appearance = { theme: 'system', density: 'normal', scale: 100, contentWidth: 'normal' }

const ROW_HEIGHT: Record<Density, number> = { compact: 24, normal: 28, comfortable: 32 }
const CONTENT_WIDTH: Record<ContentWidth, string> = { narrow: '680px', normal: '820px', wide: '1040px', full: '100%' }
const KEY = 'ui.appearance'

export function themeById(id: string) {
  return THEMES.find((t) => t.id === id)
}

/** O tema efetivo: "system" vira o escuro ou o claro padrão do app. */
export function resolveTheme(id: string, systemDark: boolean): Theme {
  if (id === 'system' || !themeById(id)) return themeById(systemDark ? 'dark' : 'light')!
  return themeById(id)!
}

/** Luminância relativa (WCAG) de uma cor #rrggbb. */
function luminance(hex: string) {
  const [r, g, b] = [1, 3, 5].map((i) => {
    const c = parseInt(hex.slice(i, i + 2), 16) / 255
    return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4
  })
  return 0.2126 * r + 0.7152 * g + 0.0722 * b
}

export function contrast(a: string, b: string) {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x)
  return (hi + 0.05) / (lo + 0.05)
}

/** Cor do texto sobre um fundo colorido (botão primário): branco ou escuro,
 *  o que der mais contraste. Accents claros (Dracula, Catppuccin, Tokyo...)
 *  pedem texto escuro; o escuro dos temas escuros é o próprio fundo do tema. */
export function onColor(bg: string, t: Theme) {
  const dark = t.mode === 'dark' ? t.palette.bg : '#15171b'
  return contrast(bg, '#ffffff') >= contrast(bg, dark) ? '#ffffff' : dark
}

/** Variáveis CSS de uma paleta (as que o app.css define). */
export function cssVars(t: Theme): Record<string, string> {
  const c = t.palette
  return {
    '--bg': c.bg,
    '--bg-panel': c.panel,
    '--bg-elev': c.elev,
    '--bg-hover': c.hover,
    '--bg-active': c.active,
    '--bg-input': c.input,
    '--border': c.border,
    '--border-strong': c.borderStrong,
    '--text': c.text,
    '--text-muted': c.muted,
    '--text-faint': c.faint,
    '--accent': c.accent,
    '--on-accent': onColor(c.accent, t),
    '--blue': c.blue,
    '--amber': c.amber,
    '--red': c.red,
    '--add-fg': t.mode === 'dark' ? `color-mix(in srgb, ${c.accent} 75%, white)` : `color-mix(in srgb, ${c.accent} 80%, black)`,
    '--del-fg': t.mode === 'dark' ? `color-mix(in srgb, ${c.red} 75%, white)` : `color-mix(in srgb, ${c.red} 85%, black)`,
    '--add-bg': `color-mix(in srgb, ${c.accent} ${t.mode === 'dark' ? 14 : 11}%, transparent)`,
    '--del-bg': `color-mix(in srgb, ${c.red} ${t.mode === 'dark' ? 14 : 9}%, transparent)`,
    '--accent-soft': `color-mix(in srgb, ${c.accent} 14%, transparent)`,
  }
}

/** Tema escolhido antes da tela Aparência existir (só o id, noutra chave). */
const LEGACY_KEY = 'lazyaicode.theme'

export function loadAppearance(): Appearance {
  try {
    const saved = localStorage.getItem(KEY)
    if (saved) return { ...DEFAULT_APPEARANCE, ...JSON.parse(saved) }
    const legacy = localStorage.getItem(LEGACY_KEY)
    return legacy ? { ...DEFAULT_APPEARANCE, theme: legacy } : { ...DEFAULT_APPEARANCE }
  } catch {
    return { ...DEFAULT_APPEARANCE }
  }
}

export function saveAppearance(a: Appearance) {
  try {
    localStorage.setItem(KEY, JSON.stringify(a))
  } catch {}
}

const systemDark = () => typeof matchMedia === 'undefined' || matchMedia('(prefers-color-scheme: dark)').matches

/** Aplica no <html>: paleta, modo (scrollbars e controles nativos),
 *  densidade, escala e largura da conversa. */
export function applyAppearance(a: Appearance, root: HTMLElement = document.documentElement) {
  const t = resolveTheme(a.theme, systemDark())
  for (const [k, v] of Object.entries(cssVars(t))) root.style.setProperty(k, v)
  root.style.setProperty('--row-h', `${ROW_HEIGHT[a.density] ?? 28}px`)
  root.style.setProperty('--content-w', CONTENT_WIDTH[a.contentWidth] ?? '820px')
  root.style.colorScheme = t.mode
  root.dataset.theme = t.id
  root.dataset.mode = t.mode
  root.dataset.density = a.density
  // zoom no body: o layout inteiro é em px
  if (typeof document !== 'undefined' && document.body) (document.body.style as any).zoom = String((a.scale || 100) / 100)
  // quem pinta fora do CSS (o terminal) se atualiza por este evento
  if (typeof window !== 'undefined') window.dispatchEvent(new CustomEvent('appearance-changed', { detail: t }))
  return t
}

/** Mantém "Sistema" em sincronia quando o sistema troca claro/escuro. */
export function watchSystemTheme(get: () => Appearance) {
  if (typeof matchMedia === 'undefined') return () => {}
  const mq = matchMedia('(prefers-color-scheme: dark)')
  const on = () => get().theme === 'system' && applyAppearance(get())
  mq.addEventListener('change', on)
  return () => mq.removeEventListener('change', on)
}
