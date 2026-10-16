import { expect, it } from 'vitest'
import { parsePatch } from './diff'

it('numbers both sides of a hunk and skips the file header', () => {
  const lines = parsePatch(
    ['diff --git a/x b/x', '--- a/x', '+++ b/x', '@@ -3,2 +3,3 @@ fn', ' um', '-dois', '+2', '+tres', '\\ No newline at end of file'].join('\n'),
  )
  expect(lines.map((l) => l.t)).toEqual(['hunk', 'ctx', 'del', 'add', 'add'])
  expect(lines[1]).toMatchObject({ a: 3, b: 3 })
  expect(lines[2]).toMatchObject({ a: 4, b: null })
  expect(lines[4]).toMatchObject({ a: null, b: 5, text: 'tres' })
})
