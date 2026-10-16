import type { Messages } from '../index.svelte'

export default {
  en: { error: 'error', result: 'Result', empty: '(empty)' },
  pt: { error: 'erro', result: 'Resultado', empty: '(vazio)' },
  es: { error: 'error', result: 'Resultado', empty: '(vacío)' },
  zh: { error: '错误', result: '结果', empty: '（空）' },
} satisfies Messages
