# doctr8 frontend

TanStack Start (React 19) with TanStack Router and Query, Tailwind CSS v4,
ESLint and Prettier. It talks to the Rust/Axum API in `../backend`.

## Running it

Needs Node 22.12+ (`.node-version` pins 26; mise picks it up) and the API
running on `127.0.0.1:3000` (see `../backend/README.md`).

```sh
cd frontend
npm install
npm run dev        # http://localhost:5173
```

| Script              | What it does                                         |
| ------------------- | ---------------------------------------------------- |
| `npm run dev`       | Dev server on port 5173 with the API proxied at /api |
| `npm run build`     | Production build into `dist/`                        |
| `npm run preview`   | Serve the production build                           |
| `npm run lint`      | ESLint                                               |
| `npm run typecheck` | `tsc --noEmit`                                       |
| `npm run test`      | Vitest unit tests                                    |
| `npm run test:e2e`  | Playwright against the real API (see below)          |
| `npm run check`     | Prettier check + ESLint + typecheck (run before PRs) |
| `npm run format`    | Prettier write + ESLint fix                          |

### How the browser reaches the API

The browser only ever talks to the frontend's origin. The dev server forwards
`/api/*` to the API with the `/api` prefix removed (WebSockets included), so
the refresh-token cookie is first-party and the API needs no CORS. The API
scopes that cookie to `Path=/auth`; the proxy rewrites it to `/api/auth` so the
browser sends it back. Set `DOCTR8_API_URL` to point the proxy elsewhere.
Production needs the same arrangement from its reverse proxy.

### Browser tests

`npm run test:e2e` runs `e2e/run.sh`. It recreates the API's test database
(`TEST_DATABASE_URL` from `../backend/.env`; the name must end in `_test`),
starts the API on :3100 and the dev server on :5174, seeds users, and runs
the Playwright specs in Chromium. It uses the system `chromium` if one is
installed, otherwise run `npx playwright install chromium` once.

### Development stubs

`.env.development` turns on `VITE_TWO_STEP_STUB`, which simulates two-step
verification on the client because the API doesn't have it yet. The code is
`000000`, and any recovery code shaped like `ABCD-1234` is accepted. **It
isn't security**, and the screen says so. See "Missing backend endpoints" below.

## Folder structure

```
src/
  routes/                    file-based routes (TanStack Router)
    __root.tsx               HTML shell, theme bootstrap, devtools
    _auth.tsx                auth layout: centred, no sidebar, mark in the corner
    _auth/sign-in.tsx
    _auth/two-step.tsx
    _auth/choose-practice.tsx
    _app.tsx                 app layout + route guard: sidebar, top bar, realtime
    _app/index.tsx           dashboard ("/")
    _app/<section>.tsx       diary, billing, claims, finance, settings
    _app/patients/           list, file view (+ editing), new-file flow,
                             account statement
  features/
    patients/                API hooks, types, formatting, error wording,
                             new-file/ and edit/ form models (zod) and forms
    billing/                 statement: API hook, types, rand formatting,
                             visit grouping (statement.ts) and StatementView
  components/
    ui/                      Button, TextField, PasswordField, Checkbox, Tabs …
    form/                    TanStack Form hook + field components (useAppForm)
    layout/                  Sidebar, TopBar, PracticeSwitcher, UserMenu, Logo
  lib/
    api/                     fetch client (auth header, 401 → refresh → retry), errors
    auth/                    session store, sign-in flow, guards, permissions, stubs
    ws/                      WebSocket connection to the API's /ws
    theme.ts                 dark/light theme with no flash on load
    nav.ts                   navigation items and the permissions each needs
  integrations/              TanStack Query provider and devtools
  router.tsx                 router + query client
  styles.css                 design tokens (CSS variables) + Tailwind
  test/                      unit-test fixtures
e2e/                         Playwright specs + run.sh (real API, test database)
```

Routes beginning with `_` are layouts that don't add to the URL. Pages own
their data with TanStack Query. Shared, server-free logic lives in `lib/`
and has unit tests next to it (`*.test.ts`).

## Design rules

These are rules, not suggestions. Review against them.

**Theme**

- Dark mode is the default; a toggle switches to light and is remembered per
  browser.
- Every colour is a CSS variable in `src/styles.css`, exposed to Tailwind as
  `bg-page`, `bg-panel`, `bg-card`, `bg-raised`, `border-border`,
  `border-border-strong`, `text-text`, `text-muted`, `text-subtle`,
  `bg-accent`, `text-on-accent`, `text-danger` and so on. Don't use raw
  Tailwind colours (`bg-neutral-800`) or hex values in components.
- Dark surfaces: neutral-950 page, neutral-900 panels, neutral-800 cards and
  inputs, neutral-700/600 for borders, hovers and raised elements.

**Accent**

- One accent green, `#3EE26C`, used for primary buttons, focus states and key
  numbers only. Text on green is dark (`#04210D`, `text-on-accent`).

**Flat**

- No gradients anywhere. Flat colours only. No glows, shadows-as-decoration,
  blur or other decorative effects.

**Accessible**

- Minimum touch target 44px (`min-h-11` / `size-11`) for anything clickable.
- Every input has a real, visible `<label>`. Placeholders are hints, not labels.
- Keyboard focus is always visible (a 2px focus-colour outline, global in
  `styles.css`). Never remove outlines without a replacement.
