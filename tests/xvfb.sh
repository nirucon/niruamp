#!/bin/sh
# X11 end-to-end checks in a private Xvfb with a sandboxed NPLAY service:
# first frame, memory, idle CPU, transport keys, the context menu, the queue,
# windowshade, search, a settings check box, dragging the window, pausing the
# spectrum while unmapped, and closing.
#
# Usage: tests/xvfb.sh PATH/TO/niruamp [NPLAY-COMMAND]
#   NPLAY-COMMAND defaults to `nplay` (any NPLAY 1.8+ with --serve).
#   NIRUAMP_TEST_DISPLAY picks the display number (default 64).
set -eu
BIN=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
NPLAY=${2:-nplay}
HERE=$(cd "$(dirname "$0")/.." && pwd)
SB=$(mktemp -d)
PIDS=""
cleanup() {
  for p in $PIDS; do kill "$p" 2>/dev/null || true; done
  sleep 0.3
  for p in $PIDS; do kill -9 "$p" 2>/dev/null || true; done
  rm -rf "$SB"
}
trap cleanup EXIT INT TERM
fail() { echo "FAIL: $*" >&2; [ -f "$SB/amp.err" ] && tail -5 "$SB/amp.err" >&2; exit 1; }

export HOME="$SB/home" XDG_CONFIG_HOME="$SB/config" XDG_DATA_HOME="$SB/data" \
  XDG_STATE_HOME="$SB/state" XDG_CACHE_HOME="$SB/cache" XDG_RUNTIME_DIR="$SB/run"
mkdir -p "$HOME/Music/Artist/Album" "$HOME/Music/Band/Second" "$SB/bin" "$XDG_RUNTIME_DIR" \
  "$XDG_CONFIG_HOME/nplay" "$XDG_CONFIG_HOME/niruamp"
chmod 700 "$XDG_RUNTIME_DIR"
unset WAYLAND_DISPLAY DBUS_SESSION_BUS_ADDRESS HYPRLAND_INSTANCE_SIGNATURE || true
ln -s "$HERE/tools/fake-mpv" "$SB/bin/mpv"
ln -s "$HERE/tools/fake-cava" "$SB/bin/cava"
export PATH="$SB/bin:$PATH"
python3 - "$HOME/Music" <<'PY'
import sys, wave
for d, n in (("Artist/Album", 3), ("Band/Second", 12)):
    for i in range(n):
        w = wave.open(f"{sys.argv[1]}/{d}/{i+1:02d} Song {i+1}.wav", "wb")
        w.setnchannels(1); w.setsampwidth(2); w.setframerate(8000)
        w.writeframes(b"\0\0" * 8000 * 120); w.close()
PY
# Offline sources only: the test never touches the network.
cat > "$XDG_CONFIG_HOME/nplay/config.ini" <<EOF
[general]
music_dirs = $HOME/Music
youtube_enabled = false
sr_enabled = false
custom_radio_enabled = false
navidrome_enabled = false
statistics_enabled = false
track_notifications = false
mpris_enabled = false
EOF
printf 'startup_sound = false\ntray_enabled = false\n' > "$XDG_CONFIG_HOME/niruamp/config.toml"

D=:${NIRUAMP_TEST_DISPLAY:-64}
Xvfb "$D" -screen 0 1280x900x24 -nolisten tcp >/dev/null 2>&1 &
PIDS="$PIDS $!"
export DISPLAY=$D
sleep 1

$NPLAY --scan >/dev/null 2>&1 || true
SOCK="$XDG_RUNTIME_DIR/nplay/control.sock"
$NPLAY --serve --service-socket "$SOCK" >"$SB/serve.log" 2>&1 &
SRV=$!
PIDS="$PIDS $SRV"
i=0; while [ ! -S "$SOCK" ] && [ $i -lt 100 ]; do sleep 0.1; i=$((i + 1)); done
[ -S "$SOCK" ] || fail "NPLAY service did not start: $(tail -3 "$SB/serve.log")"
export NPLAY_SERVICE_SOCKET="$SOCK"

