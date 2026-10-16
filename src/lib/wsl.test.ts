import { describe, expect, it } from 'vitest'
import { baseName, wslDistro, wslParent } from './wsl'

describe('wslDistro', () => {
  it('reads the distro from WSL UNC paths', () => {
    expect(wslDistro('\\\\wsl.localhost\\Ubuntu\\home\\me\\repo')).toBe('Ubuntu')
    expect(wslDistro('\\\\wsl$\\Debian\\srv')).toBe('Debian')
    expect(wslDistro('//wsl.localhost/Ubuntu/home')).toBe('Ubuntu')
    expect(wslDistro('\\\\?\\UNC\\wsl.localhost\\Ubuntu\\home')).toBe('Ubuntu')
  })

  it('is null for other paths', () => {
    expect(wslDistro('C:\\Users\\me\\repo')).toBeNull()
    expect(wslDistro('/home/me/repo')).toBeNull()
    expect(wslDistro('\\\\server\\share\\repo')).toBeNull()
    expect(wslDistro(undefined)).toBeNull()
  })
})

describe('wslParent / baseName', () => {
  it('walks up until the distro root', () => {
    expect(wslParent('\\\\wsl.localhost\\Ubuntu\\home\\me')).toBe('\\\\wsl.localhost\\Ubuntu\\home')
    expect(wslParent('\\\\wsl.localhost\\Ubuntu\\home')).toBe('\\\\wsl.localhost\\Ubuntu')
    expect(wslParent('\\\\wsl.localhost\\Ubuntu')).toBeNull()
  })

  it('takes the last segment with either separator', () => {
    expect(baseName('\\\\wsl.localhost\\Ubuntu\\home\\me\\acme')).toBe('acme')
    expect(baseName('/home/me/acme/')).toBe('acme')
  })
})
