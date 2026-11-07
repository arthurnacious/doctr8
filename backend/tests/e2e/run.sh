#!/usr/bin/env bash
# End-to-end test for the access module.
# Spins up a throwaway PostgreSQL cluster, applies migrations, starts the API
# as doctr8_app (RLS enforced), runs tests/e2e/api.sh, then tears it all down.
# Needs: initdb, pg_ctl, psql, curl, jq on PATH.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORK="$(mktemp -d)"
PORT="${E2E_PG_PORT:-54329}"
API_PORT="${E2E_API_PORT:-38080}"
SRV=""

cleanup() {
  [[ -n "$SRV" ]] && kill "$SRV" 2>/dev/null && wait "$SRV" 2>/dev/null || true
  pg_ctl -D "$WORK/data" -w stop >/dev/null 2>&1 || true
  rm -rf "$WORK"
}
trap cleanup EXIT

cargo build --quiet --manifest-path "$ROOT/Cargo.toml"

initdb -D "$WORK/data" -U postgres -A trust >/dev/null
pg_ctl -D "$WORK/data" -o "-k $WORK -p $PORT -c listen_addresses=''" -l "$WORK/pg.log" -w start >/dev/null

PSQL="psql -h $WORK -p $PORT -U postgres -q -v ON_ERROR_STOP=1"
$PSQL -c 'CREATE DATABASE doctr8'
for f in "$ROOT"/migrations/*.sql; do $PSQL -d doctr8 -f "$f"; done
$PSQL -d doctr8 -c "ALTER ROLE doctr8_app LOGIN PASSWORD 'e2e'"

DATABASE_URL="postgres://doctr8_app:e2e@localhost/doctr8?host=$WORK&port=$PORT" \
JWT_SECRET="e2e-secret-e2e-secret-e2e-secret-e2e" \
BIND_ADDR="127.0.0.1:$API_PORT" COOKIE_SECURE=false RUST_LOG=warn \
  "$ROOT/target/debug/doctr8-api" >"$WORK/api.log" 2>&1 &
SRV=$!

for _ in $(seq 100); do curl -sf "127.0.0.1:$API_PORT/health" >/dev/null && break; sleep 0.1; done

set +e
"$ROOT/tests/e2e/api.sh" "http://127.0.0.1:$API_PORT" "$PSQL -d doctr8" | tee "$WORK/result"
grep -q 'RESULT: .* 0 failed' "$WORK/result"; status=$?
[[ $status -ne 0 ]] && { echo "--- api log:"; cat "$WORK/api.log"; }
exit $status
