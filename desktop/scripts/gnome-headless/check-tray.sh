#!/bin/bash
# Prints the registered StatusNotifierItems, their menu paths and the dbusmenu layouts, i.e. what
# the AppIndicator extension would show. Uses the bus from env.sh (ENVFILE=env-x11.sh for X11).
# shellcheck disable=SC1090
. "${ENVFILE:-$(dirname "$0")/env.sh}"
ITEMS=$(gdbus call --session --dest org.kde.StatusNotifierWatcher --object-path /StatusNotifierWatcher \
  --method org.freedesktop.DBus.Properties.Get org.kde.StatusNotifierWatcher RegisteredStatusNotifierItems)
echo "RegisteredStatusNotifierItems: $ITEMS"
for it in $(echo "$ITEMS" | grep -o "'[^']*'" | tr -d "'"); do
  BUS=${it%%@*}; P=${it#*@}
  MENU=$(gdbus call --session --dest "$BUS" --object-path "$P" \
    --method org.freedesktop.DBus.Properties.Get org.kde.StatusNotifierItem Menu | grep -o "'[^']*'" | tr -d "'")
  echo "menu path: $MENU"
  gdbus call --session --dest "$BUS" --object-path "$MENU" \
    --method com.canonical.dbusmenu.GetLayout -- 0 -1 "['label','type','enabled','visible']"
done
