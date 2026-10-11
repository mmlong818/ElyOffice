#!/usr/bin/env bash
# MARK-08, MARK-12, QA-22 live: outside changes, autosave, restore.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
[ "$(uname)" = Darwin ] || { echo "SKIP: macOS only"; exit 0; }
[ -d "${ELY_CEF:-$root/engines/cef}/Chromium Embedded Framework.framework" ] || {
  echo "SKIP: no CEF; run scripts/fetch-cef.sh"
  exit 0
}
command -v npm >/dev/null || { echo "SKIP: no npm"; exit 0; }
work=$(mktemp -d)
app=
trap 'if [ -n "$app" ]; then kill "$app" 2>/dev/null || true; wait "$app" 2>/dev/null || true; fi; rm -rf "$work"' EXIT
fail() {
  echo "FAIL: $*" >&2
  exit 1
}

"$root/scripts/build-mark-content.sh" >"$work/build" 2>&1 || fail "build: $(tail -n 5 "$work/build")"
cargo build -q -p ely-mark --manifest-path "$root/Cargo.toml"
for tool in ax-values ax-press; do
  swiftc -O -o "$work/$tool" "$root/tests/scripts/$tool.swift"
done
mkdir "$work/notes"
doc=$work/notes/t.md
printf 'one\n\ntwo\n' >"$doc"
launch() {
  ELY_LOG=info ELY_LOCALE=en ELY_NO_DIALOG=1 HOME="$work" \
    "$root/target/debug/ely-mark" "$@" >"$work/out" 2>"$work/err" &
  app=$!
  waits "ElyMark window open"
  osascript -e "tell application \"System Events\" to set frontmost of (first process whose unix id is $app) to true" -e 'delay 2' >/dev/null
}
waits() {
  for _ in $(seq 120); do
    grep -qF "$1" "$work/err" && return 0
    kill -0 "$app" 2>/dev/null || fail "ElyMark quit: $(tail -n 5 "$work/err")"
    sleep 0.25
  done
  fail "never saw '$1': $(grep -E 'mark|office' "$work/err" | tail -n 5)"
}
count() { grep -c "$1" "$work/err" || true; }
# Waits for `$1` to show more than `$2` times.
more() {
  for _ in $(seq 120); do
    [ "$(count "$1")" -gt "$2" ] && return 0
    sleep 0.25
  done
  fail "no new '$1': $(grep -E 'mark:|recovery' "$work/err" | tail -n 6)"
}
# Types unpaused: a write lands within the autosave second.
now() { osascript -e "tell application \"System Events\" to keystroke \"$1\"" >/dev/null; }
key() { osascript -e "tell application \"System Events\" to $1" -e 'delay 0.8' >/dev/null; }
press() {
  for _ in $(seq 20); do
    [ "$("$work/ax-press" "$app" "$1")" = pressed ] && { sleep 0.8; return 0; }
    sleep 0.25
  done
  fail "could not press $1"
}
# The window's own titles; system menus list recent files too.
seen() { "$work/ax-values" "$app" | grep -v '^AXMenu' | grep -qF "title=$1"; }
# Waits for an autosave past the `n` seen so far.
autosaved() {
  for _ in $(seq 40); do
    [ "$(count "autosaves")" -gt "$1" ] && sleep 1 && return 0
    sleep 0.25
  done
  fail "no autosave: $(tr '\n' '/' <"$doc") $(grep -E 'mark:|office:' "$work/err" | tail -n 5)"
}
is() { [ "$(cat "$1")" = "$2" ] || fail "$3: $(cat "$1")"; }

