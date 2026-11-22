#!/usr/bin/env bash
# NIRUAMP installer.
#
#   ./install.sh             download the published static binary (verified with
#                            SHA256SUMS), or build with cargo when that fails
#   ./install.sh --prebuilt  only the published binary
#   ./install.sh --source    always build with cargo (also: --build)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")" && pwd)"
BIN="${HOME}/.local/bin"
APPS="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
ICONS="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/scalable/apps"
B=$'\033[1m'; G=$'\033[32m'; R=$'\033[31m'; N=$'\033[0m'

MODE="${NIRUAMP_INSTALL_MODE:-auto}"
for arg in "$@"; do
  case "$arg" in
    --prebuilt) MODE=prebuilt ;;
    --source|--build) MODE=source ;;
    -h|--help)
      printf "Usage: ./install.sh [--prebuilt|--source]\n"
      printf "  --prebuilt  Download the published static x86_64 binary (default when available)\n"
      printf "  --source    Build with cargo from this checkout (also --build)\n"
      exit 0 ;;
    *) printf "%sERROR%s unknown option: %s\n" "$R" "$N" "$arg"; exit 2 ;;
  esac
done

printf "%sNIRUAMP%s%s\n" "$B" "$N" ": graphical frontend for NPLAY"

version_at_least() { # HAVE WANT
  [ "$(printf '%s\n%s\n' "$2" "$1" | sort -V | head -1)" = "$2" ]
}

command -v nplay >/dev/null || {
  printf "%sERROR%s NPLAY is required and was not found in PATH.\n" "$R" "$N"
  printf "Install NPLAY 1.7.3 or newer: https://github.com/nirucon/nplay\n"
  printf "For a development checkout, set NPLAY_DEV_PATH when running niruamp.\n"
  [ "${NIRUAMP_SKIP_NPLAY_CHECK:-0}" = "1" ] || exit 1
}
if command -v nplay >/dev/null; then
  VERSION_NPLAY="$(nplay --version 2>/dev/null | awk '{print $2}')"
  if [ -n "$VERSION_NPLAY" ] && ! version_at_least "$VERSION_NPLAY" 1.7.3; then
    printf "%sERROR%s NPLAY %s is too old; 1.7.3 or newer is required.\n" "$R" "$N" "$VERSION_NPLAY"
    printf "Upgrade NPLAY, then run this installer again.\n"
    [ "${NIRUAMP_SKIP_NPLAY_CHECK:-0}" = "1" ] || exit 1
  fi
  printf "NPLAY %s found\n" "${VERSION_NPLAY:-unknown}"
fi

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"
ARCH="$(uname -m)"
BASE="https://github.com/nirucon/niruamp/releases/download/v${VERSION}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
BINARY=""

fetch() { # URL FILE
  if command -v curl >/dev/null; then
    curl -fsSL --retry 2 --connect-timeout 15 -o "$2" "$1"
  elif command -v wget >/dev/null; then
    wget -qO "$2" "$1"
  else
    return 1
  fi
}

download_prebuilt() {
  case "$ARCH" in x86_64|amd64) ;; *) return 1 ;; esac
  command -v sha256sum >/dev/null || return 1
  printf "Downloading prebuilt NIRUAMP %s…\n" "$VERSION"
  fetch "$BASE/niruamp" "$TMP/niruamp" || return 1
  fetch "$BASE/SHA256SUMS" "$TMP/SHA256SUMS" || return 1
  expected=$(awk '$2 == "niruamp" || $2 == "*niruamp" { print $1; exit }' "$TMP/SHA256SUMS")
  actual=$(sha256sum "$TMP/niruamp" | awk '{print $1}')
  if [ -z "$expected" ] || [ "$expected" != "$actual" ]; then
    printf "%sERROR%s the downloaded binary failed SHA-256 verification.\n" "$R" "$N"
    return 1
  fi
  printf "SHA-256 verification passed.\n"
  chmod +x "$TMP/niruamp"
  "$TMP/niruamp" --version | grep -qx "NIRUAMP $VERSION" || return 1
  BINARY="$TMP/niruamp"
}

build_from_source() {
  command -v cargo >/dev/null || {
    printf "%sERROR%s No prebuilt binary could be used and cargo is not installed.\n" "$R" "$N"
    printf "Install Rust (https://rustup.rs) or check your network.\n"
    exit 1
  }
  printf "Building release binary from source…\n"
  cd "$ROOT"
  cargo build --release --locked
  BINARY="$ROOT/target/release/niruamp"
}

if [ "$MODE" = "prebuilt" ]; then
  download_prebuilt || {
    printf "%sERROR%s No verified prebuilt binary could be downloaded.\n" "$R" "$N"
    printf "Try ./install.sh --source or check your network.\n"
    exit 1
  }
elif [ "$MODE" = "source" ]; then
  build_from_source
else
  if ! download_prebuilt; then
    printf "Prebuilt binary unavailable; falling back to a source build.\n"
    build_from_source
  fi
fi

mkdir -p "$BIN" "$APPS" "$ICONS"
# Replace the executable with an atomic rename so an already-running instance
# keeps its old inode and never reads a truncated binary.
install -m 0755 "$BINARY" "$BIN/.niruamp.new.$$"
mv -f "$BIN/.niruamp.new.$$" "$BIN/niruamp"
install -m 0644 "$ROOT/assets/niruamp.svg" "$ICONS/niruamp.svg"
sed "s|^Exec=niruamp$|Exec=$BIN/niruamp|" "$ROOT/assets/niruamp.desktop" > "$APPS/niruamp.desktop"
chmod 0755 "$APPS/niruamp.desktop"
if command -v update-desktop-database >/dev/null; then update-desktop-database "$APPS" >/dev/null 2>&1 || true; fi
if command -v gtk-update-icon-cache >/dev/null; then gtk-update-icon-cache -q "$(dirname "$(dirname "$ICONS")")" >/dev/null 2>&1 || true; fi
printf "%s✓%s Installed %s\n" "$G" "$N" "$BIN/niruamp"
printf "%s✓%s Desktop entry %s\n" "$G" "$N" "$APPS/niruamp.desktop"
printf "\nRun: niruamp\nNPLAY remains available standalone: nplay\n"
printf "Optional dependencies: cava (visualization), yt-dlp (YouTube), librespot (Spotify local playback).\n"
