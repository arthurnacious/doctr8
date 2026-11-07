import { defineConfig } from 'vite'
import { devtools } from '@tanstack/devtools-vite'
import { tanstackStart } from '@tanstack/react-start/plugin/vite'
import viteReact from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

/** The Rust/Axum API. Override with DOCTR8_API_URL. */
const apiTarget = process.env.DOCTR8_API_URL ?? 'http://127.0.0.1:3000'

const config = defineConfig({
  resolve: { tsconfigPaths: true },
  plugins: [devtools(), tailwindcss(), tanstackStart(), viteReact()],
  server: {
    // The browser only ever talks to this origin. /api/* is forwarded to the
    // API with the prefix removed, so the refresh-token cookie is first-party
    // and the API needs no CORS. The API scopes that cookie to Path=/auth;
    // rewrite it to /api/auth so the browser sends it back through the proxy.
    proxy: {
      '/api': {
        target: apiTarget,
        changeOrigin: true,
        ws: true,
        rewrite: (path) => path.replace(/^\/api/, ''),
        cookiePathRewrite: { '/auth': '/api/auth' },
      },
    },
  },
})

export default config
