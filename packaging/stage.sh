#!/usr/bin/env bash
# Stages an app with its engines. Usage: stage.sh <slug> <dir>
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
slug=${1:?app slug, such as ely-doc}
out=${2:?the folder to fill}
case $(uname) in
  MINGW* | MSYS*) exe=.exe ;;
  Linux) exe= ;;
  *)
    echo "stage.sh: Windows or Linux only" >&2
    exit 1
    ;;
esac
grep -q "^$slug	" "$root/packaging/apps.tsv" || {
  echo "stage.sh: unknown app $slug" >&2
  exit 1
}
uses() { grep -q "^$1" "$root/apps/$slug/Cargo.toml"; }
need() { # path, what to run
  [ -e "$1" ] || {
    echo "stage.sh: no $1; run $2" >&2
    exit 1
  }
}
build() { cargo build --manifest-path "$root/Cargo.toml" --release "$@"; }
release=${CARGO_TARGET_DIR:-$root/target}/release

rm -rf "$out"
mkdir -p "$out"
build -p "$slug"
cp "$release/$slug$exe" "$out/"
if uses ely-lok; then
  lo=${ELY_LO_DIR:-$root/engines/lo-src/instdir}
  need "$lo/program" scripts/build-libreoffice.sh
  build -p ely-lok --bin ely-lok-worker
  cp "$release/ely-lok-worker$exe" "$out/"
  # The kernel in LibreOffice's layout, beside the app.
  mkdir "$out/libreoffice"
  cp -R "$lo/program" "$lo/share" "$lo/presets" "$out/libreoffice/"
  cp "$lo/LICENSE.html" "$lo/NOTICE" "$lo/CREDITS.fodt" "$out/libreoffice/"
  # Merged-library builds leave text placeholders.
  while IFS= read -r stub; do
    if [ "$(cat "$stub")" = "invalid - merged lib" ]; then
      rm "$stub"
    fi
  done < <(find "$out/libreoffice/program" -type f -size -64c)
  if [ -n "$exe" ]; then
    # The kernel needs the VC++ runtime; it ships beside it.
    crt=$(ls -d "/c/Program Files/Microsoft Visual Studio/2022/"*/VC/Redist/MSVC/*/x64/Microsoft.VC143.CRT | tail -n 1)
    need "$crt" "the Visual Studio 2022 C++ tools"
    cp "$crt"/*.dll "$out/libreoffice/program/"
  fi
fi
if uses ely-pdf-engine; then
  tessdata=${ELY_TESSDATA:-$root/engines/tessdata}
  need "$tessdata/eng.traineddata" scripts/fetch-tessdata.sh
  build -p ely-pdf-engine --bin ely-pdf-worker
  cp "$release/ely-pdf-worker$exe" "$out/"
  mkdir "$out/tessdata"
  cp "$tessdata"/{eng,chi_sim,chi_tra}.traineddata "$out/tessdata/"
fi
if [ "$slug" = ely-tree ] || [ "$slug" = ely-mark ]; then
  need "$root/engines/git" "scripts/build-git.sh and scripts/fetch-git-lfs.sh"
  cp -R "$root/engines/git" "$out/git"
fi
if uses ely-web-content; then
  cef=${ELY_CEF:-$root/engines/cef}
  need "$cef/icudtl.dat" scripts/fetch-cef.sh
  # CEF's runtime sits beside the app, as cef-dll-sys has it.
  find "$cef" -maxdepth 1 -type f \( -name '*.dll' -o -name '*.so' -o -name '*.so.*' -o -name '*.pak' \
    -o -name '*.bin' -o -name '*.dat' -o -name '*.json' -o -name chrome-sandbox \) -exec cp {} "$out/" \;
  cp -R "$cef/locales" "$out/"
  content=$root/engines/content/$slug
  need "$content/index.html" "its scripts/build-*-content.sh"
  cp -R "$content" "$out/content"
fi
"$root/packaging/licenses.sh" "$slug" "$out/licenses"
echo "$out"
