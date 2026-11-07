/**
 * Placeholder for the Ctrl/Cmd K command palette. The top bar's search
 * button and the keyboard shortcut call `openCommandPalette()`; it does
 * nothing until the palette is built.
 */
export function openCommandPalette(): void {
  // Intentionally empty for now.
}

/** "⌘ K" on Apple platforms, "Ctrl K" elsewhere. */
export function paletteShortcutLabel(
  platform: string = typeof navigator === 'undefined' ? '' : navigator.platform,
): string {
  return /Mac|iPhone|iPad/.test(platform) ? '⌘ K' : 'Ctrl K'
}
