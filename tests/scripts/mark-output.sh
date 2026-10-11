#!/usr/bin/env bash
# MARK-09, 11, 13, 14 live: look, output, Git, large files.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
[ "$(uname)" = Darwin ] || { echo "SKIP: macOS only"; exit 0; }
[ -d "${ELY_CEF:-$root/engines/cef}/Chromium Embedded Framework.framework" ] || {
  echo "SKIP: no CEF; run scripts/fetch-cef.sh"
  exit 0
}
command -v npm >/dev/null || { echo "SKIP: no npm"; exit 0; }
command -v pdftotext >/dev/null || { echo "SKIP: no pdftotext"; exit 0; }
[ -x "$root/engines/git/bin/git" ] || export ELY_GIT=${ELY_GIT:-$(command -v git)}
work=$(mktemp -d)
app=
trap 'if [ -n "$app" ]; then kill "$app" 2>/dev/null || true; wait "$app" 2>/dev/null || true; fi; pkill -f "$work" 2>/dev/null || true; rm -rf "$work"' EXIT
fail() {
  echo "FAIL: $*" >&2
  exit 1
}

"$root/scripts/build-mark-content.sh" >"$work/build" 2>&1 || fail "build: $(tail -n 5 "$work/build")"
cargo build -q -p ely-mark -p ely-tree -p ely-pdf --manifest-path "$root/Cargo.toml"
for tool in ax-values ax-press; do
  swiftc -O -o "$work/$tool" "$root/tests/scripts/$tool.swift"
done
repo=$work/repo
mkdir -p "$repo/assets"
python3 -c 'import struct,zlib,sys
c=lambda t,d: struct.pack(">I",len(d))+t+d+struct.pack(">I",zlib.crc32(t+d))
open(sys.argv[1],"wb").write(b"\x89PNG\r\n\x1a\n"+c(b"IHDR",struct.pack(">IIBBBBB",1,1,8,2,0,0,0))+c(b"IDAT",zlib.compress(b"\0\xff\0\0"))+c(b"IEND",b""))' "$repo/assets/pic.png"
cp "$repo/assets/pic.png" "$repo/assets/high.png"
doc=$repo/t.md
printf '%s\n' '# Title' '' 'Some $x^2$ math.' '' '![pic](assets/pic.png)' '' "<img src='assets/pic.png'>" '' \
  "<img alt='a > b' src='assets/p&#105;c.png'>" '' '```mermaid' 'graph TD; A-->B' '```' '' \
  '```mermaid' 'graph TD; SECRET_WORDS -->' '```' '' '<div>RAW TEXT</div><div>TWO</div>' '' \
  "<img src='assets/pic.png' srcset='assets/high.png 1x'>" '' "<img srcset=' assets/high.png 1x, assets/pic.png 2x'>" '' \
  "<picture><source srcset='assets/high.png'><img src='assets/pic.png'></picture>" '' \
  '$\privateDocumentToken$' '' '![gone](missing.png)' '' 'END MARK' >"$doc"
git -C "$repo" init -q
git -C "$repo" -c user.name=t -c user.email=t@e.x add -A
git -C "$repo" -c user.name=t -c user.email=t@e.x commit -qm one
original=$(cat "$doc")
launch() {
  ELY_LOG=info ELY_LOCALE=en ELY_NO_DIALOG=1 HOME="$work" ELY_PRINT_TO="$work/printed.pdf" \
    "$root/target/debug/ely-mark" "$@" >"$work/log" 2>&1 &
  app=$!
  waits "mark: shown as"
  osascript -e "tell application \"System Events\" to set frontmost of (first process whose unix id is $app) to true" -e 'delay 2' >/dev/null
}
waits() {
  for _ in $(seq 160); do
    grep -qF "$1" "$work/log" && return 0
    kill -0 "$app" 2>/dev/null || fail "ElyMark quit: $(tail -n 5 "$work/log")"
    sleep 0.25
  done
  fail "never saw '$1': $(grep -E 'mark:|office:' "$work/log" | tail -n 5)"
}
count() { grep -c "$1" "$work/log" || true; }
front() { osascript -e "tell application \"System Events\" to set frontmost of (first process whose unix id is $app) to true" -e 'delay 1' >/dev/null; }
key() { osascript -e "tell application \"System Events\" to $1" -e 'delay 0.8' >/dev/null; }
# The window's own titles; system menus list recent files too.
seen() { "$work/ax-values" "$app" | grep -v '^AXMenu' | grep -qF "title=$1"; }
press() {
  for _ in $(seq 40); do
    [ "$("$work/ax-press" "$app" "$@")" = pressed ] && { sleep 0.8; return 0; }
    sleep 0.25
  done
  fail "could not press $*"
}
quit() {
  kill "$app"
  wait "$app" 2>/dev/null || true
  app=
}
# Asks a `kind` export beside the document, maybe named `$2`.
ask_export() {
  front
  key 'keystroke "e" using command down'
  press "$1"
  press "Export…" AXButton
  # The panel opens in the document's folder, the stem chosen.
  sleep 3
  [ -z "${2:-}" ] || key "keystroke \"$2\""
  # The panel's own button: Return alone did not always save.
  press Save AXButton
}
# Exports `kind`; waits for its log line.
export_as() {
  local before failed
  before=$(count "mark: exported")
  failed=$(count "export failed")
  ask_export "$@"
  for _ in $(seq 160); do
    [ "$(count "mark: exported")" -gt "$before" ] && return 0
    [ "$(count "export failed")" -gt "$failed" ] && fail "export: $(grep "export failed" "$work/log" | tail -n 1)"
    sleep 0.25
  done
  fail "no $1 export: $(grep -E "mark:|office:|dialog|panel" "$work/log" | tail -n 4)"
}

