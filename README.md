# doctr8

Multi-tenant EMR and practice management system for South African practices.

## Stack

| Layer    | Tech                                                                 |
|----------|----------------------------------------------------------------------|
| Frontend | TanStack Start, Tailwind CSS                                         |
| Backend  | Rust, Axum — one module per domain (`handler`, `service`, `dto`, `repository`) |
| Database | PostgreSQL — `tenant_id` on every tenant-scoped table, row-level security |
| Auth     | JWT: short-lived access tokens, rotating refresh tokens              |

## Layout

```
backend/
  migrations/   SQL migrations (sqlx-style: <version>_<name>.sql)
frontend/       (coming)
```

## Multi-tenancy

- A **tenant** is a practice. One **user** login can belong to several practices
  through **memberships**.
- Each practice defines its own **roles** and assigns **permissions** to them.
  Permissions are a fixed list defined in code (Rust `Permission` enum).
- The API connects as `doctr8_app` (`NOBYPASSRLS`). Each request runs in a
  transaction that first sets:

  ```sql
  SELECT set_config('app.user_id',   $1, true),
         set_config('app.tenant_id', $2, true);
  ```

  Values come from the verified JWT and are transaction-local, so pooled
  connections never leak tenant context. With no context set, tenant-scoped
  queries return nothing.

## Auth flow

1. `POST /auth/login` — email + password → session + refresh token (HttpOnly
   cookie) and the list of practices the user is an active member of.
2. `POST /auth/select-practice` — verifies membership, resolves the user's
   permissions in that practice, rotates the refresh token, and issues an access
   token `{ sub, tid, perms, exp }`.
3. `POST /auth/refresh` — consumes the current refresh token and issues a new
   one. Reusing a consumed token revokes the whole session.
4. Switching practice repeats step 2; no re-login needed.

## Database

Run migrations as the schema owner (not `doctr8_app`), e.g. with sqlx:

```sh
cd backend
sqlx migrate run
```

`doctr8_app` is created `NOLOGIN` by the migration; grant it `LOGIN` and a
password in your environment.

## Domains

- [x] Practices & access — `backend/migrations/0001_practices_and_access.sql`
- [ ] Patients
- [ ] Booking
- [ ] Billing