launch "$doc"
waits 'mark: shown as "visual"'
[ "$("$work/ax-press" $$ none)" != untrusted ] || fail "needs Accessibility permission"
# A clean file follows an atomic replace; typing autosaves.
printf 'one\n\ntwo\n\nthree\n' >"$doc.tmp"
mv "$doc.tmp" "$doc"
waits "changed outside; unsaved false"
sleep 1
before=$(count autosaves)
key 'key code 125 using command down'
key 'keystroke "Z"'
autosaved "$before"
is "$doc" $'one\n\ntwo\n\nthreeZ' "reload then autosave"
# Unsaved text meets an outside write: nothing is overwritten.
now X
printf 'outside\n' >"$doc"
waits "changed outside; unsaved true"
sleep 2
is "$doc" "outside" "an outside write was overwritten"
press "Compare…"
seen "Use the Merge" || fail "no compare dialog"
press "Keep Yours"
waits "settles with Mine"
sleep 1
is "$doc" $'one\n\ntwo\n\nthreeZX' "keep yours"
# Autosave runs again once the user chose.
before=$(count autosaves)
key 'keystroke "K"'
autosaved "$before"
is "$doc" $'one\n\ntwo\n\nthreeZXK' "autosave after keep"
# Unsent keys survive a touch that keeps the text.
before=$(count autosaves)
changed=$(count "changed outside")
now QQ
touch "$doc"
more "changed outside" "$changed"
autosaved "$before"
is "$doc" $'one\n\ntwo\n\nthreeZXKQQ' "keys typed before a touch were lost"
before=$(count autosaves)
key 'key code 51'
key 'key code 51'
autosaved "$before"
is "$doc" $'one\n\ntwo\n\nthreeZXK' "keys taken back"
# Same text, new bytes: saves keep the new bytes.
lf=$(od -An -tx1 "$doc" | tr -d ' \n')
python3 -c 'import sys; d = open(sys.argv[1], "rb").read(); open(sys.argv[2], "wb").write(b"\xef\xbb\xbf" + d.replace(b"\n", b"\r\n"))' "$doc" "$work/crlf"
# BOM, only CRLF line ends, and `$1` in it.
kept() { python3 -c 'import sys; d = open(sys.argv[1], "rb").read(); sys.exit(not (d.startswith(b"\xef\xbb\xbf") and d.count(b"\n") == d.count(b"\r\n") and sys.argv[2].encode() in d))' "$doc" "$1"; }
changed=$(count "changed outside")
cp "$work/crlf" "$doc"
more "changed outside" "$changed"
before=$(count autosaves)
now W
autosaved "$before"
kept W || fail "a save lost the new bytes: $(od -c "$doc" | head -3)"
before=$(count autosaves)
key 'key code 51'
autosaved "$before"
cmp -s "$doc" "$work/crlf" || fail "the save is not the new bytes: $(od -c "$doc" | head -3)"
changed=$(count "changed outside")
printf '%s' "$lf" | xxd -r -p >"$doc"
more "changed outside" "$changed"
# A conflict the disk takes back resumes autosave.
cp "$doc" "$work/saved"
conflicts=$(count "unsaved true")
now X
printf 'external\n' >"$doc"
more "unsaved true" "$conflicts"
cp "$work/saved" "$doc"
waits "local edits stay"
before=$(count autosaves)
now Y
autosaved "$before"
is "$doc" $'one\n\ntwo\n\nthreeZXKXY' "autosave after the disk came back"
before=$(count autosaves)
key 'key code 51'
key 'key code 51'
autosaved "$before"
is "$doc" $'one\n\ntwo\n\nthreeZXK' "keys taken back again"
# Unsaved keys meet the saved text in new bytes.
cp "$doc" "$work/saved"
python3 -c 'import sys; d = open(sys.argv[1], "rb").read(); open(sys.argv[2], "wb").write(b"\xef\xbb\xbf" + d.replace(b"\n", b"\r\n"))' "$doc" "$work/crlf"
stays=$(count "local edits stay")
before=$(count autosaves)
now V
cp "$work/crlf" "$doc"
more "local edits stay" "$stays"
autosaved "$before"
kept V || fail "edits kept lost the new bytes: $(od -c "$doc" | head -3)"
before=$(count autosaves)
key 'key code 51'
autosaved "$before"
cmp -s "$doc" "$work/crlf" || fail "not the new bytes: $(od -c "$doc" | head -3)"
changed=$(count "changed outside")
cp "$work/saved" "$doc"
more "changed outside" "$changed"
# Unsaved text written outside as is: the tab is clean.
python3 -c 'import sys; d = open(sys.argv[1], "rb").read(); open(sys.argv[2], "wb").write(d[:-1] + b"S\n" if d.endswith(b"\n") else d + b"S")' "$doc" "$work/typed"
cp "$doc" "$work/saved"
changed=$(count "changed outside")
now S
cp "$work/typed" "$doc"
more "changed outside" "$changed"
sleep 2
clean=$(count "unsaved false")
printf 'gamma\n' >>"$doc"
more "unsaved false" "$clean"
sleep 1
grep -q gamma "$doc" || fail "the outside change was not taken"
changed=$(count "changed outside")
cp "$work/saved" "$doc"
more "changed outside" "$changed"
# A save that meets the same text keeps saving after.
python3 -c 'import sys; d = open(sys.argv[1], "rb").read(); open(sys.argv[2], "wb").write(d[:-1] + b"T\n" if d.endswith(b"\n") else d + b"T")' "$doc" "$work/same"
before=$(count autosaves)
now T
for _ in $(seq 200); do [ "$(count autosaves)" -gt "$before" ] && break; sleep 0.01; done
cp "$work/same" "$doc"
sleep 2
before=$(count autosaves)
now U
autosaved "$before"
grep -q TU "$doc" || fail "the next edit after a same-text save was not saved: $(tr '\n' '/' <"$doc")"
before=$(count autosaves)
key 'key code 51'
key 'key code 51'
autosaved "$before"
# Edits apart merge; a disk change mid-way compares again.
key 'key code 126 using command down'
conflicts=$(count "unsaved true")
now A
printf 'one\n\ntwo\n\nthreeZXK\n\ntail\n' >"$doc"
more "unsaved true" "$conflicts"
press "Compare…"
compares=$(count "compares tab")
printf 'one\n\ntwo\n\nthreeZXK\n\ntail\n\nmore\n' >"$doc"
press "Use the Merge"
waits "during the comparison"
more "compares tab" "$compares"
is "$doc" $'one\n\ntwo\n\nthreeZXK\n\ntail\n\nmore' "a stale merge was written"
press "Use the Merge"
waits "settles with Merged"
before=$(count autosaves)
autosaved "$before"
is "$doc" $'Aone\n\ntwo\n\nthreeZXK\n\ntail\n\nmore' "merge"
# A rename outside: the tab follows, saves the new name.
mv "$doc" "$work/notes/u.md"
waits "was renamed outside"
seen "u.md" || fail "the tab kept its old name"
before=$(count autosaves)
key 'keystroke "R"'
autosaved "$before"
[ ! -e "$doc" ] || fail "the old name came back"
grep -q '^RAone' "$work/notes/u.md" || fail "rename save: $(cat "$work/notes/u.md")"
# An unreadable change says so; a fresh look recovers.
clean=$(count "unsaved false")
printf 'locked\n' >"$work/notes/u.md"
chmod 000 "$work/notes/u.md"
for _ in $(seq 40); do seen "Check Again" && break; sleep 0.25; done
seen "Check Again" || fail "no offer to look again"
chmod 644 "$work/notes/u.md"
press "Check Again"
more "unsaved false" "$clean"
# A delete keeps the text and never recreates the file.
rm "$work/notes/u.md"
waits "was deleted outside"
seen "Save As…" || fail "no Save As offer"
before=$(count autosaves)
key 'keystroke "D"'
sleep 3
[ "$(count autosaves)" = "$before" ] || fail "autosave ran on a deleted file"
[ ! -e "$work/notes/u.md" ] || fail "autosave recreated a deleted file"
kill "$app"
wait "$app" 2>/dev/null || true
app=
! grep -q "panicked" "$work/err" || fail "panics: $(grep -m3 panicked "$work/err")"
# The killed run above left a crash; start clean.
rm -rf "$work/Library/Application Support/ElyOffice/ely-mark/"{session.json,session.lock,recovery}
# A clean quit keeps the tabs, mode, line and column.
printf 'kept\n\nsecond\n\nthird\n' >"$doc"
gone=$work/notes/w.md
printf 'other\n' >"$gone"
launch "$gone" "$doc"
more 'mark: shown as "visual"' 1
press t.md
key 'keystroke "f" using command down'
key 'key code 53'
key 'keystroke "2" using command down'
waits 'mark: shown as "source"'
key 'key code 125 using command down'
key 'key code 126'
key 'key code 123 using command down'
key 'key code 124'
key 'key code 124'
sleep 1
key 'keystroke "q" using command down'
for _ in $(seq 40); do kill -0 "$app" 2>/dev/null || break; sleep 0.25; done
! kill -0 "$app" 2>/dev/null || fail "Cmd+Q did not quit"
app=
launch
waits "restores tab"
waits 'mark: shown as "source"'
sleep 1
# Restored tabs open in any order; pick t.md's page.
press t.md
key 'keystroke "f" using command down'
key 'key code 53'
before=$(count autosaves)
key 'keystroke "Q"'
autosaved "$before"
is "$doc" $'kept\n\nsecond\n\nthQird' "the restored caret"
# Unsaved text, even a deleted file's, returns as copies.
points=$(count "recovery: a point")
press t.md
conflicts=$(count "unsaved true")
now Y
printf 'disk\n' >"$doc"
more "unsaved true" "$conflicts"
press w.md
# Closing the find box hands the keys to the page.
key 'keystroke "f" using command down'
key 'key code 53'
conflicts=$(count "unsaved true")
now V
printf 'gone\n' >"$gone"
more "unsaved true" "$conflicts"
rm "$gone"
more "recovery: a point" "$((points + 1))"
kill -9 "$app"
wait "$app" 2>/dev/null || true
app=
launch
waits "ended Crashed"
waits "did not open"
for _ in $(seq 40); do [ "$(count ": unsaved text from .* is back")" -ge 2 ] && break; sleep 0.25; done
[ "$(count ": unsaved text from .* is back")" = 2 ] || fail "recovered copies: $(grep "unsaved text from" "$work/err")"
sleep 3
is "$doc" "disk" "recovered text went into the file"
[ ! -e "$gone" ] || fail "a deleted file came back"
# Each copy holds its own text, past the first sweep.
rec="$work/Library/Application Support/ElyOffice/ely-mark/recovery"
session="$work/Library/Application Support/ElyOffice/ely-mark/session.json"
sleep 12
withy=$(grep -rlF Y "$rec" --include='*.bin' | head -n 1 || true)
withv=$(grep -rlF V "$rec" --include='*.bin' | head -n 1 || true)
[ -n "$withy" ] && [ -n "$withv" ] || fail "a copy lost its text: $(ls -R "$rec")"
! grep -qF V "$withy" && ! grep -qF Y "$withv" || fail "a copy took another's text"
# A cancelled Save panel stops the quit; the list follows.
key 'keystroke "q" using command down'
for _ in $(seq 20); do seen "Don't Save" && break; sleep 0.25; done
[ "$("$work/ax-press" "$app" Save AXButton)" = pressed ] || fail "no Save button"
sleep 2
key 'key code 53'
key 'keystroke "n" using command down'
sleep 2
points=$(count "recovery: a point")
key 'keystroke "third"'
more "recovery: a point" "$points"
for _ in $(seq 60); do [ "$(grep -c '"slot"' "$session")" = 3 ] && ! grep -q '"path": "' "$session" && break; sleep 0.5; done
[ "$(grep -c '"slot"' "$session")" = 3 ] && ! grep -q '"path": "' "$session" || fail "the kept list froze: $(tr -d " \n" <"$session")"
# A discard holds even through a crash right after.
key 'keystroke "q" using command down'
for _ in $(seq 20); do seen "Don't Save" && break; sleep 0.25; done
press "Don't Save"
for _ in $(seq 20); do seen "Don't Save" && break; sleep 0.25; done
press Cancel
kill -9 "$app"
wait "$app" 2>/dev/null || true
app=
launch
waits "ended Crashed"
sleep 4
[ "$(count ": unsaved text from .* is back")" = 2 ] || fail "after a discard: $(grep ": unsaved text from .* is back" "$work/err")"
key 'keystroke "q" using command down'
for _ in $(seq 4); do
  for _ in $(seq 20); do seen "Don't Save" && break; sleep 0.25; done
  seen "Don't Save" || break
  press "Don't Save"
