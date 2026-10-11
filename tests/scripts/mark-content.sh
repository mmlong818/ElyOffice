#!/usr/bin/env bash
# ElyMark's built page renders each package offline, live.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
[ "$(uname)" = Darwin ] || { echo "SKIP: macOS only"; exit 0; }
[ -d "${ELY_CEF:-$root/engines/cef}/Chromium Embedded Framework.framework" ] || {
  echo "SKIP: no CEF; run scripts/fetch-cef.sh"
  exit 0
}
command -v npm >/dev/null || { echo "SKIP: no npm"; exit 0; }
work=$(mktemp -d)
probe=
trap 'if [ -n "$probe" ]; then kill "$probe" 2>/dev/null || true; fi; rm -rf "$work"' EXIT
fail() {
  echo "FAIL: $*" >&2
  exit 1
}

"$root/scripts/build-mark-content.sh" >"$work/build" 2>&1 || fail "build: $(tail -n 5 "$work/build")"
content=$root/engines/content/ely-mark
for name in @milkdown/kit @codemirror/view katex mermaid; do
  grep -q "^$name " "$content/THIRD-PARTY-NOTICES.txt" || fail "no notice for $name"
done
cargo build -q -p ely-mark --example markprobe --manifest-path "$root/Cargo.toml"
swiftc -O -o "$work/share" "$root/tests/scripts/window-share.swift"
ELY_LOG=info ELY_LOCALE=en ELY_SMOKE=hold HOME="$work" XDG_DATA_HOME="$work/data" \
  perl -e 'alarm 120; exec @ARGV' "$root/target/debug/examples/markprobe" >"$work/out" 2>"$work/err" &
probe=$!
for _ in $(seq 240); do
  grep -q '"step":"visual"' "$work/out" && break
  kill -0 "$probe" 2>/dev/null || break
  sleep 0.25
done
grep -q '"step":"visual"' "$work/out" || fail "no visual report: $(tail -n 5 "$work/err")"
# Less than all background: the page is drawn.
bg=$(grep '"step":"visual"' "$work/out" | grep -oE '"bg":"rgba\([0-9]+, [0-9]+, [0-9]+' | grep -oE '[0-9]+, [0-9]+, [0-9]+$' | tr -d ' ')
share=$("$work/share" "$probe" "$bg")
case $share in
  denied) fail "needs Screen Recording permission" ;;
  none) fail "the probe has no window" ;;
esac
awk -v s="$share" 'BEGIN { exit !(s < 0.97) }' || fail "the page is not on screen (background share $share)"
wait "$probe" || fail "the probe failed: $(tail -n 5 "$work/err")"
probe=
grep -qxF "mark: done" "$work/out" || fail "unfinished: $(cat "$work/out")"
# The earlier of two quick shows never reports.
shows=$(grep '^mark: shown' "$work/out" | tr -d '"' | cut -d' ' -f3 | tr '\n' ' ')
[ "$shows" = "visual preview source source visual visual preview source source visual preview source visual source source visual preview source source source visual visual visual " ] || fail "shows reported: $shows"
python3 - "$work/out" <<'PY'
import json, sys
seen = {}
for line in open(sys.argv[1]):
    if line.startswith("mark: {"):
        read = json.loads(line[6:])
        seen[read["step"]] = read
def want(step, key, value):
    if seen[step][key] != value:
        sys.exit(f"FAIL: {step} {key} is {seen[step][key]!r}, not {value!r}")
languages = ["rust", "LaTeX", "LaTeX", "mermaid", "mermaid"]
for step in ("visual", "themed", "preview"):
    for key, value in [("title", "Title"), ("heading", True), ("inline", True), ("display", True), ("tasks", 2),
                       ("cells", 2), ("notes", 1), ("font", True), ("editors", 1)]:
        want(step, key, value)
    # Every task box: text-sized, centred on its line.
    for width, height, off, drawn in seen[step]["boxes"]:
        if not drawn or abs(width - 1) > 0.01 or abs(height - 1) > 0.01 or abs(off) > 1:
            sys.exit(f"FAIL: {step} task boxes are {seen[step]['boxes']}")
    blocks = seen[step]["blocks"]
    if [block["language"] for block in blocks] != languages:
        sys.exit(f"FAIL: {step} blocks are {blocks}")
    # Preview shows a block's preview alone, when it has one.
    for at, block in enumerate(blocks):
        if block["editors"] != 1 or block["code"] != (step != "preview" or at == 0):
            sys.exit(f"FAIL: {step} block {at} code is not shown: {block}")
    width, height = blocks[3]["diagram"]
    if not blocks[1]["math"] or not blocks[3]["drawn"] or width < 100 or height < 80:
        sys.exit(f"FAIL: {step} math or diagram is not shown: {blocks}")
    if "(9)" not in blocks[2]["failure"] or "line 3" not in blocks[4]["failure"]:
        sys.exit(f"FAIL: {step} errors lack source or place: {blocks}")
    editable = "true" if step != "preview" else "false"
    if any(block["editable"] != editable for block in blocks):
        sys.exit(f"FAIL: {step} code blocks are not {editable}: {blocks}")
fills = [seen[step]["blocks"][3]["fill"] for step in ("visual", "themed")]
if not fills[0] or fills[0] == fills[1]:
    sys.exit(f"FAIL: the diagram kept its colors: {fills}")
want("visual", "editable", "true")
want("preview", "editable", "false")
want("source", "first", "# Title")
want("source", "source", True)
want("source", "blocks", [])
want("race", "mode", "source")
want("race", "first", "# NEW")
want("race", "editors", 1)
want("asked", "mode", "preview")
want("stale", "mode", "source")
want("owner", "mode", "source")
# Two broken patterns per mode error; empty ones open nothing.
if seen["find"]["panels"] != {"source": 0, "visual": 0} or seen["find"]["errors"] != 4:
    sys.exit(f"FAIL: find: {seen['find']}")
# A zero-width match after an emoji: seven hits, per mode.
if seen["find"]["totals"].count(7) != 2:
    sys.exit(f"FAIL: zero-width find: {seen['find']}")
look = seen["look"]
# 36rem at 120 percent of 16px: 691.2px.
if look["dim"] != look["blocks"] - 1 or look["bright"] != "1" or not look["centered"] or look["width"] != "691.2px" or not look["serif"]:
    sys.exit(f"FAIL: the writing view: {look}")
if "OLD" in seen["race"]["text"]:
    sys.exit("FAIL: the earlier show stayed")
# A diagram begun before a change never lands after.
language = seen["language"]["blocks"][0]
if language["language"].lower() != "rust" or language["drawn"] or language["diagram"] != [0, 0]:
    sys.exit(f"FAIL: an old diagram stayed in a Rust block: {language}")
late = seen["midtheme"]["blocks"][0]
if not late["drawn"] or {late["fill"], late["first"]} != {seen["visual"]["blocks"][3]["fill"]}:
    sys.exit(f"FAIL: a diagram drawn mid-change showed the old theme: {late}")
# Draws for a gone editor never run.
if seen["release"]["long"] > 1:
    sys.exit(f"FAIL: {seen['release']['long']} long stalls after leaving the diagrams")
PY
! grep -q "refused" "$work/err" || fail "the page reached outside: $(grep -m3 refused "$work/err")"
! grep -q "panicked" "$work/err" || fail "panics: $(grep -m3 panicked "$work/err")"
echo "ok: mark content (background share $share)"
