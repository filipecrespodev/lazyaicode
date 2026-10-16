import type { Messages } from '../index.svelte'

export default {
  en: { now: 'now', minutes: '{n} min', hours: '{n} h', days: '{n} d' },
  pt: { now: 'agora', minutes: '{n} min', hours: '{n} h', days: '{n} d' },
  es: { now: 'ahora', minutes: '{n} min', hours: '{n} h', days: '{n} d' },
  zh: { now: '刚刚', minutes: '{n} 分钟', hours: '{n} 小时', days: '{n} 天' },
} satisfies Messages
