import type { Messages } from '../index.svelte'

export default {
  en: {
    tooMany: 'At most {max} images per message.',
    pasted: 'pasted image',
    unsupported: '{name}: unsupported format (use PNG, JPEG, GIF or WebP).',
    tooLarge: '{name}: larger than 5 MB.',
    unreadable: '{name}: could not read the file.',
  },
  pt: {
    tooMany: 'No máximo {max} imagens por mensagem.',
    pasted: 'imagem colada',
    unsupported: '{name}: formato não suportado (use PNG, JPEG, GIF ou WebP).',
    tooLarge: '{name}: maior que 5 MB.',
    unreadable: '{name}: não foi possível ler o arquivo.',
  },
  es: {
    tooMany: 'Máximo {max} imágenes por mensaje.',
    pasted: 'imagen pegada',
    unsupported: '{name}: formato no compatible (usa PNG, JPEG, GIF o WebP).',
    tooLarge: '{name}: mayor de 5 MB.',
    unreadable: '{name}: no se pudo leer el archivo.',
  },
  zh: {
    tooMany: '每条消息最多 {max} 张图片。',
    pasted: '粘贴的图片',
    unsupported: '{name}：不支持的格式（请使用 PNG、JPEG、GIF 或 WebP）。',
    tooLarge: '{name}：超过 5 MB。',
    unreadable: '{name}：无法读取文件。',
  },
} satisfies Messages
