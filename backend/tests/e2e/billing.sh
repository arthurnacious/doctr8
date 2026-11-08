#!/usr/bin/env bash
# Billing e2e: providers, invoices, claims, remittances, payments, refunds,
# credit notes, journals, liability transfers, reversals, database backstops,
# statements with ageing, and WebSocket events (incl. permission filtering).
. "$(dirname "$0")/lib.sh" "$@"

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PROBE="$ROOT/target/debug/examples/ws_probe"
WS="${B/http:/ws:}/ws"
j() { jq -r "$@" <<<"$BODY"; }
sa_date() { TZ=Africa/Johannesburg date -d "$1" +%F; }
TODAY=$(sa_date today)

# invoice <account> <patient> <treating> <lines-json> [extra-root-json]
invoice() {
  local extra=${5:-}; [[ -z $extra ]] && extra='{}'
  jq -nc --arg a "$1" --arg p "$2" --arg d "$3" --arg c "$ROOMS" --argjson lines "$4" --argjson extra "$extra" \
    '{account_id:$a, patient_id:$p, treating_provider_id:$d, service_centre_id:$c, lines:$lines} + $extra'
}
CONSULT='[{"tariff_code":"0190","unit_price_cents":52000,"icd10_codes":["J06.9"]}]'

echo "== setup"
req POST /users "" '{"email":"billing@clinic.co.za","full_name":"Dr Billing","password":"a very good password"}'; OWNER_USER=$(j .id)
req POST /auth/login "" '{"email":"billing@clinic.co.za","password":"a very good password"}' "$J/owner"; T=$(j .access_token)
req POST /tenants "$T" '{"name":"Billing Clinic","slug":"billing-clinic","pcns_number":"1234567"}'; P=$(j .id)
req POST /auth/switch-practice "" "{\"tenant_id\":\"$P\"}" "$J/owner"; T=$(j .access_token)
check "owner has all 17 permissions" 17 "$(jq '.permissions | length' <<<"$BODY")"

$PSQL -q <<'SQL'
INSERT INTO icd10_codes (code, description) VALUES
  ('J06.9', 'Acute upper respiratory infection, unspecified'), ('Z00.0', 'General medical examination');
INSERT INTO icd10_codes (code, description, valid_as_primary) VALUES ('Y40.0', 'Adverse effect of penicillins', false);
INSERT INTO modifiers (code, description) VALUES ('0018', 'Reduced fee'), ('0009', 'Assistant');
INSERT INTO tariff_codes (code_set, code, description) VALUES
  ('CCSA', '0190', 'Consultation, new patient'), ('CCSA', '0191', 'Consultation, established patient');
