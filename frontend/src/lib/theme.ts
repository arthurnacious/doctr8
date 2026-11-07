/**
 * Dark by default; light is opt-in and remembered per browser.
 * The choice is a `data-theme` attribute on <html>; all colours are CSS
 * variables keyed off it (src/styles.css).
 */
import { useSyncExternalStore } from 'react'

export type Theme = 'dark' | 'light'

export const THEME_STORAGE_KEY = 'doctr8.theme'

/**
 * Runs in <head> before first paint so a light-mode user never sees a flash
 * of dark. Kept tiny and dependency-free.
 */
export const themeBootScript = `(function(){try{if(localStorage.getItem('${THEME_STORAGE_KEY}')==='light'){document.documentElement.dataset.theme='light'}}catch(e){}})()`

const listeners = new Set<() => void>()

export function getTheme(): Theme {
  if (typeof document === 'undefined') return 'dark'
  return document.documentElement.dataset.theme === 'light' ? 'light' : 'dark'
}

export function setTheme(theme: Theme): void {
  const root = document.documentElement
  if (theme === 'light') root.dataset.theme = 'light'
  else delete root.dataset.theme
  try {
    if (theme === 'light') localStorage.setItem(THEME_STORAGE_KEY, 'light')
    else localStorage.removeItem(THEME_STORAGE_KEY)
  } catch {
    // Private mode or blocked storage: the theme still applies for this page.
  }
  for (const l of listeners) l()
}

export function toggleTheme(): void {
  setTheme(getTheme() === 'dark' ? 'light' : 'dark')
}

function subscribe(listener: () => void) {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

export function useTheme(): Theme {
  return useSyncExternalStore(subscribe, getTheme, () => 'dark')
}
