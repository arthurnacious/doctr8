/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** "true" enables the client-side two-step stub (src/lib/auth/stubs.ts). */
  readonly VITE_TWO_STEP_STUB?: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}