rpc() {
  python3 - "$SOCK" "$@" <<'PY'
import json, socket, sys
s = socket.socket(socket.AF_UNIX); s.connect(sys.argv[1]); f = s.makefile("rw")
def call(m, p, i):
    f.write(json.dumps({"jsonrpc": "2.0", "id": i, "method": m, "params": p}) + "\n"); f.flush()
    while True:
        r = json.loads(f.readline())
        if r.get("id") == i:
            return r.get("result", r.get("error"))
call("initialize", {"client": {"name": "xvfb-test", "version": "1"}, "protocol_versions": [1]}, 1)
m = sys.argv[2]
if m == "play":
    items = call("browse", {"source": "local", "section": "all", "limit": 50}, 2)["items"]
    tr = [t for t in items if t.get("kind") == "track"]
    call("playback.play", {"track": tr[0], "context": tr, "index": 0}, 3)
elif m == "queue_first":
    q = call("state.snapshot", {}, 2)["playback"]["queue"]
    print(q[0]["key"] if q else "-")
elif m == "state":
    st = call("state.snapshot", {}, 2)["playback"]
    cur = (st.get("current") or {}).get("title") or "-"
    print(st["status"], len(st["queue"]), int(st["volume"]), cur.replace(" ", "_"))
PY
}
state() { rpc state; }
wait_state() { # FIELD-INDEX EXPECTED
  n=0
  while [ $n -lt 40 ]; do
    v=$(state | awk -v f="$1" '{print $f}')
    [ "$v" = "$2" ] && return 0
    sleep 0.15; n=$((n + 1))
  done
  fail "expected field $1 = $2, got $(state)"
}
cpu() { awk '{print $14 + $15}' "/proc/$1/stat"; }
pct() { a=$(cpu "$1"); sleep "$2"; b=$(cpu "$1"); awk -v a="$a" -v b="$b" -v s="$2" -v h="$(getconf CLK_TCK)" 'BEGIN { printf "%.2f", (b - a) * 100 / h / s }'; }

NIRUAMP_TIMING=1 "$BIN" 2>"$SB/amp.err" &
AMP=$!
PIDS="$AMP $PIDS"
i=0; while ! grep -q 'first frame after' "$SB/amp.err" && [ $i -lt 50 ]; do sleep 0.1; i=$((i + 1)); done
ms=$(sed -n 's/.*first frame after \([0-9]*\)\..*/\1/p' "$SB/amp.err" | head -1)
[ -n "$ms" ] || fail "no first frame"
[ "$ms" -lt 1000 ] || fail "first frame took $ms ms"
W=$(xdotool search --sync --onlyvisible --pid "$AMP" | head -1)
[ -n "$W" ] || fail "no window"
xdotool search --classname niruamp | grep -qx "$W" || fail "WM_CLASS instance is not niruamp"
sleep 2
rss=$(awk '/VmRSS/ {print $2}' "/proc/$AMP/status")
[ "$rss" -lt 20000 ] || fail "memory $rss kB"
idle=$(pct "$AMP" 4)
awk -v c="$idle" 'BEGIN { exit !(c < 0.5) }' || fail "idle CPU $idle %"
echo "first frame ${ms} ms, ${rss} kB, idle ${idle} %"

k() { xdotool key --window "$W" --delay 40 "$@"; sleep 0.4; }
rpc play
wait_state 1 playing
sleep 1.5
playing=$(pct "$AMP" 4)
echo "playing with the spectrum: ${playing} % CPU, $(awk '/VmRSS/ {print $2}' "/proc/$AMP/status") kB"
k space; wait_state 1 paused
k space; wait_state 1 playing
k n; wait_state 4 02_Song_2
k plus; wait_state 3 105
k minus; wait_state 3 100
echo "transport keys ok"

