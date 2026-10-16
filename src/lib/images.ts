import { api } from './api'
import { t } from './i18n/index.svelte'

// Imagens anexadas no chat (colar com Ctrl+V ou arrastar). Vão ao agente em
// base64, como blocos de imagem; os limites são os do Claude.

export type Attachment = { id: number; name: string; mime: string; data: string }

export const IMAGE_MIMES = ['image/png', 'image/jpeg', 'image/gif', 'image/webp']
/** O Claude recusa imagens acima de 5 MB. */
export const MAX_IMAGE_BYTES = 5 * 1024 * 1024
export const MAX_IMAGES = 10

export const dataUrl = (a: { mime: string; data: string }) => `data:${a.mime};base64,${a.data}`

let seq = 0

const EXT_MIME: Record<string, string> = {
  png: 'image/png', jpg: 'image/jpeg', jpeg: 'image/jpeg', gif: 'image/gif', webp: 'image/webp',
  bmp: 'image/bmp', svg: 'image/svg+xml', tif: 'image/tiff', tiff: 'image/tiff', avif: 'image/avif', heic: 'image/heic',
}
const extMime = (name: string) => EXT_MIME[name.split('.').pop()?.toLowerCase() ?? ''] ?? ''
/** O WebKitGTK (Linux) entrega arquivos arrastados sem `type`: vale a extensão. */
const mimeOf = (f: File) => f.type || extMime(f.name)

function toBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const r = new FileReader()
    r.onload = () => resolve(String(r.result).replace(/^data:[^,]*,/, ''))
    r.onerror = () => reject(r.error)
    r.readAsDataURL(file)
  })
}

/** Lê as imagens de `files`, até completar `room` anexos. Arquivos que não
 *  são imagem são ignorados em silêncio (vale para arrastar vários de uma
 *  vez); imagem de formato ou tamanho não aceito vira um aviso. */
export async function readImages(files: Iterable<File>, room: number) {
  const added: Attachment[] = []
  const errors: string[] = []
  for (const f of files) {
    const mime = mimeOf(f)
    if (!mime.startsWith('image/')) continue
    if (added.length >= room) {
      errors.push(t('images.tooMany', { max: MAX_IMAGES }))
      break
    }
    const name = f.name || t('images.pasted')
    if (!IMAGE_MIMES.includes(mime)) {
      errors.push(t('images.unsupported', { name }))
      continue
    }
    if (f.size > MAX_IMAGE_BYTES) {
      errors.push(t('images.tooLarge', { name }))
      continue
    }
    try {
      added.push({ id: ++seq, name, mime, data: await toBase64(f) })
    } catch {
      errors.push(t('images.unreadable', { name }))
    }
  }
  return { added, errors }
}

/** Arquivos de imagem de um colar (Ctrl+V) ou de um arrastar. */
export function imageFiles(dt: DataTransfer | null): File[] {
  if (!dt) return []
  const out: File[] = []
  for (const item of dt.items ?? []) {
    if (item.kind !== 'file') continue
    const f = item.getAsFile()
    if (f && mimeOf(f).startsWith('image/')) out.push(f)
  }
  if (!out.length) for (const f of dt.files ?? []) if (mimeOf(f).startsWith('image/')) out.push(f)
  return out
}

/** Caminhos de imagem que vieram só como `file://` (arquivo copiado no
 *  gerenciador de arquivos, ou arrastado no Linux sem `File` no evento). */
export function imagePaths(dt: DataTransfer | null): string[] {
  if (!dt) return []
  const lines = [dt.getData('text/uri-list'), dt.getData('x-special/gnome-copied-files')].join('\n').split(/\r?\n/)
  const out = new Set<string>()
  for (const l of lines) {
    if (!l.startsWith('file://')) continue
    try {
      const path = decodeURIComponent(new URL(l).pathname)
      if (extMime(path).startsWith('image/')) out.add(path)
    } catch {
      /* URI malformada: ignora */
    }
  }
  return [...out]
}

/** Lê do disco as imagens de `paths` (pelo backend) como `File`. */
export async function filesFromPaths(paths: string[]) {
  const files: File[] = []
  const errors: string[] = []
  for (const path of paths) {
    const name = path.split('/').pop() || path
    try {
      files.push(new File([await api.readImageFile(path)], name, { type: extMime(path) }))
    } catch (err) {
      errors.push(t(String(err).includes('too_large') ? 'images.tooLarge' : 'images.unreadable', { name }))
    }
  }
  return { files, errors }
}

/** O colar pode trazer uma imagem que o WebView não expõe como arquivo (no
 *  WebKitGTK, um print)? Só quando não há texto nenhum: colar texto nunca é
 *  interceptado, venha com o tipo que vier. */
export function mayHaveNativeImage(dt: DataTransfer | null) {
  if (!dt) return true
  return !Array.from(dt.types ?? []).some((t) => t.startsWith('text/') && dt.getData(t).trim())
}

function canvasBlob(c: HTMLCanvasElement, type: string, quality?: number) {
  return new Promise<Blob | null>((resolve) => c.toBlob(resolve, type, quality))
}

/** Imagem do clipboard do sistema, em PNG (JPEG se o PNG passar de 5 MB). */
export async function clipboardImage(): Promise<File | null> {
  try {
    const { readImage } = await import('@tauri-apps/plugin-clipboard-manager')
    const img = await readImage()
    const [{ width, height }, rgba] = await Promise.all([img.size(), img.rgba()])
    await img.close().catch(() => {})
    if (!width || !height) return null
    const c = document.createElement('canvas')
    c.width = width
    c.height = height
    c.getContext('2d')!.putImageData(new ImageData(new Uint8ClampedArray(rgba), width, height), 0, 0)
    let blob = await canvasBlob(c, 'image/png')
    if (blob && blob.size > MAX_IMAGE_BYTES) blob = await canvasBlob(c, 'image/jpeg', 0.85)
    return blob ? new File([blob], '', { type: blob.type }) : null
  } catch {
    return null // clipboard sem imagem
  }
}

/** O arrasto traz arquivos? (durante o arrasto o tipo do arquivo pode não
 *  estar disponível; no Linux às vezes só vem a lista de URIs) */
export const dragHasFiles = (dt: DataTransfer | null) => {
  const types = Array.from(dt?.types ?? [])
  return types.includes('Files') || types.includes('text/uri-list')
}
