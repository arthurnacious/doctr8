import { defineConfig } from 'vitest/config'

// Unit tests for src/lib: no TanStack Start plugin needed.
export default defineConfig({
  resolve: { tsconfigPaths: true },
  test: {
    environment: 'jsdom',
    include: ['src/**/*.test.ts', 'src/**/*.test.tsx'],
    restoreMocks: true,
  },
})
