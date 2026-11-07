import { beforeEach, describe, expect, it } from 'vitest'

import {
  THEME_STORAGE_KEY,
  getTheme,
  setTheme,
  themeBootScript,
  toggleTheme,
} from './theme'
import { paletteShortcutLabel } from './command-palette'

describe('theme', () => {
  beforeEach(() => {
    localStorage.clear()
    delete document.documentElement.dataset.theme
  })

  it('is dark by default', () => {
    expect(getTheme()).toBe('dark')
  })

  it('toggles and remembers light mode', () => {
    toggleTheme()
    expect(document.documentElement.dataset.theme).toBe('light')
    expect(localStorage.getItem(THEME_STORAGE_KEY)).toBe('light')
    setTheme('dark')
    expect(document.documentElement.dataset.theme).toBeUndefined()
    expect(localStorage.getItem(THEME_STORAGE_KEY)).toBeNull()
  })

  it('boot script applies a saved light theme before paint', () => {
    localStorage.setItem(THEME_STORAGE_KEY, 'light')
    new Function(themeBootScript)()
    expect(getTheme()).toBe('light')
  })
})

describe('command palette shortcut label', () => {
  it('matches the platform', () => {
    expect(paletteShortcutLabel('MacIntel')).toBe('⌘ K')
    expect(paletteShortcutLabel('Win32')).toBe('Ctrl K')
    expect(paletteShortcutLabel('Linux x86_64')).toBe('Ctrl K')
  })
})
