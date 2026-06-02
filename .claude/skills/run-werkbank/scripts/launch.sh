#!/usr/bin/env bash
# Launch the Werkbank Tauri dev app and wait until it reports ready (or fails).
# Backgrounds `npm run tauri dev` via nohup, polls its log, then returns leaving
# the app running. Re-running first stops any prior instance (idempotent).
#
# Exit 0 + "READY"  → the Rust app launched (window is up).
# Exit 1 + "FAILED" → a cargo/build error; the tail of the log is printed.
set -u

# Resolve the repo's app/ dir from this script's location (…/.claude/skills/run-werkbank/scripts).
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../../.." && pwd)"
APP_DIR="$REPO_ROOT/app"
LOG=/tmp/werkbank-dev.log
TIMEOUT_S=240

"$SCRIPT_DIR/stop.sh" >/dev/null 2>&1 || true

if [ ! -f "$APP_DIR/package.json" ]; then
  echo "FAILED: app/ not found at $APP_DIR" >&2
  exit 1
fi

: > "$LOG"
# nohup so the dev server outlives this script; the window must persist for the human check.
( cd "$APP_DIR" && nohup npm run tauri dev >>"$LOG" 2>&1 & )
echo "launched: npm run tauri dev  (log: $LOG)"

# Poll the log. The clean "Rust app started" marker is the Diagnose-log line printed
# on setup; build failure shows a cargo `error` line. ANSI codes are stripped for matching.
elapsed=0
while [ "$elapsed" -lt "$TIMEOUT_S" ]; do
  clean="$(sed 's/\x1b\[[0-9;]*m//g' "$LOG" 2>/dev/null)"
  if printf '%s' "$clean" | grep -q 'Git-Diagnose-Log:'; then
    echo "READY after ${elapsed}s — app binary running"
    pgrep -af 'target/debug/app' | grep -v pgrep || true
    exit 0
  fi
  if printf '%s' "$clean" | grep -Eq '^error(\[|:)|error: could not compile|cannot find'; then
    echo "FAILED — build error:" >&2
    printf '%s\n' "$clean" | grep -Ei 'error' | tail -15 >&2
    exit 1
  fi
  sleep 3
  elapsed=$((elapsed + 3))
done

echo "FAILED — no ready signal within ${TIMEOUT_S}s; last log lines:" >&2
tail -20 "$LOG" >&2
exit 1
