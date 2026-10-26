#!/usr/bin/env bash
# Measures CPU and memory of the daemon in four states: silence (sink suspended), silent stream, music, paused.
# Usage: bench/bench.sh [seconds-per-phase] [-- daemon args…]
# CPU is the sum of every thread's on-CPU time (/proc/<pid>/task/*/schedstat, ns) over the phase,
# as % of one core. RSS / PSS / private memory come from /proc/<pid>/status and smaps_rollup.
# Audio goes to a temporary null sink (`rd-bench`), so the test is inaudible and independent of whatever the user
# is playing; the daemon captures that sink with --audio-target. The sink and the daemon are removed on exit.
# The daemon always runs under `timeout` and is killed at the end, whatever happens.
set -u
here=$(cd "$(dirname "$0")" && pwd)
bin=${BIN:-$here/../target/release/reactive-desktop}
phase=${1:-15}; shift || true
[ "${1:-}" = "--" ] && shift
work=$(mktemp -d)
wav=$work/beat.wav
python3 "$here/make-beat.py" "$wav" $((phase + 10))
python3 - "$work/zero.wav" $((phase + 6)) <<'PY'
import sys, wave
with wave.open(sys.argv[1], "wb") as w:
    w.setnchannels(2); w.setsampwidth(2); w.setframerate(48000); w.writeframes(bytes(4 * 48000 * int(sys.argv[2])))
PY
sink=rd-bench
pw-cli create-node adapter "{ factory.name=support.null-audio-sink node.name=$sink media.class=Audio/Sink object.linger=true audio.position=[FL FR] }" > /dev/null
sink_id() { pw-dump | python3 -c "import json,sys; print(next(o['id'] for o in json.load(sys.stdin) if o.get('info',{}).get('props',{}).get('node.name')=='$sink'))"; }

cputime() { # total ns of all threads
  local t=0 v
  for f in /proc/"$1"/task/*/schedstat; do read -r v _ < "$f" 2>/dev/null && t=$((t + v)); done
  echo "$t"
}
mem() { # rss pss private (kB)
  local rss pss priv
  rss=$(awk '/^VmRSS/ {print $2}' /proc/"$1"/status)
  pss=$(awk '/^Pss:/ {print $2}' /proc/"$1"/smaps_rollup)
  priv=$(awk '/^Private_(Clean|Dirty):/ {s += $2} END {print s}' /proc/"$1"/smaps_rollup)
  echo "$rss $pss $priv"
}
measure() { # label
  local c0 c1 t0 t1 maxrss=0 r
  c0=$(cputime "$pid"); t0=$(date +%s%N)
  for _ in $(seq "$phase"); do
    sleep 1
    r=$(awk '/^VmRSS/ {print $2}' /proc/"$pid"/status); [ "$r" -gt "$maxrss" ] && maxrss=$r
  done
  c1=$(cputime "$pid"); t1=$(date +%s%N)
  read -r rss pss priv < <(mem "$pid")
  awk -v l="$1" -v c=$((c1 - c0)) -v w=$((t1 - t0)) -v rss="$rss" -v m="$maxrss" -v pss="$pss" -v p="$priv" \
    'BEGIN { printf "| %-13s | %6.2f %% | %6.1f MB | %6.1f MB | %6.1f MB | %6.1f MB |\n", l, 100*c/w, rss/1024, m/1024, pss/1024, p/1024 }'
}

timeout $((phase * 4 + 50)) "$bin" --stats --audio-target "$sink" "$@" > "$work/log" 2>&1 &
tpid=$!
trap 'kill $tpid 2>/dev/null; pkill -f "pw-play.*$work" 2>/dev/null; wait; pw-cli destroy "$(sink_id)" > /dev/null 2>&1; rm -rf "$work"' EXIT
sleep 0.5
pid=$(pgrep -P "$tpid") || { echo "daemon did not start"; cat "$work/log"; exit 1; }
sleep 5.5 # startup + 2 s of quiet → idle rate
echo "| state         | CPU      | RSS      | max RSS  | PSS      | private  |"
echo "|---------------|----------|----------|----------|----------|----------|"
measure silence
pw-play --target "$sink" "$work/zero.wav" &
sleep 2
measure "silent stream"
pkill -f "pw-play.*zero.wav"
pw-play --target "$sink" "$wav" &
sleep 3
measure music
"$bin" ctl pause > /dev/null
sleep 2
measure paused
"$bin" ctl resume > /dev/null
pkill -f "pw-play.*$work"
echo
grep -E "stats:|gpu:|rendering|wallpaper|audio:" "$work/log" | sed 's/^\[[^]]*\] *//' | cut -c1-200