launch "$doc"
[ "$("$work/ax-press" $$ none)" != untrusted ] || fail "needs Accessibility permission"
# Git sees the file; its item opens ElyTree there.
for _ in $(seq 40); do seen "Git: unchanged" && break; sleep 0.25; done
seen "Git: unchanged" || fail "no Git state: $(grep 'git says' "$work/log" | tail -n 1)"
key 'keystroke "f" using command down'
key 'key code 53'
key 'key code 125 using command down'
key 'keystroke "Z"'
for _ in $(seq 60); do seen "Git: changed" && break; sleep 0.25; done
seen "Git: changed" || fail "Git missed the change: $(grep -E "git says|autosave|saved" "$work/log" | tail -n 4)"
printf '%s\n' "$original" >"$doc"
press "Git: changed"
for _ in $(seq 40); do pgrep -f "target/debug/ely-tree .*${repo#/private}" >/dev/null && break; sleep 0.25; done
pgrep -f "target/debug/ely-tree .*${repo#/private}" >/dev/null || fail "ElyTree did not open the repository"
# ElyTree takes the file itself and shows its history (§16).
waits "tree: file history of t.md"
pkill -f "target/debug/ely-tree .*${repo#/private}" || true
sleep 2
front
# Focus mode folds the chrome; the look outlives the app.
press "Focus Mode"
! seen "t.md" || fail "focus mode kept the tabs"
press "Serif Font"
quit
grep -q '"focus": true' "$work/Library/Application Support/ElyOffice/ely-mark/look.json" || fail "the look was not kept"
[ "$(cat "$doc")" = "$original" ] || fail "the look changed the file: $(cat "$doc")"
printf '# Other\n' >"$repo/b.md"
launch "$repo/b.md" "$doc"
! seen "t.md" || fail "focus mode did not come back"
press "Focus Mode"
seen "t.md" || fail "focus mode stayed"
# Tabs open in any order; t.md's page takes the keys.
press t.md
key 'keystroke "f" using command down'
key 'key code 53'
# Each export holds the whole document.
export_as HTML
html=$repo/t.html
grep -q 'class="katex' "$html" && grep -q '<svg' "$html" && grep -q 'END MARK' "$html" ||
  fail "the HTML lacks a part"
# Both image forms inline; what was left out is said.
[ "$(grep -o 'data:image/png;base64' "$html" | wc -l | tr -d ' ')" = 6 ] || fail "an image stayed outside"
! grep -qE 'srcset|assets/' "$html" || fail "a responsive source stayed outside"
grep -q "without 1 images and 1 diagrams; formulas failed at lines \[27\]" "$work/log" ||
  fail "the lost parts went unsaid: $(grep 'mark: exported' "$work/log" | tail -n 1)"
