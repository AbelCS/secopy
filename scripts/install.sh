#!/usr/bin/env bash
# Installs (or updates) the latest Secopy app into /Applications.
#
#   curl -fsSL https://raw.githubusercontent.com/AbelCS/secopy/main/scripts/install.sh | bash
#
# The app isn't notarized yet, so macOS 27 won't open a copy downloaded with a browser
# ("damaged"). Files downloaded with curl aren't flagged, so this opens normally. The
# download is checked against the release's SHA-256 before anything is installed.
# INSTALL_DIR changes where the app goes (default /Applications).
set -euo pipefail

REPO="AbelCS/secopy"
INSTALL_DIR="${INSTALL_DIR:-/Applications}"
APP="$INSTALL_DIR/Secopy.app"

fail() {
  echo "Secopy: $*" >&2
  exit 1
}

[[ "$(uname -s)" == "Darwin" && "$(uname -m)" == "arm64" ]] || fail "this app needs a Mac with Apple silicon."
if pgrep -xq secopy; then
  fail "quit Secopy first, then run this again."
fi

work="$(mktemp -d)"
mount="$work/mount"
cleanup() {
  hdiutil detach -quiet "$mount" 2>/dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT

# The latest release's dmg, from the GitHub API (no jq on a fresh Mac).
url="$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" |
  grep -o '"browser_download_url": *"[^"]*_aarch64\.dmg"' |
  sed 's/.*"\(https[^"]*\)"/\1/' | head -n 1)"
[[ -n "$url" ]] || fail "couldn't find the latest release on GitHub."
dmg="$(basename "$url")"

echo "Downloading ${dmg}…"
curl -fsSL -o "$work/$dmg" "$url"
curl -fsSL -o "$work/$dmg.sha256" "$url.sha256"
(cd "$work" && shasum -a 256 -c "$dmg.sha256" >/dev/null) || fail "the download doesn't match its checksum; nothing was installed."

hdiutil attach -quiet -nobrowse -readonly -mountpoint "$mount" "$work/$dmg"
[[ -d "$mount/Secopy.app" ]] || fail "the disk image has no Secopy.app."
mkdir -p "$INSTALL_DIR"
rm -rf "$APP"
ditto "$mount/Secopy.app" "$APP"
# Nothing above sets it, but an older copy may have kept it.
xattr -dr com.apple.quarantine "$APP" 2>/dev/null || true

version="$(defaults read "$APP/Contents/Info" CFBundleShortVersionString 2>/dev/null || echo "")"
echo "Secopy ${version:+$version }is installed in $INSTALL_DIR."