# Right-click the third playlist row, "Add to queue" (third menu item).
xdotool mousemove 150 370 click 3; sleep 0.4
xdotool mousemove 190 428 click 1; sleep 0.5
wait_state 2 1
k 2 Delete
wait_state 2 0
echo "context menu and queue ok"

# Drag the second queue row above the first: the queue is reordered.
k 1
xdotool mousemove 150 370 click 3; sleep 0.3; xdotool mousemove 190 428 click 1; sleep 0.4
xdotool mousemove 150 397 click 3; sleep 0.3; xdotool mousemove 190 455 click 1; sleep 0.4
wait_state 2 2
k 2
first=$(rpc queue_first)
xdotool mousemove 150 343 mousedown 1
for y in 340 336 330 325 318 314; do xdotool mousemove 150 "$y"; sleep 0.08; done
xdotool mouseup 1; sleep 0.6
[ "$(rpc queue_first)" != "$first" ] || fail "queue drag did not reorder"
k Delete Delete
wait_state 2 0
k 1
echo "queue drag ok"

# A second launch activates the running window and exits.
"$BIN" 2>/dev/null &
SECOND=$!
i=0; while kill -0 "$SECOND" 2>/dev/null && [ $i -lt 30 ]; do sleep 0.1; i=$((i + 1)); done
kill -0 "$SECOND" 2>/dev/null && fail "second instance kept running"
[ "$(xdotool search --classname niruamp | wc -l)" = 1 ] || fail "two windows"
echo "single instance ok"

k w
h=$(xwininfo -id "$W" | awk '/Height:/ {print $2}')
[ "$h" = 58 ] || fail "windowshade height $h"
k w
h=$(xwininfo -id "$W" | awk '/Height:/ {print $2}')
[ "$h" -gt 300 ] || fail "unshade height $h"
echo "windowshade ok"

# Search the local library and play the first hit.
k 1 slash
xdotool type --window "$W" --delay 40 "Song 12"; sleep 0.3
k Return
sleep 1.5
k Return
wait_state 4 12_Song_12
echo "search ok"

# Settings: "Show remaining time" is saved.
k 4
xdotool mousemove 230 390 click 1; sleep 0.6
grep -q 'remaining_time = true' "$XDG_CONFIG_HOME/niruamp/config.toml" || fail "setting not saved"
echo "settings ok"

# Drag the title strip: the window follows (no window manager here).
xdotool mousemove 300 20; sleep 0.3; xdotool mousedown 1; sleep 0.1
for x in 320 350 380 400; do xdotool mousemove "$x" $((x - 280)); sleep 0.1; done
xdotool mouseup 1; sleep 0.5
xw=$(xwininfo -id "$W" | awk '/Absolute upper-left X:/ {print $4}')
[ "$xw" -ge 90 ] || fail "window did not move (x=$xw)"
echo "window drag ok"

# Unmapped (another workspace): the spectrum and the frames stop.
xdotool windowunmap "$W"; sleep 1
hidden=$(pct "$AMP" 3)
awk -v c="$hidden" 'BEGIN { exit !(c < 0.5) }' || fail "unmapped CPU $hidden %"
xdotool windowmap "$W"; sleep 0.6
echo "unmapped: ${hidden} % CPU"

# Close: the process exits and keeps its window state.
xdotool mousemove 0 0; sleep 0.2
xw=$(xwininfo -id "$W" | awk '/Absolute upper-left X:/ {print $4}')
yw=$(xwininfo -id "$W" | awk '/Absolute upper-left Y:/ {print $4}')
wd=$(xwininfo -id "$W" | awk '/Width:/ {print $2}')
xdotool mousemove $((xw + wd - 24)) $((yw + 25)) click 1
i=0; while kill -0 "$AMP" 2>/dev/null && [ $i -lt 30 ]; do sleep 0.1; i=$((i + 1)); done
kill -0 "$AMP" 2>/dev/null && fail "close button did not quit"
[ -f "$XDG_STATE_HOME/niruamp/window.toml" ] || fail "window state not saved"
echo "close ok"
echo "PASS"
