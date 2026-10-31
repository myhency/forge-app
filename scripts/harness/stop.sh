#!/usr/bin/env bash
# Quits the harness app and removes its fixture. Safe to run when nothing is running.
set -uo pipefail

run_dir="${FORGE_HARNESS_DIR:-/private/tmp/forge-harness-home}"
pid_file=/private/tmp/forge-harness.pid

if [[ -f "$pid_file" ]]; then
  pid="$(cat "$pid_file")"
  # Only signal a live process that really is forge-app (the pid may have been recycled).
  if kill -0 "$pid" 2>/dev/null && [[ "$(ps -p "$pid" -o comm= 2>/dev/null)" == *forge-app ]]; then
    kill "$pid"
  fi
  rm -f "$pid_file"
fi

# Only directories this harness created carry the marker.
for dir in "$run_dir" /private/tmp/forge-harness; do
  if [[ -f "$dir/.forge-harness" ]]; then
    rm -rf "$dir"
  fi
done
echo "stopped"
