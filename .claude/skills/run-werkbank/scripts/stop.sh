#!/usr/bin/env bash
# Stop a running Werkbank dev session: the app window, the cargo runner, and vite.
# Idempotent — prints what was stopped, exits 0 even if nothing was running.
set -u

stopped=0
for pat in 'target/debug/app' 'cargo  run --no-default-features' 'tauri dev' 'vite dev'; do
  if pgrep -f "$pat" >/dev/null 2>&1; then
    pkill -f "$pat" 2>/dev/null || true
    stopped=1
  fi
done

sleep 1
if pgrep -af 'target/debug/app|vite dev' | grep -qv pgrep; then
  echo "WARN: some processes still alive:" >&2
  pgrep -af 'target/debug/app|vite dev' | grep -v pgrep >&2
  exit 0
fi

[ "$stopped" -eq 1 ] && echo "dev session stopped" || echo "nothing was running"
exit 0
