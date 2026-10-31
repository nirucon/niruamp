# Testing NIRUAMP

## Automated suites

```sh
# Frontend
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release --locked

# Backend (NPLAY repository)
python3 -m compileall -q nplay
python3 -m unittest discover -s tests -v
```

The Rust suite covers wire-model invariants, time formatting, PNG artwork
decoding, standalone/builtin/custom/sparse appearance resolution, light-theme
contrast derivation, malformed appearance recovery, malformed custom-theme
rejection and live debounced theme reload.

The NPLAY suite covers the existing player/providers/statistics/theme tests plus
the control service: handshake and capability discovery, protocol rejection,
snapshot/event ordering, bulk queueing, playlists, external queueing, artwork
redaction, invalid/oversized frames, socket/directory permissions, second
service behaviour, graceful shutdown, idle exit, and local playback through the
real NPLAY `Player` code with a deterministic mpv IPC harness.

## Deterministic transport harness

`tools/fake-mpv` implements the subset of mpv JSON IPC NPLAY uses. It does not
decode audio and is not a player. It exercises the real NPLAY
`MpvStartCoordinator`/`Player` boundary and lets CI verify `loading → playing →
paused → seek → stopped`, position advancement and UI state without audio
hardware.

```sh
mkdir -p /tmp/niruamp-test-bin
ln -sf "$PWD/tools/fake-mpv" /tmp/niruamp-test-bin/mpv
PATH=/tmp/niruamp-test-bin:$PATH NPLAY_DEV_PATH=/path/to/nplay niruamp
```

Run the backend playback integration test alone:

```sh
# from the NPLAY checkout
python3 -m unittest tests.test_service.ServiceTests.test_local_playback_through_real_mpv_boundary_with_fake_ipc -v
```

`tools/fake-cava` is a synthetic bar-output harness used **only** to measure the
cost of the visualization drawing/polling path. It is not audio capture and its
output must not be presented as real CAVA visualization.

## Manual desktop checks

```sh
# Build/run under Xvfb when no desktop session is available.
sudo apt-get install -y xvfb xdotool ffmpeg   # CI/dev machine
Xvfb :98 -screen 0 1280x900x24 &
DISPLAY=:98 NPLAY_DEV_PATH=/path/to/nplay target/release/niruamp
```

Useful checks:

1. Launch from the installed desktop entry (or `~/.local/bin/niruamp`).
2. Search `/`, type a query, verify results appear asynchronously and late
   responses do not replace a newer query.
3. Queue a group, switch to Queue, remove/move/reorder and clear.
4. Right-click a track and add it to a playlist; create/rename/delete playlists.
5. Toggle windowshade (`W`), always-on-top (`T`), resize and restart to verify
   geometry persistence.
6. Change the NIRUX theme in `~/.config/niruwm/config.toml` while running;
   verify live apply. Repeat with a sparse custom theme, malformed
   `appearance.toml`, and the standalone `classic` setting.
7. Stop NPLAY (`kill` the service) and verify the connection-lost screen;
   press Retry and verify a fresh service connects.
8. Launch `niruamp` a second time; verify the first window is activated and no
   second audio engine starts.
9. Drag a local media file/stream URL onto the window; verify it plays/queues.
10. Test at 800×600, 1366×768, 1920×1080 and a scaled display; confirm no
    clipped controls. The window is clamped once at startup to the monitor work
    area with a small margin.

Screenshots can be captured by the running release build:

```sh
NIRUAMP_SCREENSHOT=/tmp/niruamp.png NIRUAMP_SCREENSHOT_DELAY=3 \
  target/release/niruamp
```

`NIRUAMP_SCREENSHOT_DELAY` is a development aid; the screenshot request is
processed by egui's viewport screenshot API and saved as PNG.

## Lifecycle matrix

| Scenario | Expected | Automated / manual coverage |
|---|---|---|
| Backend absent, installed `nplay` | Launch compatible service, connect | Covered by service tests and release UI run |
| Backend absent, `nplay` missing | Graphical dependency screen with remedy | Covered manually; screenshot in validation report |
| Backend present | Attach without spawning another engine | Covered by second-service test |
| GUI start/exit | Preferences/geometry saved; owned service stopped unless keep-playing | Manual GUI exit; service shutdown test |
| GUI crash | Owned service exits after `--idle-exit` | Service idle-exit test |
| Backend crash/restart | Actionable error; Retry reconnects | Manual kill/Retry; service tests |
| Two GUI launches | Second activates first; one player | Manual; single-instance socket |
| Terminal NPLAY alone | Fully standalone | Full NPLAY suite passes without Rust/GUI |
| Terminal + GUI simultaneously | One authoritative state via the service | Protocol design; terminal attach is a documented supported client |
| Slow/dead client | Bounded queues; playback unaffected | Service queue/outstanding bounds; no playback stall test under load yet |
| Stale socket | Replaced after lock/ownership check | Service test for live endpoint; stale path handled in code |

## Resource measurement

```sh
scripts/measure.py 45 <frontend-pid> <backend-pid> [<helper-pid>...]
```

The script samples `/proc/PID/stat` for CPU ticks and `/proc/PID/status` for
peak RSS. Measure a sustained interval, not one sample. On the validation host
the display was Xvfb with llvmpipe software rendering; hardware GPU cost will be
lower. CAVA/mpv harnesses are called out explicitly in the report.

## Not covered by automation

- Real mpv audio output, real CAVA audio capture, PulseAudio/PipeWire sessions.
- Live Navidrome, Sveriges Radio, YouTube, Spotify and custom stream accounts.
- A real NIRUX/NIRUWM session, mixed-DPI monitors, WM tiling rules or tray
  behaviour (there is no tray icon).
- Actual desktop-entry activation by a desktop environment; the installed
  entry and release binary were verified separately.

These are stated as not live-verified rather than claimed as passing.
