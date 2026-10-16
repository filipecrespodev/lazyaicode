import { describe, expect, it } from 'vitest'
import { contextFromResult, fmtTokens, usedTokens } from './session'

describe('context usage', () => {
  it('counts input plus cache of the last iteration against the main model window', () => {
    const c = contextFromResult({
      usage: {
        input_tokens: 99,
        iterations: [
          { input_tokens: 1, cache_read_input_tokens: 10, cache_creation_input_tokens: 5 },
          { input_tokens: 2, cache_read_input_tokens: 11563, cache_creation_input_tokens: 12474 },
        ],
      },
      modelUsage: {
        'claude-haiku-5-5': { contextWindow: 200000 },
        'claude-opus-5-5': { contextWindow: 1000000 },
      },
    })
    expect(c).toEqual({ used: 24039, window: 1000000, model: 'claude-opus-5-5' })
  })

  it('returns nothing without usage and formats token counts compactly', () => {
    expect(contextFromResult({ modelUsage: {} })).toBeNull()
    expect(usedTokens(undefined)).toBe(0)
    expect(fmtTokens(24039)).toBe('24k')
    expect(fmtTokens(1_000_000)).toBe('1.0M')
    expect(fmtTokens(512)).toBe('512')
  })
})
