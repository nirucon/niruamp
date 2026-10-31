#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")" && pwd)"
BIN="${HOME}/.local/bin"
APPS="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
ICONS="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/scalable/apps"
B=$'\033[1m'; G=$'\033[32m'; Y=$'\033[33m'; R=$'\033[31m'; N=$'\033[0m'

MODE="${NIRUAMP_INSTALL_MODE:-auto}"
for arg in "$@"; do
  case "$arg" in
    --prebuilt) MODE=prebuilt ;;
    --source) MODE=source ;;
    -h|--help)
      printf "Usage: ./install.sh [--prebuilt|--source]\n"
      printf "  --prebuilt  Download the published x86_64 Linux binary (default when available)\n"
      printf "  --source    Build with cargo from this checkout\n"
      exit 0 ;;
    *) printf "%sERROR%s unknown option: %s\n" "$R" "$N" "$arg"; exit 2 ;;
  esac
done

printf "%sNIRUAMP%s%s\n" "$B" "$N" " — Rust graphical frontend for NPLAY"
command -v nplay >/dev/null || {
  printf "%sERROR%s NPLAY is required and was not found in PATH.\n" "$R" "$N"
  printf "Install NPLAY 1.7.0 or newer: https://github.com/nirucon/nplay\n"
  printf "For a development checkout, set NPLAY_DEV_PATH when running niruamp.\n"
  [ "${NIRUAMP_SKIP_NPLAY_CHECK:-0}" = "1" ] || exit 1
}
if command -v nplay >/dev/null; then
  VERSION_NPLAY="$(nplay --version 2>/dev/null | awk '{print $2}')"
  if [ -n "$VERSION_NPLAY" ] && ! python3 - "$VERSION_NPLAY" <<'PY'
import sys
v=sys.argv[1].split('.')
try: ok=(int(v[0]),int(v[1]),int(v[2] if len(v)>2 else 0)) >= (1,7,0)
except Exception: ok=False
sys.exit(0 if ok else 1)
PY
  then
    printf "%sERROR%s NPLAY %s is too old; 1.7.0 or newer is required.\n" "$R" "$N" "$VERSION_NPLAY"
    printf "Upgrade NPLAY, then run this installer again.\n"
    [ "${NIRUAMP_SKIP_NPLAY_CHECK:-0}" = "1" ] || exit 1
  fi
  printf "NPLAY %s found\n" "${VERSION_NPLAY:-unknown}"
fi

if ! ldconfig -p 2>/dev/null | grep -q 'libxkbcommon-x11.so.0'; then
  printf "%sWARNING%s libxkbcommon-x11 is not visible. If the GUI fails to start:\n" "$Y" "$N"
  printf "  sudo apt install libxkbcommon-x11-0 libx11-6 libgl1 libegl1\n"
fi

cargo_version_ok() {
  command -v cargo >/dev/null || return 1
  python3 - "$(cargo --version 2>/dev/null | awk '{print $2}')" <<'PY'
import sys
v=sys.argv[1].split('.')
try: ok=(int(v[0]),int(v[1])) >= (1,75)
except Exception: ok=False
sys.exit(0 if ok else 1)
PY
}

glibc_ok() {
  command -v getconf >/dev/null || return 1
  python3 - "$(getconf GNU_LIBC_VERSION 2>/dev/null | awk '{print $2}')" <<'PY'
import sys
v=sys.argv[1].split('.')
try: ok=(int(v[0]),int(v[1])) >= (2,36)
except Exception: ok=False
sys.exit(0 if ok else 1)
PY
}

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"
ARCH="$(uname -m)"
ASSET=""
case "$ARCH" in
  x86_64|amd64) ASSET="niruamp-${VERSION}-x86_64-linux-gnu.tar.gz" ;;
esac
URL="https://github.com/nirucon/niruamp/releases/download/v${VERSION}/${ASSET}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
BINARY=""

download_prebuilt() {
  [ -n "$ASSET" ] || return 1
  glibc_ok || return 1
  command -v curl >/dev/null || return 1
  printf "Downloading prebuilt %s…\n" "$ASSET"
  curl -fL --retry 2 --connect-timeout 15 -o "$TMP/$ASSET" "$URL" >/dev/null 2>&1 || return 1
  tar -xzf "$TMP/$ASSET" -C "$TMP" || return 1
  if [ -f "$TMP/niruamp" ]; then
    BINARY="$TMP/niruamp"
    return 0
  fi
  return 1
}

build_from_source() {
  if ! cargo_version_ok; then
    printf "%sERROR%s No prebuilt binary is available for this architecture/glibc,\n" "$R" "$N"
    printf "and cargo 1.75+ is required to build from source.\n"
    printf "Install Rust: https://rustup.rs\n"
    exit 1
  fi
  printf "Building release binary from source…\n"
  cd "$ROOT"
  cargo build --release --locked
  BINARY="$ROOT/target/release/niruamp"
}

if [ "$MODE" = "prebuilt" ]; then
  download_prebuilt || {
    printf "%sERROR%s No compatible prebuilt binary could be downloaded.\n" "$R" "$N"
    printf "Try ./install.sh --source or check your network/glibc version.\n"
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
install -m 0755 "$BINARY" "$BIN/niruamp"
install -m 0644 "$ROOT/assets/niruamp.svg" "$ICONS/niruamp.svg"
sed "s|^Exec=niruamp$|Exec=$BIN/niruamp|" "$ROOT/assets/niruamp.desktop" > "$APPS/niruamp.desktop"
chmod 0755 "$APPS/niruamp.desktop"
if command -v update-desktop-database >/dev/null; then update-desktop-database "$APPS" >/dev/null 2>&1 || true; fi
if command -v gtk-update-icon-cache >/dev/null; then gtk-update-icon-cache -q "$(dirname "$(dirname "$ICONS")")" >/dev/null 2>&1 || true; fi
printf "%s✓%s Installed %s\n" "$G" "$N" "$BIN/niruamp"
printf "%s✓%s Desktop entry %s\n" "$G" "$N" "$APPS/niruamp.desktop"
printf "\nRun: niruamp\nNPLAY remains available standalone: nplay\n"
printf "Optional dependencies: cava (visualization), yt-dlp (YouTube), librespot (Spotify local playback).\n"
