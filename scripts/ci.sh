#!/usr/bin/env bash
# A platform's release. Usage: ci.sh <macos|windows|linux> <tag>
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
os=${1:?macos, windows or linux}
tag=${2:?the release tag, such as v0.1.3}
version=$(awk -F'"' '/^version = / { print $2; exit }' "$root/Cargo.toml")
[ "$tag" = "v$version" ] || {
  echo "ci.sh: tag $tag is not v$version" >&2
  exit 1
}
: "${ELY_RELEASE_TOKEN:?the Gitea token that uploads}"
[ -z "${ELY_PATH:-}" ] || PATH=$ELY_PATH:$PATH
cd "$root"
# Linux builds and packages in the Ubuntu 26.04 container.
inside=()
[ "$os" != linux ] || inside=(packaging/linux/container.sh)

# Engines another clone on this host built; else built here.
if [ -n "${ELY_SHARE:-}" ]; then
  for part in engines reference; do
    # Never mix this clone's own build with shared engines.
    if [ "$os" != windows ] && { [ -e "$part" ] || [ -L "$part" ]; }; then
      # A sentinel keeps a trailing newline in the target.
      target=$( (readlink "$part" || true) && printf x)
      target=${target%x}
      [ "$target" = "$ELY_SHARE/$part"$'\n' ] || {
        echo "ci.sh: $root/$part is not a link to $ELY_SHARE/$part; remove it" >&2
        exit 1
      }
    fi
    if [ ! -e "$part" ]; then
      case $os in
        windows) powershell -NoProfile -Command "New-Item -ItemType Junction -Path '$part' -Target '$(cygpath -w "$ELY_SHARE/$part")' | Out-Null" ;;
        *) ln -s "$ELY_SHARE/$part" "$part" ;;
      esac
    fi
  done
else
  scripts/fetch-references.sh
  ${inside[@]+"${inside[@]}"} scripts/build-mupdf.sh
  ${inside[@]+"${inside[@]}"} scripts/fetch-cef.sh
  ${inside[@]+"${inside[@]}"} scripts/build-git.sh
  ${inside[@]+"${inside[@]}"} scripts/fetch-git-lfs.sh
  ${inside[@]+"${inside[@]}"} scripts/fetch-tessdata.sh
  ${inside[@]+"${inside[@]}"} scripts/build-mark-content.sh
  ${inside[@]+"${inside[@]}"} scripts/build-libreoffice.sh
fi

dist=$root/dist
rm -rf "$dist"
batch=$(awk -F'\t' '$4 == 1 { print $1 }' packaging/apps.tsv)
case $os in
  macos) packaging/macos/release.sh 1 ;;
  windows) for slug in $batch; do packaging/windows/installer.sh "$slug"; done ;;
  linux) for slug in $batch; do ${inside[@]+"${inside[@]}"} packaging/linux/package.sh "$slug"; done ;;
  *)
    echo "ci.sh: no release for $os" >&2
    exit 1
    ;;
esac

# macOS's release.sh writes its own source archives.
[ "$os" = macos ] || packaging/source.sh "$dist"

# The tag's release, made by whichever platform comes first.
api=https://git.zacharyzhang.com/api/v1/repos/ZacharyZhang-NY/ElyOffice
auth="Authorization: token $ELY_RELEASE_TOKEN"
release_id() { curl -fsS -H "$auth" "$api/releases/tags/$tag" | python3 -c 'import json, sys; print(json.load(sys.stdin)["id"])'; }
if ! id=$(release_id 2>/dev/null); then
  body="{\"tag_name\": \"$tag\", \"name\": \"Ely Office $version\"}"
  # Another platform may make it first; the read decides.
  curl -fsS -H "$auth" -H "Content-Type: application/json" -d "$body" "$api/releases" >/dev/null || true
  id=$(release_id)
fi
for file in "$dist"/*; do
  [ -f "$file" ] || continue
  name=$(basename "$file")
  echo "ci.sh: uploading $name"
  curl -fsS -H "$auth" -F "attachment=@$file" "$api/releases/$id/assets?name=$name" >/dev/null
done
echo "ci.sh: $os uploaded to $tag"
