#!/usr/bin/env bash
# sched-experiment.sh — M22 measurement: does AI-aware (deprioritized) agent
# scheduling protect a latency-sensitive foreground task under CPU contention?
# Reproducible; real numbers only, no claim beyond what it measures.
set -uo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
FG="$HERE/target/release/aios-bench"
[ -x "$FG" ] || { echo "build bench first"; exit 1; }
HOGS=$(( $(nproc) - 1 )); REP=5; ITERS=120000
HOG_PIDS=()

start_hogs() { # $1 = "default" | "aware"
  HOG_PIDS=()
  for _ in $(seq "$HOGS"); do
    if [ "$1" = aware ]; then
      systemd-run --user -q --property=CPUWeight=10 --property=Nice=19 \
        -- timeout 30 sh -c 'while :; do :; done' >/dev/null 2>&1 &
    else
      timeout 30 sh -c 'while :; do :; done' >/dev/null 2>&1 &
    fi
    HOG_PIDS+=("$!")
  done
  sleep 0.4
}
stop_hogs() {
  for p in "${HOG_PIDS[@]:-}"; do kill "$p" 2>/dev/null || true; done
  # systemd-run children: stop transient units we own
  systemctl --user stop 'run-*.service' >/dev/null 2>&1 || true
  HOG_PIDS=()
  sleep 0.3
}
fg_median() {
  local xs=() t0 t1
  for _ in $(seq "$REP"); do
    t0=$(date +%s.%N); "$FG" "$ITERS" >/dev/null 2>&1; t1=$(date +%s.%N)
    xs+=("$(echo "$t1 - $t0" | bc -l)")
  done
  printf '%s\n' "${xs[@]}" | sort -n | awk '{a[NR]=$1} END{print a[int((NR+1)/2)]}'
}

echo "M22 scheduling experiment: $HOGS agent-hogs, fg=aios-bench($ITERS), reps=$REP, cores=$(nproc)"
BASE=$(fg_median); echo "baseline (idle):            ${BASE}s"
start_hogs default; DEF=$(fg_median); stop_hogs
echo "default-priority agents:    ${DEF}s"
start_hogs aware;   AIA=$(fg_median); stop_hogs
echo "AI-aware deprioritized:     ${AIA}s"
echo
echo "RESULTS (lower=better):"
printf "  idle           %ss\n" "$BASE"
printf "  default        %ss  (%.2fx vs idle)\n" "$DEF" "$(echo "$DEF/$BASE"|bc -l)"
printf "  ai-aware       %ss  (%.2fx vs idle)\n" "$AIA" "$(echo "$AIA/$BASE"|bc -l)"
printf "  fg protection  %.2fx  (default/ai-aware; >1 means AI-aware helped)\n" "$(echo "$DEF/$AIA"|bc -l)"
