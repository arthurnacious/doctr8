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
      coding/   ICD-10, tariff, NAPPI and modifier reference (global)
      billing/  providers, invoices, transactions, statements
    common/ws/  WebSocket endpoint, event bus, per-connection filtering
  examples/     ws_probe (WebSocket client for testing)
  tests/e2e/    end-to-end suite
frontend/       TanStack Start app (see frontend/README.md)
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

Local development uses one Postgres login (see `backend/.env`):

| Setting | Value |
|---------|-------|
| host / port | `127.0.0.1` / `5432` |
| user | `postgres` |
| dev database | `doctr8` |
| test database | `doctr8_test` (dropped and recreated by `tests/e2e/run.sh`) |

`postgres` is a superuser, which bypasses row-level security. The API therefore
switches every connection to the `doctr8_app` role (`DATABASE_ROLE=doctr8_app`,
created by migration 0001 as `NOLOGIN NOSUPERUSER NOBYPASSRLS`) and **refuses
to start** if it would run with RLS bypassed. In production, prefer logging in
as a dedicated `doctr8_app` login rather than switching roles from a superuser.

First-time setup:

```sh
psql -h 127.0.0.1 -U postgres -c 'CREATE DATABASE doctr8'
cd backend
cp .env.example .env          # then fill in the password and JWT_SECRET
cargo run -- migrate          # applies migrations via MIGRATION_DATABASE_URL
cargo run                     # API on BIND_ADDR, reading backend/.env
```

`cargo run -- migrate` uses the same `_sqlx_migrations` table as
`sqlx migrate run`, so either works.

## Testing

```sh
cd backend
cargo test              # unit tests
tests/e2e/run.sh        # end-to-end: throwaway Postgres + migrations + API + curl checks
```

`run.sh` needs `psql`, `curl` and `jq` on `PATH` and uses `TEST_DATABASE_URL`
(it refuses any database whose name doesn't end in `_test`). The API runs as
`doctr8_app`, so row-level security is exercised for real.

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

## Billing ledger

Money is integer cents. Every movement is a `transaction` with signed
`transaction_entries` per invoice and bucket (`medical_aid` | `patient`);
what each party owes is the sum of entries. Posted money, invoices and
credit notes are immutable (triggers + grants); corrections are new
transactions (reversal, journal, credit note). Shapes are re-validated at
COMMIT by deferred triggers (`app.validate_transaction`, `app.validate_invoice`,
`app.validate_credit_note`).

## Real-time

`GET /ws` — send `{"type":"auth","token":"<access JWT>"}` first (and again
after each refresh), then `{"type":"subscribe","account_id":"…"}` for open
account screens. Services publish with `pg_notify` inside their transaction,
so events are delivered only after COMMIT. Events carry ids only; clients
refetch over REST. Protocol details: `backend/src/common/ws/mod.rs`.

## Domains

- [x] Practices & access — schema + `backend/src/modules/access`
- [x] Patients & medical aid — `0002_patients_and_medical_aid.sql` + `backend/src/modules/people`
- [x] Billing & statements — `0003_billing.sql` + `backend/src/modules/{billing,coding}`
- [ ] Booking
