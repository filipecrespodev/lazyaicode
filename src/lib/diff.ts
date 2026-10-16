export type DiffLine =
  | { t: 'hunk'; text: string }
  | { t: 'ctx' | 'add' | 'del'; text: string; a: number | null; b: number | null }

/** Linhas renderizáveis de um patch de um arquivo, com numeração dos dois lados. */
export function parsePatch(patch: string): DiffLine[] {
  const out: DiffLine[] = []
  let a = 0
  let b = 0
  let inHunk = false
  for (const line of patch.split('\n')) {
    const h = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@(.*)$/.exec(line)
    if (h) {
      a = Number(h[1])
      b = Number(h[2])
      inHunk = true
      out.push({ t: 'hunk', text: line })
      continue
    }
    if (!inHunk) continue
    if (line.startsWith('+')) out.push({ t: 'add', text: line.slice(1), a: null, b: b++ })
    else if (line.startsWith('-')) out.push({ t: 'del', text: line.slice(1), a: a++, b: null })
    else if (line.startsWith('\\')) continue
    else out.push({ t: 'ctx', text: line.slice(1), a: a++, b: b++ })
  }
  return out
}
