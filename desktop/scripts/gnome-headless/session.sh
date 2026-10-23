#!/bin/bash
# Headless GNOME Shell on Wayland (virtual monitor, no XWayland) with the AppIndicator extension.
# Run it inside its own D-Bus session and leave it running:
#   dbus-run-session -- ./session.sh [outdir] &
# It writes the bus address to <outdir>/bus (for env.sh) and stops when <outdir>/stop appears.
OUT=${1:-/tmp/gnome-test}; mkdir -p "$OUT"; rm -f "$OUT/stop"
EXT=${EXT:-ubuntu-appindicators@ubuntu.com}   # Debian / Fedora: appindicatorsupport@rgcjonas.gmail.com
echo "$DBUS_SESSION_BUS_ADDRESS" > "$OUT/bus"
gsettings set org.gnome.shell disable-user-extensions false
gsettings set org.gnome.shell enabled-extensions "['$EXT']"
gnome-shell --headless --wayland --virtual-monitor 1280x800 --no-x11 >"$OUT/shell.log" 2>&1 &
echo $! > "$OUT/shell.pid"
for i in $(seq 1 60); do
  gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell \
    --method org.freedesktop.DBus.Properties.Get org.gnome.Shell ShellVersion >/dev/null 2>&1 && break
  sleep 1
done
echo "shell up after ${i}s" > "$OUT/env"
while [ ! -e "$OUT/stop" ]; do sleep 1; done
kill "$(cat "$OUT/shell.pid")"
