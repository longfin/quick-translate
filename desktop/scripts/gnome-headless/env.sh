# shellcheck shell=bash
# Source this to talk to the session started by session.sh (Wayland): `. ./env.sh`
DBUS_SESSION_BUS_ADDRESS="$(cat "${OUT:-/tmp/gnome-test}/bus")"
export DBUS_SESSION_BUS_ADDRESS
export WAYLAND_DISPLAY=wayland-0 XDG_SESSION_TYPE=wayland GDK_BACKEND=wayland XDG_CURRENT_DESKTOP=ubuntu:GNOME
unset DISPLAY
