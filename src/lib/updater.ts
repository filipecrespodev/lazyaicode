import { check } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'

// Um servidor de atualização fora do ar não pode atrasar a abertura.
const CHECK_TIMEOUT_MS = 5000

/**
 * Instala uma versão nova publicada no GitHub Releases e reinicia nela.
 * Resolve em silêncio quando não há atualização ou algo falha (offline,
 * build de desenvolvimento, assinatura inválida): o app segue na versão atual.
 */
export async function applyUpdate(onProgress: (version: string, pct: number | null) => void) {
  try {
    const update = await Promise.race([
      check(),
      new Promise<null>((r) => setTimeout(() => r(null), CHECK_TIMEOUT_MS)),
    ])
    if (!update) return
    let total = 0
    let got = 0
    onProgress(update.version, null)
    await update.downloadAndInstall((e) => {
      if (e.event === 'Started') total = e.data.contentLength ?? 0
      else if (e.event === 'Progress') {
        got += e.data.chunkLength
        onProgress(update.version, total ? got / total : null)
      }
    })
    await relaunch()
  } catch {
    // segue na versão atual
  }
}
