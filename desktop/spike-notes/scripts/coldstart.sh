#!/bin/zsh
# Cold start: epoch ms before `open` vs TRAY_READY epoch in log.
APP=$(cd $(dirname $0)/../.. && pwd)/src-tauri/target/release/bundle/macos/QuickTranslateSpike.app
LOG=$HOME/Library/Logs/QuickTranslateSpike.log
for i in 1 2 3 4; do
  : > $LOG
  t0=$(python3 -c 'import time;print(int(time.time()*1000))')
  open -n --env QT_NO_PREWARM=1 $APP
  for k in $(seq 1 100); do grep -q TRAY_READY $LOG 2>/dev/null && break; sleep 0.05; done
  t1=$(grep TRAY_READY $LOG | sed -E 's/^\[([0-9]+)\].*/\1/')
  tm=$(grep "main() entered" $LOG | sed -E 's/^\[([0-9]+)\].*/\1/')
  echo "run $i: open→main ${$((tm-t0))}ms  open→TRAY_READY $((t1-t0))ms  ($(grep TRAY_READY $LOG | awk '{print $2}'))"
  sleep 1.5
  pid=$(pgrep -f QuickTranslateSpike.app/Contents/MacOS)
  echo "   pid=$pid rss_kb=$(ps -o rss= -p $pid)"
  kill $pid; sleep 1
done
