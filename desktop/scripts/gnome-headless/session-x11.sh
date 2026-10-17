#!/bin/bash
# GNOME Shell as the X11 window manager / compositor on Xvfb :98, with the AppIndicator extension.
# Run it inside its own D-Bus session and leave it running:
#   dbus-run-session -- ./session-x11.sh [outdir] &
# It writes the bus address to <outdir>/bus (for env-x11.sh) and stops when <outdir>/stop appears.
OUT=${1:-/tmp/gnome-test-x11}; mkdir -p "$OUT"; rm -f "$OUT/stop"
EXT=${EXT:-ubuntu-appindicators@ubuntu.com}   # Debian / Fedora: appindicatorsupport@rgcjonas.gmail.com
echo "$DBUS_SESSION_BUS_ADDRESS" > "$OUT/bus"
Xvfb :98 -screen 0 1280x800x24 >"$OUT/xvfb.log" 2>&1 & echo $! > "$OUT/xvfb.pid"
sleep 1
export DISPLAY=:98 XDG_SESSION_TYPE=x11 XDG_CURRENT_DESKTOP=ubuntu:GNOME
gsettings set org.gnome.shell disable-user-extensions false
gsettings set org.gnome.shell enabled-extensions "['$EXT']"
gnome-shell --x11 --replace >"$OUT/shell.log" 2>&1 & echo $! > "$OUT/shell.pid"
for i in $(seq 1 60); do
  gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell \
    --method org.freedesktop.DBus.Properties.Get org.gnome.Shell ShellVersion >/dev/null 2>&1 && break
  sleep 1
done
echo "shell up after ${i}s" > "$OUT/env"
while [ ! -e "$OUT/stop" ]; do sleep 1; done
kill "$(cat "$OUT/shell.pid")"; sleep 1; kill "$(cat "$OUT/xvfb.pid")"
