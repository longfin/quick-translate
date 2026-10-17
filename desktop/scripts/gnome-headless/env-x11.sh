# shellcheck shell=bash
# Source this to talk to the session started by session-x11.sh: `. ./env-x11.sh`
DBUS_SESSION_BUS_ADDRESS="$(cat "${OUT:-/tmp/gnome-test-x11}/bus")"
export DBUS_SESSION_BUS_ADDRESS
export DISPLAY=:98 XDG_SESSION_TYPE=x11 GDK_BACKEND=x11 XDG_CURRENT_DESKTOP=ubuntu:GNOME
unset WAYLAND_DISPLAY
