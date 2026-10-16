import { mount } from 'svelte'
import '@xterm/xterm/css/xterm.css'
import './app.css'
import App from './App.svelte'
import { applyAppearance, loadAppearance, watchSystemTheme } from './lib/theme'
import { applyFonts, loadFonts } from './lib/fonts'

// antes de montar: o app abre já no tema escolhido, sem piscar o padrão
applyAppearance(loadAppearance())
watchSystemTheme(loadAppearance)
applyFonts(loadFonts())

export default mount(App, { target: document.getElementById('app')! })