INSERT INTO nappi_codes (code, description) VALUES ('716541001', 'Paracetamol 500mg x 20');
SQL
read -r SCHEME PLAN < <($PSQL -tA -F' ' -c "
  WITH s AS (INSERT INTO medical_schemes (name) VALUES ('Billing Health') RETURNING id),
       p AS (INSERT INTO medical_scheme_plans (scheme_id, name) SELECT id, 'Core' FROM s RETURNING id)
  SELECT (SELECT id FROM s), (SELECT id FROM p)")

req GET /coding/icd10?q=j06 "$T"; check "ICD-10 search by code prefix" J06.9 "$(j '.[0].code')"
req GET "/coding/tariffs?q=consult" "$T"; check "tariff search by description" 2 "$(jq length <<<"$BODY")"

# Reference rates and ICD-10 limits (global data, seeded as superuser).
Y=$(TZ=Africa/Johannesburg date +%Y)
$PSQL -q -c "
  INSERT INTO tariff_prices (tariff_code_id, plan_id, year, price_cents)
    SELECT id, NULL::uuid, $Y, 50000 FROM tariff_codes WHERE code = '0190'
    UNION ALL SELECT id, '$PLAN'::uuid, $Y, 52000 FROM tariff_codes WHERE code = '0190'
    UNION ALL SELECT id, NULL::uuid, $Y - 1, 45000 FROM tariff_codes WHERE code = '0190';
  INSERT INTO icd10_codes (code, description, sex, min_age_years, max_age_years)
    VALUES ('O80', 'Single spontaneous delivery', 'female', 12, 55);"
req GET "/coding/tariff-price?code=0190&plan_id=$PLAN&date=$TODAY" "$T"
check "plan rate for this year" "52000 plan" "$(j '"\(.price_cents) \(.source)"')"
req GET "/coding/tariff-price?code=0190&date=$TODAY" "$T"
check "private: default rate" "50000 default" "$(j '"\(.price_cents) \(.source)"')"
req GET "/coding/tariff-price?code=0190&plan_id=$PLAN&date=$((Y - 1))-06-01" "$T"
check "last year: falls back to that year's default" "45000 default $((Y - 1))" "$(j '"\(.price_cents) \(.source) \(.year)"')"
req GET "/coding/tariff-price?code=0191&date=$TODAY" "$T"; check "no rate -> 404" 404 $CODE
req GET "/coding/tariff-price?code=01&date=$TODAY" "$T"; check "bad code -> 422" 422 $CODE
req GET "/coding/icd10?q=O80" "$T"
check "ICD-10 search returns sex and age limits" "female 12 55" "$(j '.[0] | "\(.sex) \(.min_age_years) \(.max_age_years)"')"

req GET /service-centres "$T"; ROOMS=$(j '.[0].id')
check "Rooms seeded for new practice" "Rooms 11" "$(j '.[0] | "\(.name) \(.default_place_of_service)"')"
req POST /service-centres "$T" '{"kind":"hospital","name":"Umhlanga Hospital","practice_number":"5555555"}'
check "hospital defaults to POS 21" 21 "$(j .default_place_of_service)"
req GET /journal-reason-types "$T"; check "journal reasons seeded" 5 "$(jq length <<<"$BODY")"
WRITE_OFF=$(jq -r '.[] | select(.code=="small_balance_write_off") | .id' <<<"$BODY")
OTHER_REASON=$(jq -r '.[] | select(.code=="other") | .id' <<<"$BODY")
req POST /journal-reason-types "$T" '{"code":"Fee_Waiver","name":"Fee waiver"}'; check "custom journal reason" fee_waiver "$(j .code)"
req POST /journal-reason-types "$T" '{"code":"fee_waiver","name":"Again"}'; check "duplicate reason code -> 409" 409 $CODE

echo "== providers"
req POST /providers "$T" "{\"kind\":\"practice\",\"title\":\"Dr\",\"first_name\":\"Ayanda\",\"last_name\":\"Owner\",\"practice_number\":\"1234567\",\"hpcsa_number\":\"MP 0123456\",\"user_id\":\"$OWNER_USER\"}"
check "own provider linked to their login" "$OWNER_USER" "$(j .user_id)"; DOC=$(j .id)
check "HPCSA number normalised" MP0123456 "$(j .hpcsa_number)"
req POST /providers "$T" '{"kind":"external","title":"Dr","last_name":"Referrer","practice_number":"7654321"}'; REF=$(j .id)
req POST /providers "$T" '{"kind":"external","last_name":"Assist","hpcsa_number":"MP7777777"}'; ASSIST=$(j .id)
req POST /providers "$T" '{"kind":"external","last_name":"Dup","hpcsa_number":"mp7777777"}'; check "duplicate HPCSA -> 409" 409 $CODE
req POST /providers "$T" '{"kind":"practice","last_name":"Stranger","user_id":"00000000-0000-0000-0000-000000000999"}'
check "linking a non-member user -> 422" 422 $CODE
req GET "/providers?kind=external" "$T"; check "filter by kind" 2 "$(jq length <<<"$BODY")"

echo "== patient files"
req POST /patient-files "$T" '{"patients":[{"person":{"new":{"first_name":"Zanele","last_name":"Khumalo"}}}],"guarantor":{"patient":0}}'
PRIV=$(j .id); ZANELE=$(j '.patients[0].patient_id')
req POST /patient-files "$T" "{\"patients\":[{\"person\":{\"new\":{\"first_name\":\"Bongani\",\"last_name\":\"Mthembu\",\"address_line1\":\"12 Long St\",\"city\":\"Durban\",\"postal_code\":\"4001\"}}}],\"guarantor\":{\"patient\":0},\"medical_aid\":{\"new\":{\"scheme_id\":\"$SCHEME\",\"plan_id\":\"$PLAN\",\"membership_number\":\"BH-100\",\"main_member\":{\"patient\":0}}}}"
MA=$(j .id); BONGANI=$(j '.patients[0].patient_id')

echo "== websocket clients"
req POST /roles "$T" '{"name":"Reception","permissions":["patients.view"]}'; RROLE=$(j .id)
req POST /users "" '{"email":"reception@clinic.co.za","full_name":"Front Desk","password":"another good password"}'; REC_USER=$(j .id)
$PSQL -q -c "INSERT INTO memberships (tenant_id, user_id, status, joined_at) VALUES ('$P', '$REC_USER', 'active', now())"
req PUT /members/$REC_USER/roles "$T" "{\"role_ids\":[\"$RROLE\"]}"
req POST /auth/login "" "{\"email\":\"reception@clinic.co.za\",\"password\":\"another good password\",\"tenant_id\":\"$P\"}" "$J/rec"; R=$(j .access_token)
# Owner follows the medical aid account only; reception (no billing.view) follows it too.
"$PROBE" "$WS" "$T" "$MA" > "$J/ws_owner" 2>/dev/null & PROBE_OWNER=$!
"$PROBE" "$WS" "$R" "$MA" > "$J/ws_rec" 2>/dev/null & PROBE_REC=$!
for _ in $(seq 50); do [[ -s $J/ws_owner && -s $J/ws_rec ]] && break; sleep 0.1; done
check "owner socket ready" ready "$(head -1 "$J/ws_owner" | jq -r .type)"
check "reception socket ready" ready "$(head -1 "$J/ws_rec" | jq -r .type)"

echo "== invoices"
req POST /invoices "$T" "$(invoice "$PRIV" "$ZANELE" "$DOC" '[{"tariff_code":"0190","unit_price_cents":52000,"icd10_codes":["j069"]},{"nappi_code":"716541001","quantity":"2","unit_price_cents":1250,"icd10_codes":["J06.9"]}]')"
check "private invoice created" 201 $CODE; INV_P=$(j .id)
check "first invoice number" 1 "$(j .invoice_no)"
check "total = R520 + 2 x R12.50" 54500 "$(j .total_cents)"
check "private: patient owes it all" "0 54500" "$(j '"\(.medical_aid_due_cents) \(.patient_due_cents)"')"
check "description defaults from tariff" "Consultation, new patient" "$(j '.lines[0].description')"
check "place of service from Rooms" 11 "$(j '.lines[0].place_of_service')"
check "ICD-10 normalised" J06.9 "$(j '.lines[0].icd10_codes[0]')"
LINE_MED=$(j '.lines[1].id')

req POST /invoices "$T" "$(invoice "$MA" "$BONGANI" "$DOC" "[{\"tariff_code\":\"0191\",\"unit_price_cents\":52000,\"amount_cents\":39000,\"modifiers\":[\"0018\"],\"icd10_codes\":[\"J06.9\",\"Z00.0\"],\"assistant_provider_id\":\"$ASSIST\"}]" "{\"referring_provider_id\":\"$REF\",\"authorisation_number\":\"AUTH 77\"}")"
check "medical aid invoice created" 201 $CODE; INV_M=$(j .id)
check "medical aid owes it all" "39000 0" "$(j '"\(.medical_aid_due_cents) \(.patient_due_cents)"')"
check "modifier and explicit amount kept" "0018 39000" "$(j '"\(.lines[0].modifiers[0]) \(.lines[0].amount_cents)"')"
check "diagnoses kept in order" "J06.9,Z00.0" "$(j '.lines[0].icd10_codes | join(",")')"
check "referring provider" "Dr Referrer" "$(j .referring_provider.name)"
check "authorisation normalised" AUTH77 "$(j .authorisation_number)"

bad() { req POST /invoices "$T" "$1"; check "$2" 422 $CODE; }
bad "$(invoice "$PRIV" "$ZANELE" "$DOC" '[{"tariff_code":"0190","unit_price_cents":100,"icd10_codes":["A00.0"]}]')" "unknown ICD-10 -> 422"
bad "$(invoice "$PRIV" "$ZANELE" "$DOC" '[{"tariff_code":"0190","unit_price_cents":100,"icd10_codes":["Y40.0"]}]')" "non-primary ICD-10 first -> 422"
bad "$(invoice "$PRIV" "$ZANELE" "$DOC" '[{"tariff_code":"9999","unit_price_cents":100,"icd10_codes":["J06.9"]}]')" "unknown tariff -> 422"
bad "$(invoice "$MA" "$ZANELE" "$DOC" "$CONSULT")" "patient not on the account -> 422"
bad "$(invoice "$PRIV" "$ZANELE" "$REF" "$CONSULT")" "external doctor as treating provider -> 422"
bad "$(invoice "$PRIV" "$ZANELE" "$DOC" '[{"tariff_code":"0190","unit_price_cents":0,"icd10_codes":["J06.9"]}]')" "zero-total invoice -> 422"
bad "$(invoice "$PRIV" "$ZANELE" "$DOC" "$CONSULT" "{\"invoice_date\":\"$(sa_date tomorrow)\"}")" "future invoice date -> 422"

echo "== claims"
req POST /invoices/$INV_M/claims "$T" '{}'; check "claim submitted" submitted "$(j .status)"; CLAIM=$(j .id)
check "claims what the scheme owes" 39000 "$(j .claimed_cents)"
req POST /invoices/$INV_M/claims "$T" '{}'; check "second open claim -> 409" 409 $CODE
req POST /invoices/$INV_P/claims "$T" '{}'; check "claim on a private account -> 409" 409 $CODE
req POST /claims/$CLAIM/status "$T" '{"status":"accepted"}'; check "claim accepted" accepted "$(j .status)"
req POST /claims/$CLAIM/status "$T" '{"status":"submitted"}'; check "accepted -> submitted not allowed" 409 $CODE
check "history written by the database" "submitted,accepted" \
  "$($PSQL -tAc "SELECT string_agg(status, ',' ORDER BY changed_at) FROM claim_status_history WHERE claim_id = '$CLAIM'")"
req GET "/claims?status=accepted" "$T"; check "claims queue filters by status" 1 "$(jq length <<<"$BODY")"

echo "== remittances"
REMIT="{\"scheme_id\":\"$SCHEME\",\"reference\":\"RA-001\",\"received_date\":\"$TODAY\",\"total_cents\":50000}"
req POST /remittances "$T" "$REMIT"; RA=$(j .id); check "remittance captured, nothing allocated" 50000 "$(j .unallocated_cents)"
req POST /remittances "$T" "$REMIT"; check "same remittance twice -> 409" 409 $CODE
req POST /remittances/$RA/allocations "$T" "{\"account_id\":\"$PRIV\",\"lines\":[{\"invoice_id\":\"$INV_P\",\"amount_cents\":100}]}"
check "allocating to a private account -> 409" 409 $CODE
req POST /remittances/$RA/allocations "$T" "{\"account_id\":\"$MA\",\"lines\":[{\"invoice_id\":\"$INV_M\",\"amount_cents\":39001}]}"
check "paying more than the scheme owes -> 422" 422 $CODE
req POST /remittances/$RA/allocations "$T" "{\"account_id\":\"$MA\",\"lines\":[{\"invoice_id\":\"$INV_M\",\"amount_cents\":30000}]}"
check "partial allocation" 20000 "$(j .unallocated_cents)"
req GET /invoices/$INV_M "$T"
check "part-paid claim stays accepted" accepted "$(j '.claims[0].status')"
check "scheme still owes R90" 9000 "$(j .medical_aid_due_cents)"

req POST /invoices "$T" "$(invoice "$MA" "$BONGANI" "$DOC" '[{"tariff_code":"0191","unit_price_cents":10000,"icd10_codes":["Z00.0"]}]')"; INV_M2=$(j .id)
req POST /invoices/$INV_M2/claims "$T" '{}'; CLAIM2=$(j .id)
req POST /remittances/$RA/allocations "$T" "{\"account_id\":\"$MA\",\"lines\":[{\"invoice_id\":\"$INV_M2\",\"amount_cents\":10000}]}"
check "second allocation" 10000 "$(j .unallocated_cents)"
check "allocations listed" 2 "$(j '.allocations | length')"
req GET /invoices/$INV_M2 "$T"; check "fully paid claim moves to paid automatically" paid "$(j '.claims[0].status')"
req POST /remittances/$RA/allocations "$T" "{\"account_id\":\"$MA\",\"lines\":[{\"invoice_id\":\"$INV_M\",\"amount_cents\":9000},{\"invoice_id\":\"$INV_M2\",\"amount_cents\":1001}]}"
check "over-allocating a remittance -> 422" 422 $CODE
req GET "/remittances?unallocated_only=true" "$T"; check "unallocated remittances queue" 1 "$(jq length <<<"$BODY")"

echo "== liability transfer (co-payment)"
req POST /liability-transfers "$T" "{\"invoice_id\":\"$INV_M\",\"from\":\"medical_aid\",\"amount_cents\":9001,\"notes\":\"co-payment\"}"
check "transfer more than owed -> 422" 422 $CODE
req POST /liability-transfers "$T" "{\"invoice_id\":\"$INV_M\",\"from\":\"medical_aid\",\"amount_cents\":9000,\"notes\":\"co-payment scheme won't pay\"}"
check "co-payment moved to patient" 201 $CODE; TRANSFER=$(j .id)
check "transfer is two balancing entries" "0" "$(j '[.entries[].amount_cents] | add')"
req GET /invoices/$INV_M "$T"; check "now patient owes the R90" "0 9000" "$(j '"\(.medical_aid_due_cents) \(.patient_due_cents)"')"
req POST /liability-transfers "$T" "{\"invoice_id\":\"$INV_P\",\"from\":\"patient\",\"amount_cents\":100,\"notes\":\"x\"}"
check "no medical aid to transfer to on private account -> 422" 422 $CODE
req POST /claims/$CLAIM/status "$T" '{"status":"paid"}'; check "claim marked paid" paid "$(j .status)"

echo "== payments, credit notes, refunds"
req POST /payments "$T" "{\"account_id\":\"$PRIV\",\"method\":\"card\",\"amount_cents\":20000}"
check "card payment allocated to the invoice" "$INV_P -20000" "$(j '.entries[0] | "\(.invoice_id) \(.amount_cents)"')"
req GET /accounts/$PRIV/balance "$T"; check "patient owes R345" 34500 "$(j .patient_cents)"
req POST /payments "$T" "{\"account_id\":\"$PRIV\",\"method\":\"cash\",\"amount_cents\":40000,\"allocations\":[{\"invoice_id\":\"$INV_P\",\"amount_cents\":40000}]}"
check "allocating more than owed on an invoice -> 422" 422 $CODE
req POST /payments "$T" "{\"account_id\":\"$PRIV\",\"method\":\"eft\",\"amount_cents\":40000,\"reference\":\"EFT-9\"}"
check "overpayment: rest stays as credit" "-34500 -5500" "$(j '[.entries[] | .amount_cents] | map(tostring) | join(" ")')"
req GET /accounts/$PRIV/balance "$T"; check "R55 credit on account" "-5500 -5500" "$(j '"\(.patient_cents) \(.unallocated_cents)"')"

req POST /invoices/$INV_P/credit-notes "$T" "{\"reason\":\"medication not dispensed\",\"lines\":[{\"invoice_line_id\":\"$LINE_MED\"}]}"
check "credit note on one line" "2500 2500" "$(j '"\(.credited_cents) \(.lines[1].credited_cents)"')"
check "paid invoice credit becomes patient credit" -2500 "$(j .patient_due_cents)"
req POST /invoices/$INV_P/credit-notes "$T" "{\"reason\":\"again\",\"lines\":[{\"invoice_line_id\":\"$LINE_MED\"}]}"
check "crediting a fully credited line -> 422" 422 $CODE
req GET /accounts/$PRIV/balance "$T"; check "credit now R80" -8000 "$(j .patient_cents)"
req POST /refunds "$T" "{\"account_id\":\"$PRIV\",\"method\":\"eft\",\"amount_cents\":8001}"; check "refund more than credit -> 422" 422 $CODE
req POST /refunds "$T" "{\"account_id\":\"$PRIV\",\"method\":\"eft\",\"amount_cents\":8000}"; check "refund the credit" 201 $CODE
req GET /accounts/$PRIV/balance "$T"; check "private account settled" "0 0 0" "$(j '"\(.medical_aid_cents) \(.patient_cents) \(.unallocated_cents)"')"

echo "== journals and reversals"
req POST /journals "$T" "{\"account_id\":\"$MA\",\"reason_type_id\":\"$WRITE_OFF\",\"notes\":\"small balance\",\"entries\":[{\"invoice_id\":\"$INV_M\",\"bucket\":\"patient\",\"amount_cents\":-500}]}"
check "write-off journal" 201 $CODE; JOURNAL=$(j .id)
check "journal shows its reason" small_balance_write_off "$(j .reason.code)"
req GET /accounts/$MA/balance "$T"; check "patient owes R85" 8500 "$(j .patient_cents)"
req POST /transactions/$JOURNAL/reverse "$T" '{"notes":"posted in error"}'; check "journal reversed" 201 $CODE; REVERSAL=$(j .id)
req GET /accounts/$MA/balance "$T"; check "back to R90" 9000 "$(j .patient_cents)"
req POST /transactions/$JOURNAL/reverse "$T" '{"notes":"again"}'; check "reversing twice -> 409" 409 $CODE
req POST /transactions/$REVERSAL/reverse "$T" '{"notes":"undo the undo"}'; check "reversing a reversal -> 422" 422 $CODE
req GET /transactions/$JOURNAL "$T"; check "original points at its reversal" "$REVERSAL" "$(j .reversed_by_transaction_id)"
CHARGE=$($PSQL -tAc "SELECT id FROM transactions WHERE kind = 'invoice' AND invoice_id = '$INV_M'")
req POST /transactions/$CHARGE/reverse "$T" '{"notes":"no"}'; check "invoices are corrected by credit note, not reversal -> 422" 422 $CODE
req POST /journals "$T" "{\"account_id\":\"$MA\",\"reason_type_id\":\"$WRITE_OFF\",\"notes\":\"x\",\"entries\":[{\"invoice_id\":\"$INV_P\",\"bucket\":\"patient\",\"amount_cents\":-1}]}"
check "journal against another account's invoice -> 404" 404 $CODE

echo "== ageing (backdated activity on the private account)"
D100=$(sa_date '100 days ago'); D95=$(sa_date '95 days ago'); D50=$(sa_date '50 days ago')
req POST /invoices "$T" "$(invoice "$PRIV" "$ZANELE" "$DOC" '[{"tariff_code":"0191","unit_price_cents":30000,"icd10_codes":["J06.9"]}]' "{\"invoice_date\":\"$D100\"}")"
OLD=$(j .id)
req POST /journals "$T" "{\"account_id\":\"$PRIV\",\"reason_type_id\":\"$OTHER_REASON\",\"txn_date\":\"$D95\",\"notes\":\"admin fee\",\"entries\":[{\"bucket\":\"patient\",\"amount_cents\":1500}]}"
OLD_JOURNAL=$(j .id)
req POST /transactions/$OLD_JOURNAL/reverse "$T" '{"notes":"fee waived"}'
req GET "/accounts/$PRIV/statement" "$T"
check "today: old invoice in the 90-day band" 30000 "$(j '.ageing.patient[3]')"
check "today: reversed journal nets out of every band" "0,0,0,30000,0,0,0" "$(j '.ageing.patient | map(tostring) | join(",")')"
check "today: reversed journal hidden" "patient_payment,refund" "$(j '[.unallocated[].kind] | join(",")')"
req GET "/accounts/$PRIV/statement?include_reversed=true" "$T"
check "include_reversed shows journal and reversal" "journal,patient_payment,refund,reversal" "$(j '[.unallocated[].kind] | join(",")')"
check "reversed journal is flagged" true "$(j '.unallocated[] | select(.kind == "journal") | .reversed')"
req GET "/accounts/$PRIV/statement?as_of=$D50" "$T"
check "50 days ago: journal not yet reversed, counts" "0,31500,0,0,0,0,0" "$(j '.ageing.patient | map(tostring) | join(",")')"
check "50 days ago: later invoices not on statement" 1 "$(j '.invoices | length')"
req GET "/accounts/$PRIV/statement?as_of=$(sa_date tomorrow)" "$T"; check "future statement date -> 422" 422 $CODE

echo "== statement (medical aid account)"
req GET "/accounts/$MA/statement" "$T"
check "practice details" "Billing Clinic 1234567" "$(j '"\(.practice.name) \(.practice.pcns_number)"')"
check "practice details not set yet" null "$(j .practice.bank_account_number)"
req PATCH /tenants/current "$T" '{"details":{"address_line1":"1 Main Rd","city":"Durban","postal_code":"4001","phone":"031 555 1234","email":"accounts@clinic.co.za","bank_name":"First Bank","bank_account_name":"Billing Clinic","bank_account_number":"62 1234 5678","bank_branch_code":"250655"}}'
check "practice manager sets contact and bank details" 6212345678 "$(j .details.bank_account_number)"
req PATCH /tenants/current "$T" '{"details":{"bank_branch_code":"25065"}}'
check "bad branch code -> 422" 422 $CODE
req GET "/accounts/$MA/statement" "$T"
check "statement shows banking details" "First Bank 6212345678 250655" "$(j '.practice | "\(.bank_name) \(.bank_account_number) \(.bank_branch_code)"')"
check "statement shows practice contact" "1 Main Rd Durban 0315551234" "$(j '.practice | "\(.address_line1) \(.city) \(.phone)"')"
check "treating doctor with practice and council numbers" "Dr A Owner 1234567 MP0123456" "$(j --arg d "$DOC" '.providers[] | select(.id == $d) | "\(.name) \(.practice_number) \(.hpcsa_number)"')"
check "invoice points at its doctors" "$DOC $REF" "$(j '.invoices[0] | "\(.treating_provider_id) \(.referring_provider_id)"')"
check "assistant doctor listed" MP7777777 "$(j --arg a "$ASSIST" '.providers[] | select(.id == $a) | .hpcsa_number')"
check "addressed to the guarantor" "Bongani Mthembu Durban 4001" "$(j '.addressee | "\(.first_name) \(.last_name) \(.city) \(.postal_code)"')"
check "medical aid and main member" "Billing Health Core BH-100 Bongani Mthembu" "$(j '.medical_aid | "\(.scheme_name) \(.plan_name) \(.membership_number) \(.main_member_name)"')"
check "everyone on the file" "00" "$(j '[.patients[].dependant_code] | join(",")')"
check "both invoices" 2 "$(j '.invoices | length')"

check "lines with codes" "0191 0018 J06.9,Z00.0" "$(j '.invoices[0].lines[0] | "\(.tariff_code) \(.modifiers[0]) \(.icd10_codes | join(","))"')"
check "totals" "0 9000 9000" "$(j '.totals | "\(.medical_aid_due_cents) \(.patient_due_cents) \(.total_due_cents)"')"
check "ageing: R90 current, patient side" "0 9000" "$(j '"\(.ageing.medical_aid[0]) \(.ageing.patient[0])"')"
check "reversed write-off hidden by default" 0 "$(j '[.invoices[0].transactions[] | select(.kind=="journal" or .kind=="reversal")] | length')"
check "invoice transactions in order" "invoice,claim_submitted,remittance,liability_transfer" "$(j '[.invoices[0].transactions[].kind] | join(",")')"
check "remittance line on the medical aid side" -30000 "$(j '.invoices[0].transactions[2].medical_aid_cents')"
req GET "/accounts/$MA/statement?outstanding_only=true" "$T"; check "outstanding_only drops the paid invoice" 1 "$(j '.invoices | length')"
echo "== invoice list and details"
req POST /invoices/search "$T" '{}'
check "practice-wide list, newest first" "$(j '[.items[].invoice_date] | sort | reverse | join(",")')" "$(j '[.items[].invoice_date] | join(",")')"
check "list total" "$(j '.items | length')" "$(j .total)"
req POST /invoices/search "$T" "{\"patient_id\":\"$BONGANI\"}"
check "filter by patient" "Bongani Mthembu 00" "$(j '.items[0].patient | "\(.first_name) \(.last_name) \(.dependant_code)"')"
check "filter by patient: only theirs" 0 "$(j --arg p "$BONGANI" '[.items[] | select(.patient.patient_id != $p)] | length')"
req POST /invoices/search "$T" "{\"provider_id\":\"$REF\"}"; check "filter by treating doctor (none)" 0 "$(j .total)"
req POST /invoices/search "$T" "{\"provider_id\":\"$DOC\",\"page_size\":1}"
check "paged: one item, full total" "1 true" "$(j '"\(.items | length) \(.total > 1)"')"
check "list names the doctor" "Dr A Owner" "$(j '.items[0].treating_provider')"
req POST /invoices/search "$T" '{"status":"settled"}'
check "settled filter: nothing owed" 0 "$(j '[.items[] | select(.medical_aid_due_cents != 0 or .patient_due_cents != 0)] | length')"
req POST /invoices/search "$T" '{"status":"outstanding"}'
check "outstanding filter: something owed" 0 "$(j '[.items[] | select(.medical_aid_due_cents == 0 and .patient_due_cents == 0)] | length')"
req POST /invoices/search "$T" '{"claim_status":"none"}'
check "never-claimed filter" 0 "$(j '[.items[] | select(.claim_status != null)] | length')"
req POST /invoices/search "$T" "{\"from\":\"$TODAY\",\"to\":\"$TODAY\"}"
check "date range" 0 "$(j --arg d "$TODAY" '[.items[] | select(.invoice_date != $d)] | length')"
req POST /invoices/search "$T" '{"from":"2026-02-01","to":"2026-01-01"}'; check "from after to -> 422" 422 $CODE
req POST /invoices/search "$R" '{}'; check "reception can't list invoices -> 403" 403 $CODE
req GET /accounts/$MA/invoices "$T"; check "account list uses the same rows" "$MA" "$(j '.[0].account_id')"
req GET /invoices/$INV_M "$T"
check "invoice shows the patient" "Bongani Mthembu 00" "$(j '.patient | "\(.first_name) \(.last_name) \(.dependant_code)"')"
check "invoice shows medical aid and main member" "Billing Health BH-100 Bongani Mthembu" "$(j '.medical_aid | "\(.scheme_name) \(.membership_number) \(.main_member_name)"')"
check "invoice shows practice and banking" "Billing Clinic 6212345678" "$(j '.practice | "\(.name) \(.bank_account_number)"')"
check "invoice shows its transactions" invoice "$(j '.transactions[0].kind')"
check "invoice transactions include reversed entries, flagged" true "$(j '[.transactions[] | select(.kind == "journal")][0].reversed')"

echo "== database backstop (bypassing the API)"
out=$($PSQL -c "UPDATE transactions SET amount_cents = 1 WHERE id = '$JOURNAL'" 2>&1); contains "posted transactions can't be edited" "immutable" "$out"
out=$($PSQL -c "DELETE FROM transaction_entries WHERE transaction_id = '$JOURNAL'" 2>&1); contains "ledger entries can't be deleted" "immutable" "$out"
out=$($PSQL -c "UPDATE invoices SET total_cents = 1 WHERE id = '$INV_P'" 2>&1); contains "invoices can't be edited" "immutable" "$out"
out=$($PSQL -c "TRUNCATE transaction_entries" 2>&1); contains "ledger can't be truncated" "immutable" "$out"
out=$($PSQL 2>&1 <<SQL
BEGIN;
INSERT INTO transactions (id, tenant_id, account_id, kind, txn_date, amount_cents, invoice_id, notes)
VALUES ('00000000-0000-0000-0000-0000000000f1', '$P', '$MA', 'liability_transfer', current_date, 100, '$INV_M', 'sneaky');
INSERT INTO transaction_entries (tenant_id, transaction_id, account_id, invoice_id, bucket, amount_cents)
VALUES ('$P', '00000000-0000-0000-0000-0000000000f1', '$MA', '$INV_M', 'medical_aid', -100);
COMMIT;
SQL
); contains "one-sided liability transfer rejected at commit" "liability transfer moves one amount" "$out"
out=$($PSQL 2>&1 <<SQL
BEGIN;
INSERT INTO transactions (id, tenant_id, account_id, kind, txn_date, amount_cents, invoice_id, reverses_transaction_id, notes)
VALUES ('00000000-0000-0000-0000-0000000000f2', '$P', '$MA', 'reversal', current_date, 9000, '$INV_M', '$TRANSFER', 'half');
INSERT INTO transaction_entries (tenant_id, transaction_id, account_id, invoice_id, bucket, amount_cents)
VALUES ('$P', '00000000-0000-0000-0000-0000000000f2', '$MA', '$INV_M', 'medical_aid', 9000);
COMMIT;
SQL
); contains "partial reversal rejected at commit" "exactly negate" "$out"
out=$($PSQL 2>&1 <<SQL
BEGIN;
INSERT INTO transactions (id, tenant_id, account_id, kind, txn_date, amount_cents, remittance_id)
VALUES ('00000000-0000-0000-0000-0000000000f3', '$P', '$MA', 'remittance', current_date, 20000, '$RA');
INSERT INTO transaction_entries (tenant_id, transaction_id, account_id, invoice_id, bucket, amount_cents)
VALUES ('$P', '00000000-0000-0000-0000-0000000000f3', '$MA', '$INV_M', 'medical_aid', -20000);
COMMIT;
SQL
); contains "over-allocated remittance rejected at commit" "exceed the remittance total" "$out"
out=$($PSQL -c "UPDATE claims SET status = 'submitted' WHERE id = '$CLAIM'" 2>&1); contains "claim can't reopen" "can't go from paid to submitted" "$out"
out=$($PSQL -c "SET ROLE doctr8_app; SELECT set_config('app.tenant_id', '$P', false); UPDATE transactions SET notes = 'x' WHERE id = '$JOURNAL'" 2>&1)
contains "app role has no UPDATE on the ledger" "permission denied" "$out"

echo "== isolation"
req POST /users "" '{"email":"rival@clinic.co.za","full_name":"Rival","password":"yet another password"}'
req POST /auth/login "" '{"email":"rival@clinic.co.za","password":"yet another password"}' "$J/rival"; X=$(j .access_token)
req POST /tenants "$X" '{"name":"Rival Clinic","slug":"rival-clinic"}'; XP=$(j .id)
req POST /auth/switch-practice "" "{\"tenant_id\":\"$XP\"}" "$J/rival"; X=$(j .access_token)
req GET /invoices/$INV_M "$X"; check "other practice can't read invoice -> 404" 404 $CODE
req GET /accounts/$MA/statement "$X"; check "other practice can't read statement -> 404" 404 $CODE
req GET /accounts/$MA/transactions "$X"; check "other practice can't read ledger -> 404" 404 $CODE
req POST /payments "$X" "{\"account_id\":\"$MA\",\"method\":\"cash\",\"amount_cents\":100}"; check "other practice can't post to account -> 404" 404 $CODE
req GET /remittances/$RA "$X"; check "other practice can't read remittance -> 404" 404 $CODE
req GET /claims "$X"; check "other practice's claims queue is empty" 0 "$(jq length <<<"$BODY")"

echo "== websocket events"
sleep 0.5; kill $PROBE_OWNER $PROBE_REC 2>/dev/null; wait $PROBE_OWNER $PROBE_REC 2>/dev/null
events() { jq -c 'select(.type == "event")' "$1"; }
count() { events "$J/ws_owner" | jq -c "select(.event == \"$1\" $2)" | wc -l | tr -d ' '; }
check "claim status changes streamed in order" "submitted,accepted,paid" \
  "$(events "$J/ws_owner" | jq -r "select(.event == \"claim.status_changed\" and .id == \"$CLAIM\") | .status" | paste -sd,)"
check "auto-paid claim streamed" "submitted,paid" \
  "$(events "$J/ws_owner" | jq -r "select(.event == \"claim.status_changed\" and .id == \"$CLAIM2\") | .status" | paste -sd,)"
check "remittance received once (duplicate rolled back, no event)" 1 "$(count remittance.received)"
check "two successful allocations (failed ones emitted nothing)" 2 "$(count remittance.allocated)"
check "invoice events for the followed account" 2 "$(count invoice.created "and .account_id == \"$MA\"")"
check "balance events for the followed account" 1 "$([[ $(count account.balance_changed "and .account_id == \"$MA\"") -gt 0 ]] && echo 1 || echo 0)"
check "no account events for an account nobody follows" 0 "$(events "$J/ws_owner" | jq -c "select(.account_id == \"$PRIV\" and (.event | startswith(\"claim\") | not))" | wc -l | tr -d ' ')"
check "events carry ids, not names" 0 "$(grep -Ec 'Bongani|Mthembu|Zanele|Khumalo' "$J/ws_owner")"
check "reception (no billing.view) received no billing events" 0 "$(events "$J/ws_rec" | wc -l | tr -d ' ')"

summary
