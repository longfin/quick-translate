#!/bin/bash
# Headless smoke test of the Linux build: Xvfb + a private D-Bus session, no window manager,
# no claude / codex CLI needed.
#
#   desktop/scripts/linux-smoke.sh [path/to/quicktranslate]
#
# Default binary: desktop/src-tauri/target/release/quicktranslate (`pnpm tauri build`).
# Needs: Xvfb, xclip, dbus-launch (dbus-x11), xdotool. Settings and the log go to a temp dir, so
# a real install on the same machine is left alone. Exits non-zero on the first failed check and
# prints the app's log.
#
# Checks: the app starts, builds the tray and picks the X11 clipboard watcher; copying twice
# (two clipboard owners in quick succession) and `--translate` both open the panel with the
# clipboard text and start a translation (which then finishes, or fails with "CLI not found"
# when no CLI is installed); `--settings` maps the Settings window.
set -uo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
bin="${1:-$here/../src-tauri/target/release/quicktranslate}"
[ -x "$bin" ] || { echo "linux-smoke: no executable at $bin" >&2; exit 2; }
for tool in Xvfb xclip dbus-launch xdotool; do
    command -v "$tool" >/dev/null || { echo "linux-smoke: $tool is not installed" >&2; exit 2; }
done

work="$(mktemp -d)"
export XDG_CONFIG_HOME="$work/config" XDG_STATE_HOME="$work/state" XDG_CACHE_HOME="$work/cache"
log="$XDG_STATE_HOME/QuickTranslate/QuickTranslate.log"
app_pid="" xvfb_pid=""

cleanup() {
    [ -n "$app_pid" ] && kill "$app_pid" 2>/dev/null
    # The xclip clipboard owners exit with the X server.
    [ -n "${DBUS_SESSION_BUS_PID:-}" ] && kill "$DBUS_SESSION_BUS_PID" 2>/dev/null
    [ -n "$xvfb_pid" ] && kill "$xvfb_pid" 2>/dev/null
    rm -rf "$work"
}
trap cleanup EXIT

fail() {
    echo "FAIL: $*" >&2
    echo "---- $log" >&2
    cat "$log" >&2 2>/dev/null || echo "(no log)" >&2
    echo "---- app stderr" >&2
    tail -n 40 "$work/app.err" >&2 2>/dev/null
    exit 1
}
pass() { echo "ok: $*"; }

# wait_log <extended regex> [seconds]: waits until the log has a matching line.
wait_log() {
    local pattern="$1" deadline=$((SECONDS + ${2:-20}))
    while [ $SECONDS -lt $deadline ]; do
        grep -Eq -- "$pattern" "$log" 2>/dev/null && return 0
        sleep 0.2
    done
    return 1
}

# wait_window <exact title> [seconds]: waits until a mapped window has this title.
wait_window() {
    local deadline=$((SECONDS + ${2:-10}))
    while [ $SECONDS -lt $deadline ]; do
        xdotool search --onlyvisible --name "^$1\$" >/dev/null 2>&1 && return 0
        sleep 0.2
    done
    return 1
}

# copy <text>: takes the clipboard like an app's Ctrl+C would (xclip stays as its owner).
copy() { printf '%s' "$1" | xclip -selection clipboard -i; }

# X server on the first free display (Xvfb writes the number it picked to fd 3).
Xvfb -displayfd 3 -screen 0 1280x800x24 -nolisten tcp 3>"$work/display" >"$work/xvfb.log" 2>&1 &
xvfb_pid=$!
for _ in $(seq 50); do [ -s "$work/display" ] && break; sleep 0.2; done
display="$(tr -d '[:space:]' <"$work/display")"
[ -n "$display" ] || fail "Xvfb did not start: $(cat "$work/xvfb.log")"
export DISPLAY=":$display" XDG_SESSION_TYPE=x11 GDK_BACKEND=x11
unset WAYLAND_DISPLAY
eval "$(dbus-launch --sh-syntax)"
pass "Xvfb on $DISPLAY, D-Bus session up"

"$bin" >"$work/app.out" 2>"$work/app.err" &
app_pid=$!
wait_log "QuickTranslate [0-9.]+ started" || fail "app did not start"
wait_log "tray ready" || fail "no tray"
wait_log "trigger: clipboard watcher = X11," || fail "X11 clipboard watcher not picked"
pass "started, tray ready, X11 clipboard watcher"

# Ctrl+C Ctrl+C: two clipboard owners within the double-press interval.
copy "Smoke test, copied twice."
copy "Smoke test, copied twice."
wait_log "TRIGGER source=DoubleCopy chars=25" || fail "copying twice did not trigger"
wait_log "panel: shown" || fail "panel not shown after the double copy"
wait_window "QuickTranslate" || fail "panel window not mapped"
wait_log "translate start id=1 " || fail "translation 1 did not start"
wait_log "translate (finished|failed) id=1" 60 || fail "translation 1 did not end"
pass "double copy -> panel -> translation ($(grep -Eo 'translate (finished|failed) id=1.{0,60}' "$log" | head -n1))"

# Scripting trigger from a second instance.
sleep 1 # past the double-press interval, so the next copy is a fresh first press
copy "Hello from the smoke test."
"$bin" --translate >>"$work/app.out" 2>>"$work/app.err" || fail "second instance exited with $?"
kill -0 "$app_pid" 2>/dev/null || fail "first instance died"
wait_log "second instance launched, args=\[\"--translate\"\]" || fail "--translate not forwarded"
wait_log "TRIGGER source=SecondInstance chars=26" || fail "--translate did not trigger"
wait_log "translate start id=2 " || fail "translation 2 did not start"
wait_log "translate (finished|failed) id=2" 60 || fail "translation 2 did not end"
pass "--translate -> panel -> translation"

"$bin" --settings >>"$work/app.out" 2>>"$work/app.err" || fail "second instance exited with $?"
wait_log "settings: window shown" || fail "--settings did not open the window"
wait_window "QuickTranslate Settings" || fail "Settings window not mapped"
pass "--settings -> Settings window"

kill -0 "$app_pid" 2>/dev/null || fail "app exited"
grep -q "panicked" "$work/app.err" && fail "panic on stderr"
echo "PASS: linux smoke test"
