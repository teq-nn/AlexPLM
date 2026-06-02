#!/usr/bin/env bash
# setze-version.sh — set the Werkbank app version in every file that carries it, atomically.
#
# The version lives in FOUR places that must never drift apart:
#   app/src-tauri/tauri.conf.json   → what getVersion()/the Versionsschild shows + the bundle
#   app/src-tauri/Cargo.toml        → the Rust crate version
#   app/src-tauri/Cargo.lock        → the locked `app` package entry (else cargo rewrites it)
#   app/package.json                → the npm package version
#
# Usage: setze-version.sh X.Y.Z
# Refuses anything that is not a bare semver (no leading "v", no pre-release suffix) so the
# four files stay machine-comparable. Prints the old → new transition per file.
set -euo pipefail

NEW="${1:-}"
if [[ ! "$NEW" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "FEHLER: erwarte eine nackte Semver X.Y.Z (ohne führendes v), bekam: '$NEW'" >&2
  exit 2
fi

# Resolve the repo root from this script's location so cwd does not matter.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../../../.." && pwd)"

TAURI="$ROOT/app/src-tauri/tauri.conf.json"
CARGO_TOML="$ROOT/app/src-tauri/Cargo.toml"
CARGO_LOCK="$ROOT/app/src-tauri/Cargo.lock"
PKG="$ROOT/app/package.json"

for f in "$TAURI" "$CARGO_TOML" "$CARGO_LOCK" "$PKG"; do
  [[ -f "$f" ]] || { echo "FEHLER: nicht gefunden: $f" >&2; exit 1; }
done

# Read the current version from tauri.conf.json (the displayed source of truth) for reporting.
OLD="$(grep -m1 '"version"' "$TAURI" | sed -E 's/.*"version"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/')"
echo "Werkbank: $OLD → $NEW"

# tauri.conf.json: the first top-level "version": "..." line.
sed -i -E '0,/"version"[[:space:]]*:[[:space:]]*"[^"]+"/s//"version": "'"$NEW"'"/' "$TAURI"

# package.json: likewise its first "version": "..." line.
sed -i -E '0,/"version"[[:space:]]*:[[:space:]]*"[^"]+"/s//"version": "'"$NEW"'"/' "$PKG"

# Cargo.toml: the version line inside the [package] table (the first `version = "..."`).
sed -i -E '0,/^version[[:space:]]*=[[:space:]]*"[^"]+"/s//version = "'"$NEW"'"/' "$CARGO_TOML"

# Cargo.lock: only the `app` package block. Find `name = "app"` then bump the next version line.
sed -i -E '/^name = "app"$/{n;s/^version = "[^"]+"/version = "'"$NEW"'"/}' "$CARGO_LOCK"

echo "Gesetzt in: tauri.conf.json, Cargo.toml, Cargo.lock, package.json"
echo "Prüfen mit: grep -rn '\"version\"\\|^version' \"$TAURI\" \"$PKG\" \"$CARGO_TOML\""
