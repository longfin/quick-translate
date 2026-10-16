#!/bin/zsh
# Run N triggers via single pbcopy (needs QT_TRIGGER=single) and print the per-run timings.
LOG=$HOME/Library/Logs/QuickTranslateSpike.log
TEXTS=(
"The new release improves startup time and reduces memory usage on older machines. Please update at your earliest convenience and report any regressions you notice."
"Our team will be offline on Friday for the company retreat. Urgent issues should be sent to the on-call engineer listed in the handbook."
"This library parses configuration files lazily, so errors only surface when a value is first accessed. Consider validating the whole file at startup instead."
"The museum extended its opening hours for the summer exhibition. Tickets can be booked online, and children under twelve enter for free."
"We measured the latency of each request and found that most of the time was spent waiting for the database. Adding an index reduced the median from 80 ms to 12 ms."
)
n=${1:-5}
for i in $(seq 1 $n); do
  mark=$(wc -l < $LOG)
  print -rn -- "${TEXTS[$i]}" | pbcopy
  for k in $(seq 1 200); do tail -n +$((mark+1)) $LOG | grep -q "RESULT_\|done painted" && tail -n +$((mark+1)) $LOG | grep -q "done painted" && break; sleep 0.05; done
  echo "=== run $i"
  tail -n +$((mark+1)) $LOG | grep -E "TRIGGER|SHOW_BEGIN|SHOW_END|start painted|turn sent|FIRST_DELTA|first delta painted|RESULT|done painted|FRONTMOST|focus" | sed -E 's/^\[[0-9]+\] up\+[0-9.]+ms//'
  id=$($(dirname $0)/winid | awk "{print \$1; exit}"); [[ -n "$SHOT" && -n "$id" ]] && screencapture -x -o -l $id "$SHOT-$i.png" && echo "shot $SHOT-$i.png"; sleep 1.5
done
