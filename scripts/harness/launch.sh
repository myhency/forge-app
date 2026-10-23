#!/usr/bin/env bash
# Builds Forge, builds a fresh fixture and starts the app against it in the background.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
run_dir="${FORGE_HARNESS_DIR:-/private/tmp/forge-harness-home}"
pid_file=/private/tmp/forge-harness.pid
log_file=/private/tmp/forge-harness.log

if [[ -f "$pid_file" ]] && kill -0 "$(cat "$pid_file")" 2>/dev/null; then
  echo "the harness app is already running (pid $(cat "$pid_file")); run stop.sh first" >&2
  exit 1
fi

# A fresh worktree has no sidecar build; a build older than the sources is rebuilt (an old one lacks new events).
if [[ ! -f "$root/sidecar/dist/index.js" ]]; then
  (cd "$root/sidecar" && npm ci && npm run build)
elif [[ -n "$(find "$root/sidecar/src" -name '*.ts' -newer "$root/sidecar/dist/index.js" | head -1)" ]]; then
  (cd "$root/sidecar" && npm run build)
fi
(cd "$root" && cargo build -p forge-app)

python3 "$here/build_fixture.py" "$run_dir" >/dev/null

# An optional script that adds to the fixture (for example add_preview_samples.py).
if [[ -n "${FORGE_HARNESS_EXTRA:-}" ]]; then
  python3 "$FORGE_HARNESS_EXTRA"
fi

binary="${CARGO_TARGET_DIR:-$root/target}/debug/forge-app"
FORGE_CLAUDE_HOME="$run_dir/.claude" \
FORGE_CONFIG_PATH="$run_dir/forge-config/config.json" \
  nohup "$binary" >"$log_file" 2>&1 &
echo $! >"$pid_file"
echo "forge-app started (pid $(cat "$pid_file")), log: $log_file; next: capture.py"
