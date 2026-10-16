import type { Messages } from '../index.svelte'

export default {
  en: { markSeen: 'Mark as seen', seen: 'Seen', binary: 'Binary file', truncated: 'Diff truncated (large file)' },
  pt: { markSeen: 'Marcar como visto', seen: 'Visto', binary: 'Arquivo binário', truncated: 'Diff truncado (arquivo grande)' },
  es: { markSeen: 'Marcar como visto', seen: 'Visto', binary: 'Archivo binario', truncated: 'Diff truncado (archivo grande)' },
  zh: { markSeen: '标记为已查看', seen: '已查看', binary: '二进制文件', truncated: 'Diff 已截断（文件过大）' },
} satisfies Messages