! grep -qE 'SECRET_WORDS|privateDocumentToken|missing\.png' "$work/log" || fail "the log holds document text: $(grep -oE '.{0,80}(SECRET_WORDS|privateDocumentToken)' "$work/log" | head -n 1)"
# A name with `#` still links its folder.
export_as "HTML and Folder" "report#1"
linked=$repo/report#1.html
grep -q 'href="report%231_files/export.css"' "$linked" && grep -q 'src="report%231_files/1-pic.png"' "$linked" &&
  [ -s "$repo/report#1_files/export.css" ] || fail "the folder export: $(grep -o 'href="[^"]*"' "$linked")"
! grep -qE 'srcset|assets/' "$linked" && [ -s "$repo/report#1_files/2-high.png" ] &&
  [ "$(grep -o 'src="report%231_files/[^"]*png"' "$linked" | wc -l | tr -d ' ')" = 6 ] ||
  fail "the folder lacks a responsive image: $(ls "$repo/report#1_files")"
# A PDF never lands on the open file via links.
ln -s b.md "$repo/other.pdf"
ask_export PDF other
for _ in $(seq 20); do seen Replace && break; sleep 0.25; done
press Replace AXButton
for _ in $(seq 80); do grep -q "is open in ElyMark" "$work/log" && break; sleep 0.25; done
grep -q "is open in ElyMark" "$work/log" || fail "the export wrote over another open tab"
[ "$(cat "$repo/b.md")" = "# Other" ] || fail "another open file was overwritten"
rm "$repo/other.pdf"
ln -s t.md "$repo/alias.pdf"
ask_export PDF alias
for _ in $(seq 20); do seen Replace && break; sleep 0.25; done
press Replace AXButton
for _ in $(seq 80); do grep -q "export failed" "$work/log" && break; sleep 0.25; done
grep -q "export failed" "$work/log" || fail "the export wrote through the link"
grep -q '^# Title' "$doc" || fail "the open file was overwritten: $(head -c 20 "$doc")"
rm "$repo/alias.pdf"
export_as PDF
[ "$(head -c 5 "$repo/t.pdf")" = "%PDF-" ] || fail "no PDF"
# The toast's Open shows the PDF in ElyPDF (§16).
press Open AXButton
for _ in $(seq 40); do pgrep -f "target/debug/ely-pdf .*t.pdf" >/dev/null && break; sleep 0.25; done
pgrep -f "target/debug/ely-pdf .*t.pdf" >/dev/null || fail "Open did not show the PDF in ElyPDF"
pkill -f "target/debug/ely-pdf .*t.pdf" || true
front
pdftotext "$repo/t.pdf" - | grep -q "END MARK" || fail "the PDF lacks text"
[ "$(grep -c 'without 1 images' "$work/log")" -ge 3 ] || fail "the PDF hid a missing image"
export_as "Plain Text"
grep -q 'x^2' "$repo/t.txt" && grep -qx 'RAW TEXT' "$repo/t.txt" && grep -qx 'TWO' "$repo/t.txt" && grep -q 'END MARK' "$repo/t.txt" || fail "the text: $(cat "$repo/t.txt")"
# Print saves each job to a file here.
done_ok() { count "office: print done, printed true"; }
printed=$(done_ok)
key 'keystroke "p" using command down'
for _ in $(seq 60); do [ "$(done_ok)" -gt "$printed" ] && break; sleep 0.25; done
[ "$(done_ok)" -gt "$printed" ] || fail "print made no job: $(grep -E "mark: job|print" "$work/log" | tail -n 4)"
pdftotext "$work/printed.pdf" - | grep -q "END MARK" || fail "the printed job lacks the text"
# Two prints at once: each job renders and prints.
rm "$work/printed.pdf"
printed=$(done_ok)
jobs=$(count "mark: job .* ok true")
osascript -e 'tell application "System Events" to keystroke "p" using command down' -e 'tell application "System Events" to keystroke "p" using command down' >/dev/null
for _ in $(seq 80); do [ "$(done_ok)" -ge $((printed + 2)) ] && break; sleep 0.25; done
[ "$(done_ok)" -ge $((printed + 2)) ] || fail "a second print cancelled the first: $(grep -E "mark: job|print" "$work/log" | tail -n 6)"
[ "$(grep -o "mark: job [0-9]* printed .*, ok true" "$work/log" | tail -n 2 | cut -d' ' -f3 | sort -u | wc -l)" -eq 2 ] || fail "the two prints were not two jobs"
[ "$(count "mark: job .* ok true")" -ge $((jobs + 2)) ] || fail "a job did not render"
pdftotext "$work/printed.pdf" - | grep -q "END MARK" || fail "the last job lacks the text"
# Rich copy carries HTML; it overwrites the clipboard.
if [ "${ELY_CLIPBOARD_TEST:-}" = 1 ]; then
  key 'keystroke "f" using command down'
  key 'key code 53'
  key 'keystroke "a" using command down'
  key 'keystroke "c" using command down'
  osascript -e 'clipboard info' | grep -q 'HTML' || fail "the copy carried no HTML: $(osascript -e 'clipboard info')"
fi
quit
# A large file: source alone, every byte kept and exported.
big=$work/big.md
python3 -c 'import sys
open(sys.argv[1],"w").write("".join(f"Line {n} of a large file.\n\n" for n in range(120000))+"LAST LINE\n")' "$big"
size=$(wc -c <"$big")
launch "$big"
waits 'mark: shown as "source"'
key 'keystroke "1" using command down'
waits "a large file stays in source"
key 'key code 125 using command down'
# A digit: an input method never holds it.
key 'keystroke "9"'
for _ in $(seq 40); do grep -q 'LAST LINE' "$big" && [ "$(wc -c <"$big")" -gt "$size" ] && break; sleep 0.25; done
[ "$(wc -c <"$big")" -gt "$size" ] && grep -q 'LAST LINE' "$big" && grep -q '^Line 0 of' "$big" || fail "the large file lost bytes"
export_as "Plain Text"
grep -q 'LAST LINE' "$work/big.txt" || fail "a large export was cut"
quit
echo "ok: mark output"
