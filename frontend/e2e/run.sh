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
post /tenants '{"name":"Practice C","slug":"practice-c"}' "$T" >/dev/null  # stays empty
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
DUBE=$(post /patient-files "{\"patients\":[{\"person\":{\"new\":{\"first_name\":\"Sipho\",\"last_name\":\"Dube\"}}},{\"person\":{\"new\":{\"first_name\":\"Ayanda\",\"last_name\":\"Dube\",\"mobile\":\"0821110000\"}},\"dependant_code\":\"01\"}],\"guarantor\":{\"patient\":0},\"medical_aid\":{\"new\":{\"scheme_id\":\"$SCHEME\",\"plan_id\":\"$PLAN\",\"membership_number\":\"FAM-9\",\"main_member\":{\"patient\":0}}}}" "$TA")
patch_mode patient_per_file

# Billing: a statement with two visits on a medical aid file.
#   visit 1, 100 days ago: claimed and paid in full by the medical aid;
#   visit 2, today: referred, authorised, part-paid by the medical aid, the
#   rest moved to the patient; a write-off journal posted and reversed;
#   plus an unallocated EFT from the patient.
# Global code tables (maintained by doctr8, not practices): seed as superuser.
psql "$TEST_DATABASE_URL" -q <<'SQL'
INSERT INTO icd10_codes (code, description) VALUES
  ('J06.9', 'Acute upper respiratory infection, unspecified'), ('Z00.0', 'General medical examination');
INSERT INTO modifiers (code, description) VALUES ('0018', 'Reduced fee');
INSERT INTO tariff_codes (code_set, code, description) VALUES
  ('CCSA', '0190', 'Consultation, new patient'), ('CCSA', '0191', 'Consultation, established patient');
INSERT INTO nappi_codes (code, description) VALUES ('716541001', 'Paracetamol 500mg x 20');
INSERT INTO icd10_codes (code, description, valid_as_primary) VALUES ('Y40.0', 'Adverse effect of penicillins', false);
INSERT INTO icd10_codes (code, description, sex, min_age_years, max_age_years)
  VALUES ('O80', 'Single spontaneous delivery', 'female', 12, 55);
SQL
# Tariff rates for this year: the E2E Health Core plan's own rate for 0190,
# and default (private) rates.
Y=$(TZ=Africa/Johannesburg date +%Y)
psql "$TEST_DATABASE_URL" -q -c "
  INSERT INTO tariff_prices (tariff_code_id, plan_id, year, price_cents)
    SELECT id, '$PLAN'::uuid, $Y, 52000 FROM tariff_codes WHERE code = '0190'
    UNION ALL SELECT id, NULL::uuid, $Y, 50000 FROM tariff_codes WHERE code = '0190'
    UNION ALL SELECT id, NULL::uuid, $Y, 39000 FROM tariff_codes WHERE code = '0191';"
api() { curl -sS --fail-with-body -X "$1" "$API$2" -H 'content-type: application/json' -H "authorization: Bearer $TA" -d "$3"; }
sa_date() { TZ=Africa/Johannesburg date -d "$1" +%F; }
api PATCH /tenants/current '{"details":{"address_line1":"1 Main Rd","city":"Durban","postal_code":"4001","phone":"031 555 1234","email":"accounts@practice-a.test","bank_name":"First Bank","bank_account_name":"Practice A","bank_account_number":"62 1234 5678","bank_branch_code":"250655"}}' >/dev/null
DOC=$(api POST /providers '{"kind":"practice","title":"Dr","first_name":"Thandi","last_name":"Doctor","practice_number":"1234567","hpcsa_number":"MP0123456"}' | jq -r .id)
REF=$(api POST /providers '{"kind":"external","title":"Dr","last_name":"Referrer","practice_number":"7654321"}' | jq -r .id)
ASSIST=$(api POST /providers '{"kind":"external","title":"Dr","last_name":"Assist","hpcsa_number":"MP7777777"}' | jq -r .id)
ROOMS=$(curl -sf "$API/service-centres" -H "authorization: Bearer $TA" | jq -r '.[0].id')
STM=$(api POST /patient-files "{\"patients\":[{\"person\":{\"new\":{\"first_name\":\"Lerato\",\"last_name\":\"Statement\",\"date_of_birth\":\"1985-04-12\"}}}],\"guarantor\":{\"patient\":0},\"medical_aid\":{\"new\":{\"scheme_id\":\"$SCHEME\",\"plan_id\":\"$PLAN\",\"membership_number\":\"STM-1\",\"main_member\":{\"patient\":0}}}}")
STM_ACC=$(jq -r .id <<<"$STM"); STM_PAT=$(jq -r '.patients[0].patient_id' <<<"$STM")
inv() { api POST /invoices "$(jq -nc --arg a "$STM_ACC" --arg p "$STM_PAT" --arg d "$DOC" --arg c "$ROOMS" --argjson lines "$1" --argjson extra "$2" \
  '{account_id:$a, patient_id:$p, treating_provider_id:$d, service_centre_id:$c, lines:$lines} + $extra')" | jq -r .id; }
