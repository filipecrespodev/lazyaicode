import type { Messages } from '../index.svelte'

export default {
  en: { openError: 'Error opening terminal: {error}', exited: 'Process exited' },
  pt: { openError: 'Erro ao abrir terminal: {error}', exited: 'Processo encerrado' },
  es: { openError: 'Error al abrir la terminal: {error}', exited: 'Proceso finalizado' },
  zh: { openError: '打开终端出错：{error}', exited: '进程已结束' },
} satisfies Messages
