#!/usr/bin/env bash
set -euo pipefail
BIN="${HOME}/.local/bin/niruamp"
APPS="${XDG_DATA_HOME:-$HOME/.local/share}/applications/niruamp.desktop"
ICON="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/scalable/apps/niruamp.svg"
rm -f "$BIN" "$APPS" "$ICON"
if [ "${1:-}" = "--purge" ]; then
  rm -rf "${XDG_CONFIG_HOME:-$HOME/.config}/niruamp" "${XDG_STATE_HOME:-$HOME/.local/state}/niruamp"
  printf "Removed NIRUAMP binary, desktop entry, preferences and window state.\n"
else
  printf "Removed NIRUAMP binary and desktop entry. Preferences, window state and all NPLAY data were preserved.\n"
fi
printf "NPLAY was not modified.\n"
