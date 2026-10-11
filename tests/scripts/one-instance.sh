#!/usr/bin/env bash
# A second ElyDoc is refused; the first keeps its data.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
[ "$(uname)" = Darwin ] || { echo "SKIP: macOS only"; exit 0; }
work=$(mktemp -d)
cleanup() {
  if [ -n "${pid:-}" ]; then
    # Its worker writes into the folder too.
    pkill -9 -P "$pid" 2>/dev/null || true
    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
  fi
  rm -rf "$work"
}
trap cleanup EXIT
fail() { echo "FAIL: $*" >&2; exit 1; }

cargo build -q -p ely-lok --bins -p ely-doc --manifest-path "$root/Cargo.toml"
run() { HOME="$work/home" ELY_LOG=info ELY_LOCALE=en ELY_NO_DIALOG=1 ELY_SMOKE=hold exec "$root/target/debug/ely-doc"; }
run >"$work/first" 2>&1 &
pid=$!
for _ in $(seq 90); do grep -q "lok: first tile drawn" "$work/first" && break; sleep 1; done
grep -q "lok: first tile drawn" "$work/first" || fail "the first copy did not start: $(tail -5 "$work/first")"
data="$work/home/Library/Application Support/ElyOffice/ely-doc"
[ -d "$data/work" ] || fail "no work folder"
touch "$data/work/kept"
status=0
(run) >"$work/second" 2>&1 || status=$?
[ "$status" != 0 ] || fail "the second copy started"
grep -q "already running (process $pid)" "$work/second" || fail "the refusal: $(tail -3 "$work/second")"
[ -e "$data/work/kept" ] || fail "the second copy wiped the first's work folder"
kill -0 "$pid" || fail "the first copy died"
echo "ok: a second ElyDoc is refused and the first keeps its data"
