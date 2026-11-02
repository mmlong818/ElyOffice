#!/usr/bin/env bash
# nix.sh <tag>: nix/release.json from that release's AppImage manifests.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
tag=${1:?the release tag}
# ELY_NIX_DOWNLOAD replaces the release URL; tests use it.
download=${ELY_NIX_DOWNLOAD:-https://github.com/ELY-Open-Source/ElyOffice/releases/download}/$tag
rows=$(awk -F'\t' '$4 == 1 { print $1 "\t" $2 "\t" $5 }' "$root/packaging/apps.tsv")
manifests=$(mktemp -d)
trap 'rm -rf "$manifests"' EXIT
while IFS=$'\t' read -r slug _ _; do
  curl -fsSL -o "$manifests/$slug.json" "$download/$slug-linux-x86_64.json"
done <<<"$rows"
printf '%s\n' "$rows" >"$manifests/rows.tsv"
python3 - "$tag" "$manifests" "$root/nix/release.json" <<'EOF'
import json, sys
tag, manifests, out = sys.argv[1:]
apps = {}
for row in open(f"{manifests}/rows.tsv").read().splitlines():
    slug, name, license = row.split("\t")
    manifest = json.load(open(f"{manifests}/{slug}.json"))
    if "v" + manifest["version"] != tag:
        sys.exit(f"nix.sh: {slug}'s manifest is {manifest['version']}, not {tag}")
    asset = manifest["assets"]["linux-x86_64"]
    apps[slug] = {"name": name, "license": license, "url": asset["url"], "sha256": asset["sha256"]}
release = {"version": tag[1:], "apps": apps}
open(out, "w").write(json.dumps(release, indent=2) + "\n")
print(out)
EOF
