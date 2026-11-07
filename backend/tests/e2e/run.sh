#!/usr/bin/env bash
# End-to-end tests for every module.
# Recreates the test database (TEST_DATABASE_URL, from the environment or
# backend/.env), applies migrations with `doctr8-api migrate`, starts the API
# with DATABASE_ROLE=doctr8_app (RLS enforced), runs every suite, and checks
# the debug log for test patients' personal details.
# Needs: psql, curl, jq on PATH, and a reachable Postgres.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORK="$(mktemp -d)"
API_PORT="${E2E_API_PORT:-38080}"
SRV=""

cleanup() {
  [[ -n "$SRV" ]] && kill "$SRV" 2>/dev/null && wait "$SRV" 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT

if [[ -z "${TEST_DATABASE_URL:-}" && -f "$ROOT/.env" ]]; then
  TEST_DATABASE_URL="$(sed -n 's/^TEST_DATABASE_URL=//p' "$ROOT/.env" | tail -1)"
fi
: "${TEST_DATABASE_URL:?set TEST_DATABASE_URL (e.g. in backend/.env)}"
DB_NAME="${TEST_DATABASE_URL##*/}"; DB_NAME="${DB_NAME%%\?*}"
ADMIN_URL="${TEST_DATABASE_URL%/*}/postgres"
# This database is dropped on every run: refuse anything that isn't clearly a test database.
[[ "$DB_NAME" == *_test ]] || { echo "TEST_DATABASE_URL must point at a database whose name ends in _test (got '$DB_NAME')"; exit 2; }

cargo build --quiet --manifest-path "$ROOT/Cargo.toml" --bins --examples

psql "$ADMIN_URL" -q -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS \"$DB_NAME\" WITH (FORCE)" -c "CREATE DATABASE \"$DB_NAME\""
MIGRATION_DATABASE_URL="$TEST_DATABASE_URL" RUST_LOG=warn "$ROOT/target/debug/doctr8-api" migrate
PSQL="psql $TEST_DATABASE_URL -q -v ON_ERROR_STOP=1"

DATABASE_URL="$TEST_DATABASE_URL" DATABASE_ROLE=doctr8_app \
JWT_SECRET="e2e-secret-e2e-secret-e2e-secret-e2e" \
BIND_ADDR="127.0.0.1:$API_PORT" COOKIE_SECURE=false RUST_LOG=debug \
  "$ROOT/target/debug/doctr8-api" >"$WORK/api.log" 2>&1 &
SRV=$!

for _ in $(seq 100); do curl -sf "127.0.0.1:$API_PORT/health" >/dev/null && break; sleep 0.1; done
curl -sf "127.0.0.1:$API_PORT/health" >/dev/null || { echo "API did not start:"; cat "$WORK/api.log"; exit 1; }

set +e
status=0
for suite in access people billing; do
  echo "##### $suite"
  "$ROOT/tests/e2e/$suite.sh" "http://127.0.0.1:$API_PORT" "$PSQL" | tee "$WORK/$suite.result"
  grep -q 'RESULT: .* 0 failed' "$WORK/$suite.result" || status=1
done

# Personal details used by the suites must never reach the API log.
echo "##### privacy"
PII='Nkosi|Thandi|Dlamini|Mokoena|Khumalo|Zanele|Mthembu|Bongani|12 Long St|9002150123088|8507045000086|8001015009087|1203105000086|0825551234|thabo@example'
if grep -Eiq "$PII" "$WORK/api.log"; then
  echo "  FAIL personal details found in API log:"; grep -Eio "$PII" "$WORK/api.log" | sort | uniq -c
  status=1
else
  echo "  PASS no personal details in $(wc -l < "$WORK/api.log") lines of debug log"
fi

[[ $status -ne 0 ]] && { echo "--- api log (tail):"; tail -50 "$WORK/api.log"; }
exit $status
