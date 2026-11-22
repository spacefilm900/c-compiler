#!/usr/bin/env bash
# Run one weekly-platform check and record it.
#
#   tools/platform/record.sh [--info] NAME COMMAND [ARG...]
#
# Runs COMMAND with its output in "$RESULTS/NAME.log", prints the last
# lines, and appends one row to "$RESULTS/summary.md": the name, PASS or
# FAIL, the seconds taken and the last line of output. --info marks a
# reference-only run (another compiler's own results): it is recorded as
# INFO and never fails the step. Otherwise the exit status is COMMAND's.
# Exit status 2 means the recorder itself failed (log or summary not
# written), whatever COMMAND did.
set -uo pipefail
info=false
if [ "${1:-}" = "--info" ]; then
  info=true
  shift
fi
if [ "$#" -lt 2 ]; then
  echo "usage: $0 [--info] NAME COMMAND [ARG...]" >&2
  exit 2
fi
name=$1
shift
: "${RESULTS:?set RESULTS to the results directory}"
mkdir -p "$RESULTS"
log="$RESULTS/$name.log"
# Open the log first, so that a log that cannot be written is reported as
# the recorder's failure, not taken for COMMAND's exit status.
if ! : >"$log"; then
  echo "$0: cannot write $log" >&2
  exit 2
fi
start=$(date +%s)
"$@" >>"$log" 2>&1
rc=$?
seconds=$(($(date +%s) - start))
last=$(grep -v '^[[:space:]]*$' "$log" | tail -n 1 | cut -c1-200 | tr '|' '/' | tr -d '\r')
if [ "$info" = true ]; then
  status="INFO (exit $rc)"
elif [ "$rc" -eq 0 ]; then
  status=PASS
else
  status="FAIL (exit $rc)"
fi
if ! printf '| %s | %s | %s s | %s |\n' "$name" "$status" "$seconds" "$last" >>"$RESULTS/summary.md"; then
  echo "$0: cannot append to $RESULTS/summary.md" >&2
  tail -n 25 "$log"
  exit 2
fi
tail -n 25 "$log"
if [ "$info" = true ]; then
  exit 0
fi
exit "$rc"
