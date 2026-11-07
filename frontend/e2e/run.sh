#!/usr/bin/env bash
# Browser end-to-end tests against the real API.
# Recreates TEST_DATABASE_URL (from the environment or ../backend/.env; the
# name must end in _test), starts the API and the dev server on spare ports,
# seeds users, then runs Playwright.
# Needs: cargo, psql, curl, jq, and Chromium (system or Playwright's).
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FRONTEND="$(cd "$HERE/.." && pwd)"
BACKEND="$(cd "$FRONTEND/../backend" && pwd)"
WORK="$(mktemp -d)"
API_PORT="${E2E_API_PORT:-3100}"
WEB_PORT="${E2E_WEB_PORT:-5174}"
PIDS=()

cleanup() {
  for p in "${PIDS[@]}"; do kill "$p" 2>/dev/null || true; done
  wait 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT

if [[ -z "${TEST_DATABASE_URL:-}" && -f "$BACKEND/.env" ]]; then
  TEST_DATABASE_URL="$(sed -n 's/^TEST_DATABASE_URL=//p' "$BACKEND/.env" | tail -1)"
fi
: "${TEST_DATABASE_URL:?set TEST_DATABASE_URL (e.g. in backend/.env)}"
DB_NAME="${TEST_DATABASE_URL##*/}"; DB_NAME="${DB_NAME%%\?*}"
[[ "$DB_NAME" == *_test ]] || { echo "TEST_DATABASE_URL must end in _test (got '$DB_NAME')"; exit 2; }

echo "== building API"
cargo build --quiet --manifest-path "$BACKEND/Cargo.toml" --bins
API_BIN="$BACKEND/target/debug/doctr8-api"

echo "== database $DB_NAME"
psql "${TEST_DATABASE_URL%/*}/postgres" -q -v ON_ERROR_STOP=1 \
  -c "DROP DATABASE IF EXISTS \"$DB_NAME\" WITH (FORCE)" -c "CREATE DATABASE \"$DB_NAME\""
MIGRATION_DATABASE_URL="$TEST_DATABASE_URL" RUST_LOG=warn "$API_BIN" migrate

echo "== starting API on :$API_PORT"
DATABASE_URL="$TEST_DATABASE_URL" DATABASE_ROLE=doctr8_app \
JWT_SECRET="e2e-secret-e2e-secret-e2e-secret-e2e" BIND_ADDR="127.0.0.1:$API_PORT" \
COOKIE_SECURE=false RUST_LOG=warn "$API_BIN" >"$WORK/api.log" 2>&1 &
PIDS+=($!)
for _ in $(seq 100); do curl -sf "127.0.0.1:$API_PORT/health" >/dev/null && break; sleep 0.1; done

echo "== seeding"
API="http://127.0.0.1:$API_PORT"
post() { curl -sf -X POST "$API$1" -H 'content-type: application/json' ${3:+-H "authorization: Bearer $3"} -d "$2"; }
PW='correct horse battery'
register() { post /users "{\"email\":\"$1\",\"full_name\":\"$2\",\"password\":\"$PW\"}" | jq -r .id; }
login() { post /auth/login "{\"email\":\"$1\",\"password\":\"$PW\"${2:+,\"tenant_id\":\"$2\"}}" | jq -r .access_token; }

register owner@e2e.test "Owner Person" >/dev/null
T=$(login owner@e2e.test)
A=$(post /tenants '{"name":"Practice A","slug":"practice-a"}' "$T" | jq -r .id)
post /tenants '{"name":"Practice B","slug":"practice-b"}' "$T" >/dev/null
TA=$(login owner@e2e.test "$A")
ROLE=$(post /roles '{"name":"Reception","permissions":["patients.view"]}' "$TA" | jq -r .id)

REC=$(register reception@e2e.test "Front Desk")
psql "$TEST_DATABASE_URL" -q -c "INSERT INTO memberships (tenant_id, user_id, status, joined_at) VALUES ('$A', '$REC', 'active', now())"
curl -sf -X PUT "$API/members/$REC/roles" -H 'content-type: application/json' -H "authorization: Bearer $TA" \
  -d "{\"role_ids\":[\"$ROLE\"]}" >/dev/null

register nobody@e2e.test "No Practice" >/dev/null

# Patients in Practice A: enough for two pages, one on medical aid with an
# SA ID number, and one inactive.
read -r SCHEME PLAN < <(psql "$TEST_DATABASE_URL" -tA -F' ' -c "
  WITH s AS (INSERT INTO medical_schemes (name) VALUES ('E2E Health') RETURNING id),
       p AS (INSERT INTO medical_scheme_plans (scheme_id, name) SELECT id, 'Core' FROM s RETURNING id)
  SELECT (SELECT id FROM s), (SELECT id FROM p)")
for i in $(seq -w 1 28); do
  post /patient-files "{\"patients\":[{\"person\":{\"new\":{\"first_name\":\"Pat$i\",\"last_name\":\"Listtest\",\"date_of_birth\":\"1990-01-$i\"}}}],\"guarantor\":{\"patient\":0}}" "$TA" >/dev/null
done
post /patient-files "{\"patients\":[{\"person\":{\"new\":{\"first_name\":\"Naledi\",\"last_name\":\"Mokoena\",\"id_number\":\"8001015009087\"}}}],\"guarantor\":{\"patient\":0},\"medical_aid\":{\"new\":{\"scheme_id\":\"$SCHEME\",\"plan_id\":\"$PLAN\",\"membership_number\":\"E2E-77\",\"main_member\":{\"patient\":0}}}}" "$TA" >/dev/null
INACTIVE=$(post /patient-files '{"patients":[{"person":{"new":{"first_name":"Ina","last_name":"Ctive"}}}],"guarantor":{"patient":0}}' "$TA" | jq -r '.patients[0].patient_id')
curl -sf -X PUT "$API/patients/$INACTIVE/status" -H 'content-type: application/json' -H "authorization: Bearer $TA" -d '{"status":"inactive"}' >/dev/null

# A family file: Practice A switches to family-per-file just for this one.
patch_mode() { curl -sf -X PATCH "$API/tenants/current" -H 'content-type: application/json' -H "authorization: Bearer $TA" -d "{\"file_mode\":\"$1\"}" >/dev/null; }
patch_mode family_per_file
post /patient-files "{\"patients\":[{\"person\":{\"new\":{\"first_name\":\"Sipho\",\"last_name\":\"Dube\"}}},{\"person\":{\"new\":{\"first_name\":\"Ayanda\",\"last_name\":\"Dube\",\"mobile\":\"0821110000\"}},\"dependant_code\":\"01\"}],\"guarantor\":{\"patient\":0},\"medical_aid\":{\"new\":{\"scheme_id\":\"$SCHEME\",\"plan_id\":\"$PLAN\",\"membership_number\":\"FAM-9\",\"main_member\":{\"patient\":0}}}}" "$TA" >/dev/null
patch_mode patient_per_file

echo "== starting frontend on :$WEB_PORT"
cd "$FRONTEND"
DOCTR8_API_URL="$API" VITE_TWO_STEP_STUB=true VITE_DEVTOOLS=false npx vite dev --port "$WEB_PORT" --strictPort >"$WORK/web.log" 2>&1 &
PIDS+=($!)
for _ in $(seq 150); do curl -sf "localhost:$WEB_PORT/sign-in" >/dev/null && break; sleep 0.2; done

if [[ -z "${CHROMIUM_PATH:-}" ]] && command -v chromium >/dev/null; then
  export CHROMIUM_PATH="$(command -v chromium)"
fi
export E2E_BASE_URL="http://localhost:$WEB_PORT" E2E_PASSWORD="$PW" E2E_API_URL="$API"
set +e
npx playwright test "$@"
status=$?
[[ $status -ne 0 ]] && { echo "--- api log"; tail -30 "$WORK/api.log"; echo "--- web log"; tail -30 "$WORK/web.log"; }
exit $status
