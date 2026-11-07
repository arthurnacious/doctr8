#!/usr/bin/env bash
# Access module e2e: auth, practices, roles, members, isolation, token rotation.
. "$(dirname "$0")/lib.sh" "$@"

echo "== register + login"
req POST /users "" '{"email":"Thabo@Example.co.za","full_name":"Thabo M","password":"correct horse battery"}'; check "register thabo" 201 $CODE
req POST /users "" '{"email":"thabo@example.co.za","full_name":"Dup","password":"correct horse battery"}'; check "duplicate email (case-insensitive) -> 409" 409 $CODE
req POST /users "" '{"email":"lerato@example.co.za","full_name":"Lerato K","password":"short"}'; check "short password -> 422" 422 $CODE
req POST /users "" '{"email":"lerato@example.co.za","full_name":"Lerato K","password":"another good password"}'; check "register lerato" 201 $CODE
LERATO_ID=$(jq -r .id <<<"$BODY")
req POST /auth/login "" '{"email":"thabo@example.co.za","password":"wrong password!!"}'; check "wrong password -> 401" 401 $CODE
req POST /auth/login "" '{"email":"nobody@example.co.za","password":"wrong password!!"}'; check "unknown email -> 401 same code" invalid_credentials "$(jq -r .error.code <<<"$BODY")"
req POST /auth/login "" '{"email":"THABO@example.co.za","password":"correct horse battery"}' "$J/thabo"; check "login (mixed-case email)" 200 $CODE
check "no practice yet" null "$(jq -r .practice <<<"$BODY")"
T=$(jq -r .access_token <<<"$BODY")

echo "== practices"
req GET /roles "$T"; check "roles without practice -> 403" 403 $CODE
req GET /roles; check "roles without token -> 401" 401 $CODE
req POST /tenants "$T" '{"name":"Practice A","slug":"Dr-A","pcns_number":"012 3456"}'; check "create practice A" 201 $CODE
A=$(jq -r .id <<<"$BODY"); check "slug normalised" dr-a "$(jq -r .slug <<<"$BODY")"
req POST /tenants "$T" '{"name":"Other","slug":"dr-a"}'; check "duplicate slug -> 409 w/ message" "That practice address is already taken" "$(jq -r .error.message <<<"$BODY")"
req POST /auth/switch-practice "" "{\"tenant_id\":\"$A\"}" "$J/thabo"; check "switch to A" 200 $CODE
check "owner has all 12 perms" 12 "$(jq '.permissions|length' <<<"$BODY")"
T=$(jq -r .access_token <<<"$BODY")
req GET /tenants/current "$T"; check "current practice" "Practice A" "$(jq -r .name <<<"$BODY")"
req PATCH /tenants/current "$T" '{"pcns_number":null}'; check "clear pcns via null" null "$(jq -r .pcns_number <<<"$BODY")"

echo "== roles CRUD"
req GET /permissions "$T"; check "permission catalogue" 12 "$(jq length <<<"$BODY")"
req GET /roles "$T"; OWNER=$(jq -r '.[0].id' <<<"$BODY"); check "Owner role listed" "Owner" "$(jq -r '.[0].name' <<<"$BODY")"
req POST /roles "$T" '{"name":"Reception","permissions":["members.view","roles.assign","roles.view"]}'; check "create Reception" 201 $CODE
REC=$(jq -r .id <<<"$BODY")
req POST /roles "$T" '{"name":"reception"}'; check "duplicate role name (case-insensitive) -> 409" 409 $CODE
req POST /roles "$T" '{"name":"Bad","permissions":["patients.read"]}'; check "unknown permission -> 422" 422 $CODE
req PATCH /roles/$REC "$T" '{"description":"Front desk"}'; check "update description" "Front desk" "$(jq -r .description <<<"$BODY")"
req PUT /roles/$REC/permissions "$T" '{"permissions":["members.view","roles.assign"]}'
check "set permissions -> 2" 2 "$(jq '.permissions|length' <<<"$BODY")"
req PATCH /roles/$OWNER "$T" '{"name":"Boss"}'; check "rename Owner -> 403" 403 $CODE
req PUT /roles/$OWNER/permissions "$T" '{"permissions":[]}'; check "edit Owner perms -> 403" 403 $CODE
req DELETE /roles/$OWNER "$T"; check "delete Owner -> 403" 403 $CODE