OLD=$(inv '[{"tariff_code":"0190","unit_price_cents":52000,"icd10_codes":["J06.9"]}]' "{\"invoice_date\":\"$(sa_date '100 days ago')\"}")
NEW=$(inv "[{\"tariff_code\":\"0191\",\"unit_price_cents\":39000,\"modifiers\":[\"0018\"],\"icd10_codes\":[\"J06.9\",\"Z00.0\"],\"assistant_provider_id\":\"$ASSIST\"},{\"nappi_code\":\"716541001\",\"quantity\":\"2\",\"unit_price_cents\":1250,\"icd10_codes\":[\"J06.9\"]}]" "{\"referring_provider_id\":\"$REF\",\"authorisation_number\":\"AUTH 77\"}")
api POST /invoices/$OLD/claims '{}' >/dev/null; api POST /invoices/$NEW/claims '{}' >/dev/null
RA=$(api POST /remittances "{\"scheme_id\":\"$SCHEME\",\"reference\":\"RA-E2E\",\"received_date\":\"$(sa_date today)\",\"total_cents\":82000}" | jq -r .id)
api POST /remittances/$RA/allocations "{\"account_id\":\"$STM_ACC\",\"lines\":[{\"invoice_id\":\"$OLD\",\"amount_cents\":52000},{\"invoice_id\":\"$NEW\",\"amount_cents\":30000}]}" >/dev/null
api POST /liability-transfers "{\"invoice_id\":\"$NEW\",\"from\":\"medical_aid\",\"amount_cents\":9000,\"notes\":\"co-payment\"}" >/dev/null
WRITE_OFF=$(curl -sf "$API/journal-reason-types" -H "authorization: Bearer $TA" | jq -r '.[] | select(.code=="small_balance_write_off") | .id')
J=$(api POST /journals "{\"account_id\":\"$STM_ACC\",\"reason_type_id\":\"$WRITE_OFF\",\"notes\":\"x\",\"entries\":[{\"invoice_id\":\"$NEW\",\"bucket\":\"patient\",\"amount_cents\":-500}]}" | jq -r .id)
api POST /transactions/$J/reverse '{"notes":"posted in error"}' >/dev/null
# The Dube family file: one visit each for the main member and the dependant.
DUBE_ACC=$(jq -r .id <<<"$DUBE")
for code in 00 01; do
  P_ID=$(jq -r --arg c "$code" '.patients[] | select(.dependant_code == $c) | .patient_id' <<<"$DUBE")
  api POST /invoices "$(jq -nc --arg a "$DUBE_ACC" --arg p "$P_ID" --arg d "$DOC" --arg c "$ROOMS" \
    '{account_id:$a, patient_id:$p, treating_provider_id:$d, service_centre_id:$c, lines:[{tariff_code:"0190", unit_price_cents:52000, icd10_codes:["J06.9"]}]}')" >/dev/null
done
api POST /payments "{\"account_id\":\"$STM_ACC\",\"method\":\"eft\",\"amount_cents\":5000,\"reference\":\"EFT-1\"}" >/dev/null

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
