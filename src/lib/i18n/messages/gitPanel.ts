import type { Messages } from '../index.svelte'

export default {
  en: { loadPanelError: 'Could not load the Git panel: {error}', loadError: 'Could not load: {error}' },
  pt: { loadPanelError: 'Não foi possível carregar o painel Git: {error}', loadError: 'Não foi possível carregar: {error}' },
  es: { loadPanelError: 'No se pudo cargar el panel de Git: {error}', loadError: 'No se pudo cargar: {error}' },
  zh: { loadPanelError: '无法加载 Git 面板：{error}', loadError: '无法加载：{error}' },
} satisfies Messages
