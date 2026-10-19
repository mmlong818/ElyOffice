#!/usr/bin/env bash
# dock-macos.sh: the LOK worker never registers as a Dock app.
set -euo pipefail
[ "$(uname)" = Darwin ] || { echo "SKIP: macOS only"; exit 0; }
runs=${ELY_DOCK_RUNS:-5}
apps=("$@")
# Bundled apps from dist and one unbundled dev build.
[ ${#apps[@]} -gt 0 ] || apps=(dist/ElyDoc.app dist/ElyTable.app dist/ElyShow.app target/debug/ely-doc)

# One launch: samples the worker's type until a tile shows.
launch() {
  local bin=$1 log=$2 start pid worker="" types="" asn i=0
  start=$(( $( [ -f "$log" ] && wc -l < "$log" || echo 0 ) + 1 ))
  ELY_NO_DIALOG=1 ELY_SMOKE=hold ELY_LOG=debug "$bin" >/dev/null 2>&1 &
  pid=$!
  local end=$((SECONDS + 60)) shown=0
  while [ $SECONDS -lt $end ] && kill -0 "$pid" 2>/dev/null; do
    [ -n "$worker" ] || worker=$(pgrep -P "$pid" -x ely-lok-worker || true)
    if [ -n "$worker" ]; then
      asn=$(lsappinfo find "pid=$worker" || true)
      [ -z "$asn" ] || types+=" $(lsappinfo info -only applicationtype "$asn" | grep -o 'type="[A-Za-z]*"' || true)"
    fi
    i=$((i + 1))
    if [ $((i % 20)) -eq 0 ] && tail -n +"$start" "$log" 2>/dev/null | grep -q 'lok: tile ('; then shown=1; break; fi
  done
  kill "$pid" 2>/dev/null || true
  wait "$pid" 2>/dev/null || true
  local seen
  seen=$(tr ' ' '\n' <<<"$types" | grep . | sort | uniq -c | tr -s ' \n' ' ')
  echo "$(basename "$bin"): worker $worker:$seen"
  [ "$shown" = 1 ] || { echo "FAIL: no tile shown"; return 1; }
  [ -n "$worker" ] || { echo "FAIL: no worker started"; return 1; }
  case $types in *Foreground*) echo "FAIL: the worker was a Dock app"; return 1 ;; esac
  case $types in *UIElement*) ;; *) echo "FAIL: the worker's type was never seen"; return 1 ;; esac
  tail -n +"$start" "$log" | grep -q 'lok: the worker is an accessory app' || { echo "FAIL: no accessory log"; return 1; }
}

for app in "${apps[@]}"; do
  if [ -f "$app" ]; then
    bin=$app
    [ -x "$(dirname "$app")/ely-lok-worker" ] || { echo "FAIL: no worker beside $app (cargo build -p ely-lok --bins)"; exit 1; }
  else
    [ -d "$app" ] || { echo "FAIL: $app is missing"; exit 1; }
    bin=$(find "$app/Contents/MacOS" -maxdepth 1 -type f -name 'ely-*' ! -name 'ely-lok-worker' | head -1)
    [ -n "$bin" ] || { echo "FAIL: $app has no app binary"; exit 1; }
  fi
  log="$HOME/Library/Application Support/ElyOffice/$(basename "$bin")/ely.log"
  for _ in $(seq 1 "$runs"); do launch "$bin" "$log"; done
done
echo "PASS: no LOK worker registered as a Dock app"
