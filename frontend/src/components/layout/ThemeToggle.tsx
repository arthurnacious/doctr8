import { Moon, Sun } from 'lucide-react'

import { IconButton } from '../ui/Button'
import { toggleTheme, useTheme } from '#/lib/theme'

export function ThemeToggle() {
  const theme = useTheme()
  return (
    <IconButton
      label={theme === 'dark' ? 'Switch to light mode' : 'Switch to dark mode'}
      onClick={toggleTheme}
    >
      {theme === 'dark' ? (
        <Sun className="size-5" aria-hidden="true" />
      ) : (
        <Moon className="size-5" aria-hidden="true" />
      )}
    </IconButton>
  )
}
