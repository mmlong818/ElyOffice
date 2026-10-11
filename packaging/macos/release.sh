#!/usr/bin/env bash
# Usage: release.sh [batch]; a dmg per app of the batch.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
batch=${1:-1}
dist=${ELY_DIST:-$root/dist}
version=$(awk -F'"' '/^version = / { print $2; exit }' "$root/Cargo.toml")
arch=$(uname -m)
releases=https://git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice/releases
[ -z "${ELY_NOTARY_PROFILE:-}" ] || [ -n "${ELY_SIGN_IDENTITY:-}" ] || {
  echo "release.sh: notarizing needs ELY_SIGN_IDENTITY, a Developer ID" >&2
  exit 1
}
"$root/packaging/source.sh" "$dist"
awk -F'\t' -v batch="$batch" '$4 == batch { print $1 "\t" $2 }' "$root/packaging/apps.tsv" |
  while IFS=$'\t' read -r slug name; do
    "$root/packaging/macos/bundle.sh" "$slug" release
    codesign --verify --deep --strict "$dist/$name.app"
    dmg=$dist/$name-$version-$arch.dmg
    rm -f "$dmg"
    # The app beside Applications: drag one onto the other.
    stage=$(mktemp -d)
    ditto "$dist/$name.app" "$stage/$name.app"
    ln -s /Applications "$stage/Applications"
    hdiutil create -quiet -volname "$name" -srcfolder "$stage" -format UDZO "$dmg"
    rm -rf "$stage"
    if [ -n "${ELY_NOTARY_PROFILE:-}" ]; then
      codesign --force --timestamp --sign "$ELY_SIGN_IDENTITY" "$dmg"
      xcrun notarytool submit "$dmg" --keychain-profile "$ELY_NOTARY_PROFILE" --wait
      xcrun stapler staple "$dmg"
    else
      echo "release.sh: $dmg is not notarized (no ELY_NOTARY_PROFILE)" >&2
    fi
    echo "$dmg"
    # What the app's update check reads from the latest release.
    sha=$(shasum -a 256 "$dmg" | cut -d' ' -f1)
    url="$releases/download/v$version/$(basename "$dmg")"
    printf '{"version": "%s", "assets": {"macos-%s": {"url": "%s", "sha256": "%s"}}}\n' \
      "$version" "${arch/arm64/aarch64}" "$url" "$sha" >"$dist/$slug.json"
    echo "$dist/$slug.json"
  done
rm -rf "$root/target/release/incremental"