echo "== members + escalation guards"
$PSQL -c "INSERT INTO memberships (tenant_id,user_id,status,joined_at) VALUES ('$A','$LERATO_ID','active',now());
          INSERT INTO membership_roles (tenant_id,user_id,role_id) VALUES ('$A','$LERATO_ID','$REC');" >/dev/null
req DELETE /roles/$REC "$T"; check "delete assigned role -> 409" 409 $CODE
req GET /members "$T"; THABO_ID=$(jq -r '.[] | select(.email=="thabo@example.co.za") | .user_id' <<<"$BODY"); check "two members" 2 "$(jq length <<<"$BODY")"
req PUT /members/$THABO_ID/roles "$T" '{"role_ids":[]}'; check "remove last Owner -> 409" 409 $CODE
req POST /auth/login "" "{\"email\":\"lerato@example.co.za\",\"password\":\"another good password\",\"tenant_id\":\"$A\"}" "$J/lerato"
check "lerato login straight into A" 200 $CODE; L=$(jq -r .access_token <<<"$BODY")
check "lerato perms" '["members.view","roles.assign"]' "$(jq -c .permissions <<<"$BODY")"
req PUT /members/$LERATO_ID/roles "$L" "{\"role_ids\":[\"$REC\",\"$OWNER\"]}"; check "lerato self-assign Owner -> 403" 403 $CODE
req PUT /members/$THABO_ID/roles "$L" "{\"role_ids\":[]}"; check "lerato demote Owner -> 403" 403 $CODE
req GET /roles "$L"; check "lerato lacks roles.view -> 403" 403 $CODE
req PUT /members/$LERATO_ID/roles "$T" "{\"role_ids\":[\"$REC\",\"$OWNER\"]}"; check "owner promotes lerato" 2 "$(jq length <<<"$BODY")"
req PUT /members/$THABO_ID/roles "$T" '{"role_ids":[]}'; check "now thabo can step down (another Owner exists)" 200 $CODE
req PUT /members/$THABO_ID/roles "$L" "{\"role_ids\":[\"$OWNER\"]}"; check "restore thabo (lerato token still old perms -> 403)" 403 $CODE
req POST /auth/refresh "" "" "$J/lerato"; L=$(jq -r .access_token <<<"$BODY"); check "lerato refresh picks up Owner perms" 12 "$(jq '.permissions|length' <<<"$BODY")"
req PUT /members/$THABO_ID/roles "$L" "{\"role_ids\":[\"$OWNER\"]}"; check "lerato (now Owner) restores thabo" 200 $CODE

echo "== tenant isolation"
req POST /tenants "$L" '{"name":"Practice B","slug":"dr-b"}'; B_ID=$(jq -r .id <<<"$BODY"); check "lerato creates B" 201 $CODE
req POST /auth/switch-practice "" "{\"tenant_id\":\"$B_ID\"}" "$J/lerato"; LB=$(jq -r .access_token <<<"$BODY"); check "lerato switches to B" 2 "$(jq '.practices|length' <<<"$BODY")"
req GET /roles/$REC "$LB"; check "A's role invisible from B -> 404" 404 $CODE
req GET /roles "$LB"; check "B sees only its own Owner" 1 "$(jq length <<<"$BODY")"
req POST /auth/switch-practice "" "{\"tenant_id\":\"$B_ID\"}" "$J/thabo"; check "thabo cannot switch into B -> 403" 403 $CODE

echo "== refresh rotation + reuse detection + logout"
cp $J/thabo $J/thabo-stale   # note: the failed switch above did not rotate (tx rolled back)
req POST /auth/refresh "" "" "$J/thabo"; check "refresh rotates" 200 $CODE
check "refresh keeps practice A" "$A" "$(jq -r .practice.id <<<"$BODY")"
req POST /auth/refresh "" "" "$J/thabo-stale"; check "replay old token -> 401" 401 $CODE
req POST /auth/refresh "" "" "$J/thabo"; check "whole session revoked after reuse -> 401" 401 $CODE
check "revoked reason recorded" reuse_detected "$($PSQL -tAc "SELECT revoked_reason FROM auth_sessions s JOIN users u ON u.id=s.user_id WHERE u.email='thabo@example.co.za' AND revoked_at IS NOT NULL LIMIT 1")"
req POST /auth/logout "" "" "$J/lerato"; check "logout -> 204" 204 $CODE
req POST /auth/refresh "" "" "$J/lerato"; check "refresh after logout -> 401" 401 $CODE
req POST /auth/logout "" "" "$J/lerato"; check "logout idempotent" 204 $CODE

summary
