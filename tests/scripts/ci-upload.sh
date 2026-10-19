#!/usr/bin/env bash
# ci.sh uploads: Linux alone sends source; reruns refuse.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
fail() {
  echo "FAIL: $*" >&2
  exit 1
}
command -v python3 >/dev/null || {
  echo "SKIP: no python3"
  exit 0
}
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
version=$(awk -F'"' '/^version = / { print $2; exit }' "$root/Cargo.toml")
mkdir -p "$work/share/engines" "$work/share/reference" "$work/bin"
export STATE=$work/uploads
: >"$STATE"

# The release API: one release, its assets in $STATE.
cat >"$work/bin/curl" <<'EOF'
#!/usr/bin/env bash
url=${!#}
case $url in
  */releases/tags/*) echo '{"id": 1}' ;;
  */releases/1/assets) python3 -c 'import json, sys; print(json.dumps([{"name": l.split()[0]} for l in open(sys.argv[1]) if l.strip()]))' "$STATE" ;;
  */releases/1/assets\?name=*) echo "${url##*name=} $CI_OS" >>"$STATE" ;;
  *) echo "curl stub: unexpected $url" >&2; exit 22 ;;
esac
EOF
chmod +x "$work/bin/curl"

# A clone writing one package and the source pair.
clone() { # os
  local at=$work/$1 source='d=$1; touch "$d/ElyOffice-V-c0ffee-source.tar.gz" "$d/ElyOffice-V-c0ffee-upstream-source.tar.gz"'
  mkdir -p "$at/scripts" "$at/packaging/macos" "$at/packaging/linux"
  cp "$root/scripts/ci.sh" "$at/scripts/"
  cp "$root/Cargo.toml" "$at/"
  cp "$root/packaging/apps.tsv" "$at/packaging/"
  write() { printf '#!/usr/bin/env bash\n%s\n' "$2" >"$at/$1" && chmod +x "$at/$1"; }
  write packaging/source.sh "$source"
  write packaging/macos/release.sh "mkdir -p dist; touch dist/App-$1.dmg; packaging/source.sh dist"
  write packaging/linux/container.sh 'exec "$@"'
  write packaging/linux/package.sh "mkdir -p dist; touch dist/\$1-$1.AppImage"
}
go() { # os
  CI_OS=$1 PATH=$work/bin:$PATH ELY_SHARE=$work/share ELY_RELEASE_TOKEN=unused \
    ELY_SIGN_IDENTITY=unused ELY_NOTARY_PROFILE=unused \
    "$work/$1/scripts/ci.sh" "$1" "v$version" >"$work/$1.out" 2>&1
}
run() { # os
  clone "$1"
  go "$1" || fail "$1: $(cat "$work/$1.out")"
}
uploads() { # name pattern, os
  awk -v n="$1" -v o="$2" '$1 ~ n && $2 == o' "$STATE" | wc -l
}

# macOS first: it builds the pair and uploads neither.
run macos
[ "$(uploads 'source' macos)" -eq 0 ] || fail "macOS uploaded source: $(cat "$STATE")"
[ "$(uploads 'App-macos.dmg' macos)" -eq 1 ] || fail "macOS skipped its dmg"
run linux
[ "$(uploads '-source.tar.gz' linux)" -eq 2 ] || fail "Linux did not upload the pair: $(cat "$STATE")"
# A rebuild over its own uploads sends nothing and fails.
before=$(cat "$STATE")
rm -rf "$work/linux"
clone linux
if go linux; then fail "the rerun passed: $(cat "$work/linux.out")"; fi
grep -q "already on v$version; delete them, then rerun" "$work/linux.out" || fail "rerun: $(cat "$work/linux.out")"
[ "$(cat "$STATE")" = "$before" ] || fail "the rerun uploaded: $(cat "$STATE")"
# Only the last upload left: the rerun still sends nothing.
last=$(awk '$2 == "linux" { n = $1 } END { print n }' "$STATE")
grep -v ' linux$' "$STATE" >"$work/kept" || true
echo "$last linux" >>"$work/kept"
cp "$work/kept" "$STATE"
before=$(cat "$STATE")
if go linux; then fail "the partial rerun passed: $(cat "$work/linux.out")"; fi
grep -q "delete them, then rerun: $last$" "$work/linux.out" || fail "partial: $(cat "$work/linux.out")"
[ "$(cat "$STATE")" = "$before" ] || fail "the partial rerun uploaded: $(cat "$STATE")"
echo "ci-upload: Linux alone uploads source; reruns refuse"
