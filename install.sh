#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")" && pwd)"
BIN="${HOME}/.local/bin"
APPS="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
ICONS="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/scalable/apps"
B=$'\033[1m'; G=$'\033[32m'; Y=$'\033[33m'; R=$'\033[31m'; N=$'\033[0m'

printf "%sNIRUAMP%s%s\n" "$B" "$N" " — Rust graphical frontend for NPLAY"
command -v cargo >/dev/null || { printf "%sERROR%s cargo is required (Rust 1.75+).\n" "$R" "$N"; exit 1; }
command -v nplay >/dev/null || {
  printf "%sERROR%s NPLAY is required and was not found in PATH.\n" "$R" "$N"
  printf "Install NPLAY 1.7.0 or newer: https://github.com/nirucon/nplay\n"
  printf "For a development checkout, set NPLAY_DEV_PATH when running niruamp.\n"
  [ "${NIRUAMP_SKIP_NPLAY_CHECK:-0}" = "1" ] || exit 1
}
if command -v nplay >/dev/null; then
  VERSION="$(nplay --version 2>/dev/null | awk '{print $2}')"
  if [ -n "$VERSION" ] && ! python3 - "$VERSION" <<'PY'
import sys
v=sys.argv[1].split('.')
try: ok=(int(v[0]),int(v[1]),int(v[2] if len(v)>2 else 0)) >= (1,7,0)
except Exception: ok=False
sys.exit(0 if ok else 1)
PY
  then
    printf "%sERROR%s NPLAY %s is too old; 1.7.0 or newer is required.\n" "$R" "$N" "$VERSION"
    printf "Upgrade NPLAY, then run this installer again.\n"
    [ "${NIRUAMP_SKIP_NPLAY_CHECK:-0}" = "1" ] || exit 1
  fi
  printf "NPLAY %s found\n" "${VERSION:-unknown}"
fi

printf "Building release binary…\n"
cd "$ROOT"
cargo build --release --locked
mkdir -p "$BIN" "$APPS" "$ICONS"
install -m 0755 "$ROOT/target/release/niruamp" "$BIN/niruamp"
install -m 0644 "$ROOT/assets/niruamp.svg" "$ICONS/niruamp.svg"
sed "s|^Exec=niruamp$|Exec=$BIN/niruamp|" "$ROOT/assets/niruamp.desktop" > "$APPS/niruamp.desktop"
chmod 0755 "$APPS/niruamp.desktop"
if command -v update-desktop-database >/dev/null; then update-desktop-database "$APPS" >/dev/null 2>&1 || true; fi
if command -v gtk-update-icon-cache >/dev/null; then gtk-update-icon-cache -q "$(dirname "$(dirname "$ICONS")")" >/dev/null 2>&1 || true; fi
printf "%s✓%s Installed %s\n" "$G" "$N" "$BIN/niruamp"
printf "%s✓%s Desktop entry %s\n" "$G" "$N" "$APPS/niruamp.desktop"
printf "\nRun: niruamp\nNPLAY remains available standalone: nplay\n"
printf "Optional dependencies: cava (visualization), yt-dlp (YouTube), librespot (Spotify local playback).\n"
