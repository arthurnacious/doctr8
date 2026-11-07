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
  src/
    common/     errors, RLS-scoped transactions, JWT extractors, Require<P> guard
    modules/
      access/   auth, tenants, users, roles (mod/handler/service/dto/repository)
      people/   persons, memberships, accounts, patients
  tests/e2e/    end-to-end suite
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

Two roles, so the API can never bypass row-level security:

| Role | Used by | Privileges |
|------|---------|------------|
| `doctr8_owner` | migrations | owns the database, `BYPASSRLS`, `CREATEROLE` |
| `doctr8_app` | the API | `NOSUPERUSER NOBYPASSRLS`, DML grants only |

First-time setup (as a Postgres superuser):

```sh
psql -U postgres <<'SQL'
CREATE ROLE doctr8_owner LOGIN CREATEROLE BYPASSRLS PASSWORD '<owner password>';
CREATE DATABASE doctr8 OWNER doctr8_owner;
REVOKE ALL ON DATABASE doctr8 FROM PUBLIC;
SQL

cd backend
cp .env.example .env          # then fill in passwords and JWT_SECRET
cargo run -- migrate          # applies migrations as doctr8_owner; creates doctr8_app

psql -U postgres -d doctr8 <<'SQL'
ALTER ROLE doctr8_app LOGIN PASSWORD '<app password>';
GRANT CONNECT ON DATABASE doctr8 TO doctr8_app;
SQL

cargo run                     # API on BIND_ADDR, reading backend/.env
```

`cargo run -- migrate` uses `MIGRATION_DATABASE_URL` and the same
`_sqlx_migrations` table as `sqlx migrate run`, so either works.

## Testing

```sh
cd backend
cargo test              # unit tests
tests/e2e/run.sh        # end-to-end: throwaway Postgres + migrations + API + curl checks
```

`run.sh` needs `initdb`, `pg_ctl`, `psql`, `curl` and `jq` on `PATH`. The API
runs as `doctr8_app`, so row-level security is exercised for real.

## Patient data

- Never log names, ID numbers, contact details or addresses — UUIDs only.
  Request spans record method + path (no query strings); database errors are
  logged as SQLSTATE/constraint/table only. `tests/e2e/run.sh` fails if test
  patients' details appear in the debug log.
- Person search is `POST /persons/search` so search terms stay out of URLs.

## File modes

Each practice has a `file_mode` (`patient_per_file`, `family_per_file`,
`debtor_per_visit`). Every account is stamped with the mode in force when it
was opened and keeps it; changing the practice's mode only affects new
accounts. Rules are checked in `people::accounts::service::check_file_rules`
and re-checked at commit by deferred triggers (`app.validate_account`).

## Domains

- [x] Practices & access — schema + `backend/src/modules/access`
- [x] Patients & medical aid — `0002_patients_and_medical_aid.sql` + `backend/src/modules/people`
- [ ] Booking
- [ ] Billing
