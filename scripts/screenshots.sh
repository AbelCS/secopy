#!/usr/bin/env bash
# Remakes the user guide's screenshots (docs/images/) from the UI gallery, in English, at 2×.
#
#   scripts/screenshots.sh            # needs Google Chrome and `npm ci` done in ui/
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
chrome="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
port=5199
[[ -x "$chrome" ]] || { echo "screenshots: Google Chrome not found (set CHROME)" >&2; exit 1; }

(cd "$root/ui" && npx vite --port "$port" --strictPort >/dev/null 2>&1) &
vite=$!
trap 'kill "$vite" 2>/dev/null; pkill -f "vite --port $port" 2>/dev/null || true' EXIT
for _ in $(seq 50); do curl -fs "http://localhost:$port/gallery.html" >/dev/null && break; sleep 0.2; done

shot() { # name window-size
  local profile
  profile="$(mktemp -d)"
  local out="$root/docs/images/$1.png" before=""
  [[ -f "$out" ]] && before="$(stat -f %m "$out")"
  # Headless Chrome writes the screenshot but doesn't always exit: wait for it, then stop it.
  "$chrome" --headless=new --lang=en-US --accept-lang=en-US --user-data-dir="$profile" \
    --disable-gpu --hide-scrollbars --force-device-scale-factor=2 --window-size="$2" \
    --virtual-time-budget=5000 --screenshot="$out" \
    "http://localhost:$port/gallery.html#$1" >/dev/null 2>&1 &
  local pid=$!
  for _ in $(seq 100); do
    [[ -f "$out" && "$(stat -f %m "$out")" != "$before" ]] && break
    sleep 0.2
  done
  sleep 1
  kill "$pid" 2>/dev/null || true
  pkill -f -- "--user-data-dir=$profile" 2>/dev/null || true
  rm -rf "$profile"
  echo "docs/images/$1.png"
}

for page in setup progress summary mirror-preview verify-summary queue import; do
  shot "$page" 960,720
done
shot panel 390,240