- Errors are text, linked to their field with `aria-describedby`, not colour
  alone.

**Content**

- No fake data: no placeholder charts, numbers or sample patients. Unbuilt
  sections show a plain empty state.
- Never put patient details in URLs, `console` output or error reports.
  ESLint forbids `console`; patient search text stays in memory (never the
  URL or storage) and searches are POSTs.

**Print**

- Printed pages are A4, white with dark text whatever the theme
  (`@media print` in `styles.css`). Mark screen-only UI with `no-print`;
  green on paper is for small headings only.
- Repeating page headers and page numbers use `@page` margin boxes
  (`PrintPageSetup`): Chromium-based browsers print them, others don't.

**Forms**

- TanStack Form (`useAppForm` from `components/form`) with zod schemas, kept
  in a `model.ts` with unit tests. Errors show once a field is touched or a
  save is attempted.
- Show backend errors through `friendlyError`, which turns file-mode and
  duplicate errors into plain language with a next step.
- Only render actions the user's permissions allow (`useCan`).

## How auth works

The API's access module issues a short-lived access token (JWT, about 10
minutes) and a rotating refresh token.

**Tokens**

- The **access token lives in memory only**: a module-level store in
  `lib/auth/session.ts`, never in `localStorage`, `sessionStorage` or cookies.
  Reloading the page loses it; the app gets a new one from the refresh token.
- The **refresh token** is an httpOnly, Secure, SameSite=Strict cookie set by
  the API on `/auth/*`. The frontend never reads it; the browser sends it to
  `/api/auth/refresh`, `/api/auth/switch-practice` and `/api/auth/logout`.

**Signing in**

1. **Sign in** posts email and password to `/auth/login`.
2. **Two-step code** (stubbed for now): 6 digits, or a recovery code, with
   "don't ask again on this device for 30 days".
3. **Choose a practice** appears only when the user belongs to more than one
   practice and none was chosen. The API picks the practice itself when there's
   only one.
4. You land on the page you originally asked for, or the dashboard.

**Keeping the session alive**

- The access token is refreshed shortly before it expires.
- The API client (`lib/api/client.ts`) adds `Authorization: Bearer …` to
  every request. On a 401 it refreshes once and retries the request once; if
  that fails, the session ends and the user is sent to sign in.
- Refreshes are single-flight within a tab and coordinated across tabs with
  the Web Locks API. Two tabs never present the same refresh token, which the
  API would treat as theft and revoke the session. A tab that refreshes shares
  the new access token with the other tabs over a `BroadcastChannel`, in
  memory only. Sign-out and practice switches are broadcast the same way.

**Route guards**

- `_app.tsx`'s `beforeLoad` waits for the session (refreshing from the cookie
  on first load) and redirects signed-out users to `/sign-in?redirect=…`, so
  app pages never render for them.
- Sections that need a permission check it in their own `beforeLoad`.
- Auth and app layouts render in the browser only (`ssr: false`), because the
  server never has the user's token.

**Switching practice** posts to `/auth/switch-practice`. That gives a new
token scoped to the new practice, clears the whole TanStack Query cache,
reloads permissions (which drive navigation and guards) and reconnects the
WebSocket.

**Signing out** posts to `/auth/logout`, clears the in-memory session and
the query cache, closes the WebSocket and tells other tabs.

**Permissions** come from the access token response (`permissions`). Use
`can('billing.view')` or `<Can permission="…">` in components, and
`requirePermission()` in `beforeLoad`. Navigation items declare the
permissions they need in `lib/nav.ts` and are hidden otherwise. The API
remains the real enforcement point.

### WebSocket

After sign-in the app layout opens `/api/ws` and authenticates with the first
message (`{"type":"auth","token":…}`), never in the URL.

- **Token refresh:** the new token is sent on the open socket, so there's no
  gap in events.
- **Practice switch:** the socket is closed and reopened with the new
  practice's token.
- **Dropped connections:** it reconnects with backoff. Close code 4001 means
  refresh, then reconnect; 4009 means reconnect with the current token; 4003
  means don't reconnect.
- **Sign out:** the socket is closed.

Nothing listens to events yet. Later, events will invalidate TanStack Query
keys (see `../backend/src/common/ws/mod.rs` for the protocol).

### Missing backend endpoints

These screens are built against clearly marked stubs in
`src/lib/auth/stubs.ts` until the API provides:

| Endpoint                                                                                             | Needed for                                                                                                                                                                              |
| ---------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `POST /auth/login` → `{ two_step_required: true, challenge_token }`                                  | Login must not issue tokens or set the refresh cookie until the second step passes                                                                                                      |
| `POST /auth/two-step/verify` `{ challenge_token, code \| recovery_code, remember_device }`           | Verifying the 6-digit code or a recovery code; issues tokens + refresh cookie, and an httpOnly device-trust cookie (30 days) when `remember_device` is set                              |
| `GET/POST /auth/two-step/setup`, `POST /auth/two-step/confirm`, `POST /auth/two-step/recovery-codes` | Enrolling an authenticator app and issuing recovery codes                                                                                                                               |
| `remember: bool` on `POST /auth/login`                                                               | "Keep me signed in": the API should issue a session-only refresh cookie (no `Max-Age`) and a shorter session when it is false. The frontend already sends it; the API ignores it today. |
