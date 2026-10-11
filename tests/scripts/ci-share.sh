#!/usr/bin/env bash
# ci.sh uses shared engines only through links to them.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
fail() {
  echo "FAIL: $*" >&2
  exit 1
}
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
share=$work/share
mkdir -p "$share/engines" "$share/reference" "$share/engines"$'\n' "$share/reference"$'\n' "$work/elsewhere"
version=$(awk -F'"' '/^version = / { print $2; exit }' "$root/Cargo.toml")
# A macOS clone whose packaging stub stops before any upload.
attempt() { # name, setup; sets status, writes the output
  local clone=$work/$1
  mkdir -p "$clone/scripts" "$clone/packaging/macos"
  cp "$root/scripts/ci.sh" "$clone/scripts/"
  printf '#!/usr/bin/env bash\necho "packaging reached"\nexit 1\n' >"$clone/packaging/macos/release.sh"
  chmod +x "$clone/packaging/macos/release.sh"
  cp "$root/Cargo.toml" "$clone/"
  cp "$root/packaging/apps.tsv" "$clone/packaging/"
  (cd "$clone" && eval "$2")
  status=0
  ELY_SHARE=$share ELY_RELEASE_TOKEN=unused "$clone/scripts/ci.sh" macos "v$version" >"$work/$1.out" 2>&1 ||
    status=$?
}
# The target as stored plus a newline and x.
target() { readlink "$1" && printf x; }
passes() { # name, setup; reaches the release step
  attempt "$1" "$2"
  grep -q 'packaging reached' "$work/$1.out" || fail "$1 was refused: $(cat "$work/$1.out")"
  for part in engines reference; do
    [ "$(target "$work/$1/$part")" = "$share/$part"$'\n'x ] || fail "$1 has no $part link to $share/$part"
  done
}
refused() { # name, setup; stopped before the release step
  attempt "$1" "$2"
  [ "$status" -ne 0 ] || fail "$1 exited 0"
  grep -q 'is not a link to' "$work/$1.out" || fail "$1 was let through: $(cat "$work/$1.out")"
  ! grep -q 'packaging reached' "$work/$1.out" || fail "$1 reached the release step"
}
passes absent ':'
passes linked 'ln -s "$share/engines" engines && ln -s "$share/reference" reference'
for part in engines reference; do
  refused "built-$part" "mkdir $part"
  refused "elsewhere-$part" "ln -s \"\$work/elsewhere\" $part"
  refused "newline-$part" "ln -s \"\$share/$part\"\$'\\n' $part"
  refused "dangling-$part" "ln -s \"\$work/gone\" $part"
done
echo "ci-share: links pass; own builds, other and newline targets refused"
