// Repositórios dentro do WSL (app do Windows): o app os conhece pelo caminho
// UNC (`\\wsl.localhost\<distro>\...`), e o backend os atende por um agente
// que roda dentro da distro.

/** O WSL só existe no Windows. */
export const IS_WINDOWS = /Windows/.test(globalThis.navigator?.userAgent ?? '')

/** Distro de um caminho `\\wsl.localhost\<distro>\...` (ou `\\wsl$\`); null para o resto. */
export function wslDistro(path: string | undefined | null): string | null {
  const m = /^[\\/]{2}(?:\?[\\/]UNC[\\/])?wsl(?:\.localhost|\$)[\\/]+([^\\/]+)/i.exec(path ?? '')
  return m ? m[1] : null
}

/** Último segmento de um caminho com `/` ou `\`. */
export function baseName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? ''
}

/** Pasta-mãe de um caminho UNC do WSL; null na raiz da distro (`\\wsl.localhost\Ubuntu`). */
export function wslParent(path: string): string | null {
  const m = /^([\\/]{2}[^\\/]+[\\/]+[^\\/]+)((?:[\\/]+[^\\/]+)*?)[\\/]+[^\\/]+[\\/]*$/.exec(path)
  if (!m) return null
  return m[1] + m[2]
}
