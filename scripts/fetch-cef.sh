#!/usr/bin/env bash
# CEF's minimal build at cef-rs's pin, SHA-1 checked, into engines/cef.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
[ -d "$root/reference/cef-rs" ] || { echo "fetch-cef: run scripts/fetch-references.sh first" >&2; exit 1; }
# Not the system temp: Windows may block programs there.
mkdir -p "$root/engines"
work=$(mktemp -d "$root/engines/cef-export.XXXXXX")
trap 'rm -rf "$work"' EXIT
cd "$root/reference/cef-rs"
# The exporter checks each archive against CEF's own index.
CARGO_TARGET_DIR="$work" cargo run -q -p export-cef-dir -- --force "$root/engines/cef"
rm -f "$root"/engines/cef_binary_*.tar.bz2
cat "$root/engines/cef/archive.json"
