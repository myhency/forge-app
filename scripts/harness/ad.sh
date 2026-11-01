#!/usr/bin/env bash
# agent-desktop (npm i -g agent-desktop) aimed at the harness app's window only.
#   ad.sh snapshot            tree of interactive elements, refs like @sXXXX:e7
#   ad.sh click @sXXXX:e7     press a ref (headless: no cursor, no focus steal, works off-screen)
#   ad.sh <any agent-desktop command>   passed through; refs belong to the snapshot that made them
set -euo pipefail

pid_file=/private/tmp/forge-harness.pid
if [[ ! -f "$pid_file" ]] || ! kill -0 "$(cat "$pid_file")" 2>/dev/null; then
  echo "the harness app is not running; run launch.sh first" >&2
  exit 1
fi
pid="$(cat "$pid_file")"

# The visible window of the harness pid ("Forge"); the other one is a hidden helper.
win="$(agent-desktop list-windows | python3 -c "
import sys, json
pid = int(sys.argv[1])
ws = [w for w in json.load(sys.stdin)['data'] if w['pid'] == pid and w['visible'] and w['accessible']]
print(ws[0]['id'] if ws else '')" "$pid")"
[[ -n "$win" ]] || { echo "no visible window for pid $pid" >&2; exit 1; }

cmd="${1:?usage: ad.sh <command> [args]}"
shift
if [[ "$cmd" == snapshot ]]; then
  exec agent-desktop snapshot --window-id "$win" -i "$@"
fi
# Key presses go to a process, so they must name the harness window; a second Forge would otherwise match by name.
if [[ "$cmd" == press ]]; then
  exec agent-desktop press --window-id "$win" "$@"
fi
exec agent-desktop "$cmd" "$@"
