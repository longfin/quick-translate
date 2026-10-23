# Headless GNOME sessions

For checking the tray icon and the triggers under a real GNOME Shell without a desktop, e.g. in
a VM or over SSH. Two flavours: Wayland (`session.sh`, headless virtual monitor, no XWayland, so the
clipboard watcher reports `Unsupported`) and X11 (`session-x11.sh`, GNOME Shell on Xvfb `:98`).

Needs `gnome-shell`, the AppIndicator extension (`gnome-shell-extension-appindicator`), and
`xvfb` for the X11 one. The extension's UUID is `ubuntu-appindicators@ubuntu.com` on Ubuntu; set
`EXT=appindicatorsupport@rgcjonas.gmail.com` elsewhere.

```bash
cd desktop/scripts/gnome-headless
dbus-run-session -- ./session.sh &      # Wayland; writes /tmp/gnome-test/{bus,env,shell.log}
sleep 10; cat /tmp/gnome-test/env       # "shell up after Ns"
. ./env.sh                              # this shell now talks to that session
../../src-tauri/target/release/quicktranslate &
./check-tray.sh                         # the registered tray item and its menu
touch /tmp/gnome-test/stop              # ends the session
```

For X11 use `session-x11.sh` (output in `/tmp/gnome-test-x11`), `. ./env-x11.sh` and
`ENVFILE=./env-x11.sh ./check-tray.sh`. Pass another directory as the session script's first
argument and set `OUT` to the same directory before sourcing the env file to run several at once.

`check-tray.sh` prints the `StatusNotifierItem`s the extension sees and each one's dbusmenu
layout, so a missing tray icon or untranslated menu shows up without a screenshot. The app's log
(`~/.local/state/QuickTranslate/QuickTranslate.log`) names the clipboard watcher it picked.