done
for _ in $(seq 40); do kill -0 "$app" 2>/dev/null || break; sleep 0.25; done
app=
# A discard is not a crash: nothing comes back.
launch
sleep 4
! grep -q "unsaved text from" "$work/err" || fail "discarded text came back"
kill "$app"
wait "$app" 2>/dev/null || true
app=
# A tab discarded, then a crash: it stays gone.
launch
key 'keystroke "n" using command down'
sleep 2
points=$(count "recovery: a point")
key 'keystroke "solo"'
more "recovery: a point" "$points"
[ "$("$work/ax-press" "$app" Close AXButton)" = pressed ] || fail "no tab Close"
for _ in $(seq 20); do seen "Don't Save" && break; sleep 0.25; done
press "Don't Save"
kill -9 "$app"
wait "$app" 2>/dev/null || true
app=
launch
waits "ended Crashed"
sleep 4
! grep -q "unsaved text from" "$work/err" || fail "a discarded tab came back"
kill "$app"
wait "$app" 2>/dev/null || true
app=
# A quit's conflict reloaded stops it; the list follows.
printf 'base\n' >"$doc"
launch "$doc"
waits 'mark: shown as "visual"'
conflicts=$(count "unsaved true")
now L
printf 'other\n' >"$doc"
more "unsaved true" "$conflicts"
key 'keystroke "q" using command down'
for _ in $(seq 20); do seen "Don't Save" && break; sleep 0.25; done
[ "$("$work/ax-press" "$app" Save AXButton)" = pressed ] || fail "no Save button"
for _ in $(seq 20); do seen "Reload from Disk" && break; sleep 0.25; done
press "Reload from Disk"
sleep 2
key 'keystroke "n" using command down'
sleep 2
points=$(count "recovery: a point")
key 'keystroke "afterreload"'
more "recovery: a point" "$points"
for _ in $(seq 60); do [ "$(grep -c '"slot"' "$session")" = 2 ] && break; sleep 0.5; done
[ "$(grep -c '"slot"' "$session")" = 2 ] || fail "the list froze after a reload: $(tr -d " \n" <"$session")"
key 'keystroke "q" using command down'
for _ in $(seq 20); do seen "Don't Save" && break; sleep 0.25; done
press "Don't Save"
for _ in $(seq 40); do kill -0 "$app" 2>/dev/null || break; sleep 0.25; done
app=
# A quit's first save keeps its file next time.
rm -f "$session"
launch
key 'keystroke "n" using command down'
sleep 2
key 'keystroke "savedtext"'
key 'keystroke "q" using command down'
for _ in $(seq 20); do seen "Don't Save" && break; sleep 0.25; done
[ "$("$work/ax-press" "$app" Save AXButton)" = pressed ] || fail "no Save button"
sleep 2
key 'keystroke "g" using {command down, shift down}'
key "keystroke \"$work/notes\""
key 'key code 36'
key 'key code 36'
for _ in $(seq 40); do kill -0 "$app" 2>/dev/null || break; sleep 0.25; done
! kill -0 "$app" 2>/dev/null || fail "the quit did not finish after saving"
app=
saved=$(grep -lF savedtext "$work/notes"/*.md || true)
[ -n "$saved" ] || fail "nothing saved: $(ls "$work/notes")"
launch
waits "restores tab"
kill "$app"
wait "$app" 2>/dev/null || true
app=
# A session kept before columns were still opens.
printf '{"tabs":[{"path":"%s","name":"t.md","mode":"source","place":{"line":1,"scroll":0},"slot":""}]}' "$doc" >"$session"
rm -f "$work/Library/Application Support/ElyOffice/ely-mark/session.lock"
launch
waits "restores tab"
waits 'mark: shown as "source"'
kill "$app"
wait "$app" 2>/dev/null || true
app=
# A clean tab with the disk's text closes without asking.
clean=$work/notes/c.md
printf 'clean\n' >"$clean"
launch "$clean"
waits 'mark: shown as "visual"'
changes=$(count "unsaved false")
printf 'outside\n' >"$clean.tmp"
mv "$clean.tmp" "$clean"
more "unsaved false" "$changes"
sleep 1
[ "$("$work/ax-press" "$app" Close AXButton)" = pressed ] || fail "no tab Close"
sleep 2
! seen "Don't Save" || fail "a clean tab that took the disk's text asked to save"
kill "$app"
wait "$app" 2>/dev/null || true
app=
echo "ok: mark outside"
