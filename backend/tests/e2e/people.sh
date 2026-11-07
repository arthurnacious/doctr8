#!/usr/bin/env bash
# People module e2e: persons, medical aid, accounts, patients, file modes,
# database trigger backstop, search and tenant isolation.
. "$(dirname "$0")/lib.sh" "$@"

j() { jq -r "$1" <<<"$BODY"; }

echo "== setup"
req POST /users "" '{"email":"owner@clinic.co.za","full_name":"Dr Owner","password":"a very good password"}'
req POST /auth/login "" '{"email":"owner@clinic.co.za","password":"a very good password"}' "$J/owner"
T=$(j .access_token)
req POST /tenants "$T" '{"name":"Clinic P","slug":"clinic-p"}'; P=$(j .id)
check "new practice defaults to patient_per_file" patient_per_file "$(j .file_mode)"
req POST /auth/switch-practice "" "{\"tenant_id\":\"$P\"}" "$J/owner"; T=$(j .access_token)
check "owner has patient + account permissions" 4 "$(jq '[.permissions[] | select(startswith("patients.") or startswith("accounts."))] | length' <<<"$BODY")"

# Global reference data is maintained by doctr8, not practices: seed as superuser.
read -r SCHEME PLAN OTHER_PLAN < <($PSQL -tA -F' ' -c "
  WITH s AS (INSERT INTO medical_schemes (name, code) VALUES ('Test Health', 'TH') RETURNING id),
       o AS (INSERT INTO medical_schemes (name) VALUES ('Other Med') RETURNING id),
       p AS (INSERT INTO medical_scheme_plans (scheme_id, name) SELECT id, 'Classic' FROM s RETURNING id),
       q AS (INSERT INTO medical_scheme_plans (scheme_id, name) SELECT id, 'Basic' FROM o RETURNING id)
  SELECT (SELECT id FROM s), (SELECT id FROM p), (SELECT id FROM q)")
req GET /medical-schemes "$T"; check "scheme catalogue" 2 "$(jq length <<<"$BODY")"
check "plans nested under scheme" Classic "$(jq -r '.[] | select(.name=="Test Health") | .plans[0].name' <<<"$BODY")"

echo "== patient per file"
req POST /patient-files "$T" '{"patients":[{"person":{"new":{"first_name":"Thandi","last_name":"Nkosi","id_number":"900215 0123 088","mobile":"082 555 1234"}}}],"guarantor":{"patient":0}}'
check "private file opened" 201 $CODE
ACC1=$(j .id); PERSON1=$(j '.patients[0].person_id')
check "account number 1" 1 "$(j .account_no)"
check "account stamped patient_per_file" patient_per_file "$(j .file_mode)"
check "self-paying patient is 00" 00 "$(j '.patients[0].dependant_code')"
check "private account has no medical aid" null "$(j .medical_aid)"
req GET /persons/$PERSON1 "$T"
check "DOB derived from SA ID" 1990-02-15 "$(j .date_of_birth)"
check "ID number normalised" 9002150123088 "$(j .id_number)"
check "mobile normalised" 0825551234 "$(j .mobile)"
req POST /persons "$T" '{"first_name":"Dup","last_name":"Licate","id_number":"9002150123088"}'
check "same ID number twice -> 409" 409 $CODE
req POST /persons "$T" '{"first_name":"Bad","last_name":"Id","id_number":"9002150123089"}'
check "bad SA ID check digit -> 422" 422 $CODE
contains "error doesn't echo the ID number" "check digit" "$(j .error.message)"
req POST /persons "$T" '{"first_name":"Bad","last_name":"Dob","id_number":"9002150123088","date_of_birth":"1991-01-01"}'
check "DOB contradicting SA ID -> 422" 422 $CODE

req POST /patient-files "$T" '{"patients":[{"person":{"new":{"first_name":"Lwazi","last_name":"Nkosi","date_of_birth":"2015-06-01"}}}],"guarantor":{"existing":"'$PERSON1'"}}'
check "child on private file, parent pays" 201 $CODE
check "non-principal on private account gets 01" 01 "$(j '.patients[0].dependant_code')"
check "guarantor is the parent" "$PERSON1" "$(j .guarantor.id)"

req POST /patient-files "$T" '{"patients":[{"person":{"new":{"first_name":"A","last_name":"One"}}},{"person":{"new":{"first_name":"B","last_name":"Two"}}}],"guarantor":{"patient":0}}'
check "two patients in patient_per_file -> 409" 409 $CODE
req POST /persons "$T" '{"first_name":"Extra","last_name":"Person"}'; EXTRA=$(j .id)
req POST /accounts/$ACC1/patients "$T" '{"person_id":"'$EXTRA'"}'
check "adding a second patient to a per-file account -> 409" 409 $CODE

echo "== medical aid"
req POST /patient-files "$T" '{"patients":[{"person":{"new":{"first_name":"Sipho","last_name":"Dlamini","id_number":"8507045000086"}}}],"guarantor":{"patient":0},"medical_aid":{"new":{"scheme_id":"'$SCHEME'","plan_id":"'$PLAN'","membership_number":" ab 123 ","main_member":{"patient":0}}}}'
check "medical aid file opened" 201 $CODE
ACC_MA=$(j .id); SIPHO=$(j '.patients[0].person_id'); MEMBERSHIP=$(j .medical_aid.membership_id)
check "membership number normalised" AB123 "$(j .medical_aid.membership_number)"
check "main member is 00" 00 "$(j '.patients[0].dependant_code')"
req POST /patient-files "$T" '{"patients":[{"person":{"new":{"first_name":"Kid","last_name":"Dlamini"}}}],"guarantor":{"existing":"'$SIPHO'"},"medical_aid":{"existing":"'$MEMBERSHIP'"}}'
check "medical aid dependant without code -> 422" 422 $CODE
contains "asks for the code on the card" "membership card" "$(j .error.message)"
req POST /patient-files "$T" '{"patients":[{"person":{"new":{"first_name":"Kid","last_name":"Dlamini"}},"dependant_code":"00"}],"guarantor":{"existing":"'$SIPHO'"},"medical_aid":{"existing":"'$MEMBERSHIP'"}}'
check "00 for a non-main-member -> 422" 422 $CODE
req POST /patient-files "$T" '{"patients":[{"person":{"new":{"first_name":"Kid","last_name":"Dlamini"}},"dependant_code":"02"}],"guarantor":{"existing":"'$SIPHO'"},"medical_aid":{"existing":"'$MEMBERSHIP'"}}'
check "dependant gets own per-file account on parent's medical aid" 201 $CODE
req POST /patient-files "$T" '{"patients":[{"person":{"new":{"first_name":"X","last_name":"Y"}}}],"guarantor":{"patient":0},"medical_aid":{"new":{"scheme_id":"'$SCHEME'","plan_id":"'$OTHER_PLAN'","membership_number":"Z1","main_member":"guarantor"}}}'
check "plan from another scheme -> 422" 422 $CODE
req POST /patient-files "$T" '{"patients":[{"person":{"new":{"first_name":"X","last_name":"Y"}}}],"guarantor":{"patient":0},"medical_aid":{"new":{"scheme_id":"'$SCHEME'","plan_id":"'$PLAN'","membership_number":"AB 123","main_member":"guarantor"}}}'
check "same membership number twice -> 409" 409 $CODE
req POST /patient-files "$T" '{"patients":[{"person":{"new":{"first_name":"X","last_name":"Y"}}}],"guarantor":{"patient":3}}'
check "guarantor index out of range -> 422" 422 $CODE
NPERSONS_BEFORE=$($PSQL -tAc "SELECT count(*) FROM persons")
req POST /patient-files "$T" '{"patients":[{"person":{"new":{"first_name":"Rolled","last_name":"Back"}}},{"person":{"new":{"first_name":"Also","last_name":"Gone"}}}],"guarantor":{"patient":0}}'
check "failed file creates nothing (transaction rolled back)" "$NPERSONS_BEFORE" "$($PSQL -tAc "SELECT count(*) FROM persons")"

echo "== family per file"
req PATCH /tenants/current "$T" '{"file_mode":"family_per_file"}'
check "practice switches to family_per_file" family_per_file "$(j .file_mode)"
req POST /patient-files "$T" '{"patients":[{"person":{"new":{"first_name":"Naledi","last_name":"Mokoena","id_number":"8001015009087"}}},{"person":{"new":{"first_name":"Tumi","last_name":"Mokoena","id_number":"1203105000086"}},"dependant_code":"01"}],"guarantor":{"patient":0},"medical_aid":{"new":{"scheme_id":"'$SCHEME'","plan_id":"'$PLAN'","membership_number":"FAM-1","main_member":{"patient":0}}}}'
check "family file with main member + dependant" 201 $CODE
ACC_FAM=$(j .id)
check "family file has 2 patients" "00,01" "$(jq -r '[.patients[].dependant_code] | join(",")' <<<"$BODY")"
check "account stamped family_per_file" family_per_file "$(j .file_mode)"
req POST /patient-files "$T" '{"patients":[{"person":{"new":{"first_name":"Only","last_name":"Child"}},"dependant_code":"01"}],"guarantor":{"new":{"first_name":"Absent","last_name":"Parent"}}}'
check "family file without a 00 patient -> 422" 422 $CODE
req POST /accounts/$ACC_FAM/patients "$T" '{"person_id":"'$EXTRA'","dependant_code":"02"}'
check "add dependant to family file" 3 "$(j '.patients | length')"
req POST /accounts/$ACC_FAM/patients "$T" '{"person_id":"'$PERSON1'","dependant_code":"02"}'
check "duplicate dependant code -> 422" 422 $CODE
req POST /accounts/$ACC1/patients "$T" '{"person_id":"'$EXTRA'"}'
check "old per-file account keeps its rules after mode change -> 409" 409 $CODE
req GET /accounts/$ACC1 "$T"; check "old account still patient_per_file" patient_per_file "$(j .file_mode)"
req GET /accounts/$ACC_FAM "$T"; FAM_KID=$(jq -r '.patients[] | select(.dependant_code=="01") | .patient_id' <<<"$BODY")
req PATCH /patients/$FAM_KID "$T" '{"dependant_code":"00"}'
check "moving a dependant to 00 -> 422" 422 $CODE
req PATCH /patients/$FAM_KID "$T" '{"dependant_code":"05"}'
check "renumber dependant" 05 "$(j .dependant_code)"

echo "== new debtor per visit"
req PATCH /tenants/current "$T" '{"file_mode":"debtor_per_visit"}'
req POST /patient-files "$T" '{"patients":[{"person":{"existing":"'$PERSON1'"}}],"guarantor":{"patient":0}}'
check "returning patient gets a new account for the visit" 201 $CODE
ACC_VISIT=$(j .id)
check "visit account stamped debtor_per_visit" debtor_per_visit "$(j .file_mode)"
req GET /persons/$PERSON1/files "$T"
check "person now has 2 files (+1 as guarantor-only not counted)" 2 "$(jq length <<<"$BODY")"
req POST /accounts/$ACC_VISIT/patients "$T" '{"person_id":"'$EXTRA'"}'
check "can't add patients to a per-visit account -> 409" 409 $CODE
contains "explains why" "new account for each visit" "$(j .error.message)"

echo "== account changes"
req PATCH /accounts/$ACC_MA "$T" '{"membership_id":null}'
check "dropping medical aid keeps 00 (main member is also guarantor)" 200 $CODE
check "account now private" null "$(j .medical_aid)"
req PATCH /accounts/$ACC_FAM "$T" '{"guarantor_person_id":"'$EXTRA'","membership_id":null}'
check "making family file private with a different guarantor breaks 00 -> 422" 422 $CODE
req PATCH /accounts/$ACC1 "$T" '{"status":"closed"}'; check "close account" closed "$(j .status)"

echo "== database backstop (bypassing the API)"
out=$($PSQL -c "INSERT INTO patients (tenant_id, person_id, account_id, dependant_code) VALUES ('$P', '$EXTRA', '$ACC1', '07')" 2>&1)
contains "trigger: 2nd patient on per-file account" "exactly one patient" "$out"
out=$($PSQL -c "UPDATE patients SET dependant_code = '09' WHERE account_id = '$ACC_FAM' AND dependant_code = '00'" 2>&1)
contains "trigger: principal must stay 00" "principal member must have dependant code 00" "$out"
out=$($PSQL -c "INSERT INTO patients (tenant_id, person_id, account_id, dependant_code) VALUES ('$P', '$SIPHO', '$ACC_FAM', '00')" 2>&1)
contains "unique: second 00 on family file" "patients_dependant_code_uq" "$out"
out=$($PSQL -c "UPDATE accounts SET file_mode = 'family_per_file' WHERE id = '$ACC1'" 2>&1)
contains "trigger: file mode immutable" "file mode cannot be changed" "$out"
out=$($PSQL -c "BEGIN; INSERT INTO account_counters VALUES ('$P', 999) ON CONFLICT (tenant_id) DO UPDATE SET last_account_no = 999;
  INSERT INTO accounts (tenant_id, account_no, file_mode, guarantor_person_id) VALUES ('$P', 999, 'x', '$PERSON1'); COMMIT;" 2>&1)
contains "trigger: account with no patients can't commit" "exactly one patient" "$out"
out=$($PSQL -c "SET ROLE doctr8_app; SELECT set_config('app.tenant_id', '$P', false); UPDATE accounts SET account_no = 5000 WHERE id = '$ACC1'" 2>&1)
contains "grant: app can't rewrite account numbers" "permission denied" "$out"
out=$($PSQL -c "SET ROLE doctr8_app; DELETE FROM persons WHERE id = '$PERSON1'" 2>&1)
contains "grant: app can't delete persons" "permission denied" "$out"

echo "== search"
req POST /persons/search "$T" '{"query":"nkosi"}'; check "surname search" 2 "$(jq length <<<"$BODY")"
req POST /persons/search "$T" '{"query":"Nkosi Th"}'; check "surname + first name" Thandi "$(jq -r '.[0].first_name' <<<"$BODY")"
req POST /persons/search "$T" '{"query":"thandi nkosi"}'; check "first name + surname" 1 "$(jq length <<<"$BODY")"
req POST /persons/search "$T" '{"query":"900215 0123 088"}'; check "search by SA ID" "$PERSON1" "$(jq -r '.[0].id' <<<"$BODY")"
req POST /persons/search "$T" '{"query":"01"}'; check "search by account number" "$PERSON1" "$(jq -r '.[0].id' <<<"$BODY")"
check "search results omit ID numbers" null "$(jq -r '.[0].id_number' <<<"$BODY")"
req POST /persons/search "$T" '{"query":"%_"}'; check "LIKE wildcards are literal" 0 "$(jq length <<<"$BODY")"

echo "== patient file events (WebSocket)"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
"$ROOT/target/debug/examples/ws_probe" "${B/http:/ws:}/ws" "$T" > "$J/ws_people" 2>/dev/null & PROBE=$!
for _ in $(seq 50); do [[ -s $J/ws_people ]] && break; sleep 0.1; done
req GET /persons/$PERSON1 "$T"
req PUT /persons/$PERSON1 "$T" "$(jq -c '{first_name, last_name, id_number, mobile: "0825550000"}' <<<"$BODY")"
check "person edited" 0825550000 "$(j .mobile)"
req GET /accounts/$ACC_FAM "$T"; FAM_PATIENT=$(j '.patients[] | select(.dependant_code=="05") | .patient_id')
req PATCH /patients/$FAM_PATIENT "$T" '{"dependant_code":"06"}'; check "dependant renumbered" 06 "$(j .dependant_code)"
req POST /patient-files "$T" '{"patients":[{"person":{"new":{"first_name":"Event","last_name":"Probe"}}}],"guarantor":{"patient":0}}'; EV_ACC=$(j .id)
req PATCH /patients/$FAM_PATIENT "$T" '{"dependant_code":"00"}'; check "rejected edit (rolled back)" 422 $CODE
sleep 0.5; kill $PROBE 2>/dev/null; wait $PROBE 2>/dev/null
pf() { jq -r 'select(.type=="event" and .event=="patient_file.changed") | .account_id' "$J/ws_people"; }
check "person edit announces every file they're on as a patient" 2 "$(pf | grep -cE "^($ACC1|$ACC_VISIT)$")"
GUARANTEED=$($PSQL -tAc "SELECT a.id FROM accounts a JOIN patients pt ON pt.account_id = a.id JOIN persons p ON p.id = pt.person_id WHERE p.first_name = 'Lwazi'")
check "...and the file they pay for as guarantor" 1 "$(pf | grep -c "^$GUARANTEED$")"
check "renumbering announces the family file" 1 "$(pf | grep -c "^$ACC_FAM$")"
check "new file announced" 1 "$(pf | grep -c "^$EV_ACC$")"
check "rejected edits announce nothing (3 + 1 + 1 events in total)" 5 "$(pf | wc -l | tr -d ' ')"
check "events carry ids, not names" 0 "$(grep -cE 'Thandi|Nkosi|Probe' "$J/ws_people")"

echo "== patient list (POST /patients/search)"
req POST /patients/search "$T" '{"query":"nkosi"}'
check "name search finds every Nkosi patient row" 3 "$(j .total)"
check "ID numbers masked in the list" "900215******8" "$(jq -r '[.items[] | select(.first_name=="Thandi")][0].id_number_masked' <<<"$BODY")"
check "list never carries a full ID number" 0 "$(grep -c 9002150123088 <<<"$BODY")"
req POST /patients/search "$T" '{"query":"nkosi","page":2,"page_size":1}'
check "paged on the server" "1 3 2" "$(j '"\(.items | length) \(.total) \(.page)"')"
req POST /patients/search "$T" '{"query":"FAM-1"}'
check "membership number search" 3 "$(j .total)"
check "medical aid, plan and main member in the row" "Test Health Classic Naledi Mokoena" \
  "$(j '.items[0] | "\(.scheme_name) \(.plan_name) \(.main_member.first_name) \(.main_member.last_name)"')"
req POST /persons/search "$T" '{"query":"fam-1"}'; check "person search also matches membership numbers" 3 "$(jq length <<<"$BODY")"
req POST /patients/search "$T" '{"query":"0001"}'; check "file number search" "1" "$(j '.items[0].account_no')"
LWAZI=$($PSQL -tAc "SELECT pt.id FROM patients pt JOIN persons p ON p.id = pt.person_id WHERE p.first_name = 'Lwazi'")
req PUT /patients/$LWAZI/status "$T" '{"status":"inactive"}'; check "patient marked inactive" inactive "$(j .status)"
req POST /patients/search "$T" '{"query":"nkosi"}'; check "inactive hidden by default" 2 "$(j .total)"
req POST /patients/search "$T" '{"query":"nkosi","status":"inactive"}'; check "inactive filter" Lwazi "$(j '.items[0].first_name')"
req POST /patients/search "$T" '{"query":"nkosi","status":"all"}'; check "all filter" 3 "$(j .total)"
req GET /accounts/$ACC_FAM "$T"; check "file view shows each patient's status" active "$(j '.patients[0].status')"
req PUT /patients/$LWAZI/status "$T" '{"status":"active"}'; check "reactivated" active "$(j .status)"
req POST /patients/search "$T" '{}'; check "empty query lists everyone, paged" 25 "$(j .page_size)"

echo "== isolation"
req POST /users "" '{"email":"other@clinic.co.za","full_name":"Dr Other","password":"another good password"}'
req POST /auth/login "" '{"email":"other@clinic.co.za","password":"another good password"}' "$J/other"; O=$(j .access_token)
req POST /tenants "$O" '{"name":"Clinic Q","slug":"clinic-q"}'; Q=$(j .id)
req POST /auth/switch-practice "" "{\"tenant_id\":\"$Q\"}" "$J/other"; O=$(j .access_token)
req GET /persons/$PERSON1 "$O"; check "other practice can't read person -> 404" 404 $CODE
req GET /accounts/$ACC1 "$O"; check "other practice can't read account -> 404" 404 $CODE
req POST /persons/search "$O" '{"query":"nkosi"}'; check "other practice search finds nothing" 0 "$(jq length <<<"$BODY")"
req POST /patients/search "$O" '{"query":"nkosi"}'; check "other practice's patient list is separate" 0 "$(j .total)"
req POST /accounts/$ACC_FAM/patients "$O" '{"person_id":"'$EXTRA'","dependant_code":"08"}'
check "other practice can't add to account -> 404" 404 $CODE
req POST /patient-files "$O" '{"patients":[{"person":{"existing":"'$PERSON1'"}}],"guarantor":{"patient":0}}'
check "other practice can't open a file for our person -> 404" 404 $CODE
req POST /patient-files "$O" '{"patients":[{"person":{"new":{"first_name":"Q","last_name":"Patient"}}}],"guarantor":{"patient":0}}'
check "account numbers are per practice" 1 "$(j .account_no)"

summary
