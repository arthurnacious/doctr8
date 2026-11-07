# Shared helpers for e2e suites. Source with: . "$(dirname "$0")/lib.sh" "$@"
B=$1; PSQL=$2; J=$(mktemp -d)
trap 'rm -rf "$J"' EXIT
pass=0; fail=0

check() { # name expected actual
  if [[ "$2" == "$3" ]]; then echo "  PASS $1"; ((pass++)); else echo "  FAIL $1: expected [$2] got [$3]"; ((fail++)); fi; }

contains() { # name needle haystack
  if [[ "$3" == *"$2"* ]]; then echo "  PASS $1"; ((pass++)); else echo "  FAIL $1: expected to contain [$2] got [$3]"; ((fail++)); fi; }

req() { # method path [token] [json] [cookiejar] -> sets CODE, BODY
  local m=$1 p=$2 t=$3 d=$4 c=$5
  local args=(-s -o "$J/body" -w '%{http_code}' -X "$m" "$B$p" -H 'content-type: application/json')
  [[ -n $t ]] && args+=(-H "authorization: Bearer $t")
  [[ -n $d ]] && args+=(-d "$d")
  [[ -n $c ]] && args+=(-b "$c" -c "$c")
  CODE=$(curl "${args[@]}"); BODY=$(cat "$J/body"); }

summary() { echo; echo "RESULT: $pass passed, $fail failed"; }
