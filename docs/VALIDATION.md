# NIRUAMP validation report

Date: 2026-10-31
Frontend: NIRUAMP 0.1.0, commit `8b9d9aa` (Rust, `eframe`/`egui` 0.29.1, Glow)
Backend: NPLAY 1.7.0, control protocol 1
Target: NIRUX / X11 and non-NIRUX Linux desktops

## Test machine

| Item | Value |
|---|---|
| Host | `yggdrasil` |
| OS | Ubuntu 24.04.5 LTS, kernel 6.8.0-147-generic, x86_64 |
| CPU | 11th Gen Intel Core i7-1165G7, 8 logical CPUs |
| RAM | 15 GiB |
| Display used | Xvfb 1280×900×24 (also 800×600, 1366×768, 1600×1200 at 1.5×, 1920×1080) |
| Session / WM | X11 under Xvfb, no window manager |
| Renderer | Glow over software GL (llvmpipe); no hardware GLX session was available |
| Rust | rustc/cargo 1.99.1 |
| Python | 3.12.3 |
| NPLAY revision | `852050b` (release: NPLAY 1.7.0 headless control service) |

The owner did not provide external bare-metal/SSH test machines during this
implementation pass. No remote testing, audio device testing, GPU testing or
credentials were fabricated. All results below are from the local host.

## Automated tests

| Suite | Result |
|---|---|
| Rust format (`cargo fmt --all -- --check`) | pass |
| Rust clippy (`cargo clippy --all-targets -- -D warnings`) | pass |
| Rust unit tests (`cargo test`) | **16 passed** |
| Rust release build (`cargo build --release --locked`) | pass |
| NPLAY full Python suite (`python3 -m unittest discover -s tests`) | **80 passed** (68 pre-existing + 12 service) |
| NPLAY service tests | handshake, events, playlists, queue, bulk queue, external queueing, artwork redaction, malformed/oversized frames, permissions, second service, graceful shutdown, idle exit, local playback via real `Player` boundary with mpv IPC harness |
| Install/uninstall | release binary, desktop entry, SVG icon installed; `--version` works; uninstall preserves preferences and NPLAY data |
| Second GUI launch | exits 0; first window remains and is activated; no second backend/player |

The pre-existing NPLAY suite was run before and after the service changes and
remained green. Standalone terminal NPLAY imports no GUI toolkit and needs no
display server.

## Functional evidence

- Start with no library/session: idle deck, explicit empty playlist state.
- Search `/` → asynchronous local results, group headers, Play all/Queue all.
- Queue all 6 results → queue panel updates live to 9 tracks (session queue plus
  added tracks), demonstrating subscription/revision state flow.
- Right-click menu, playlist creation, favorites, queue remove/move/clear and
  settings toggles use backend operations.
- Local playback path exercised with the real NPLAY `Player` against
  `tools/fake-mpv` (deterministic IPC, no audio): loading → playing → position
  advancement → pause → seek → stop. Clock, status badge and seek bar update.
- Stop backend while running → graphical “NPLAY connection lost” screen; Enter /
  Retry starts a new owned service and restores the session.
- Missing NPLAY → graphical dependency screen with installation/version remedy,
  not a panic.
- Windowshade toggles to 620×58 and back; geometry is remembered.
- Live NIRUX theme change applied while running (Nord → Satie), including a
  builtin theme with no custom TOML file; malformed custom theme keeps the
  previous appearance and reports a warning.
- Light Carl Larsson theme verified for contrast and readable controls.
- Scale 1.5× verified at 1600×1200; 800×600, 1366×768 and 1920×1080 fits with a
  one-time monitor clamp and no inaccessible primary controls.

## Screenshots

| File | Notes |
|---|---|
| `docs/screenshots/main.png` | NPLAY playing state, Nord theme, playlist populated, artwork, LED clock |
| `docs/screenshots/main-light.png` | Carl Larsson light theme, same flows |
| `docs/screenshots/search.png` | Async search results with Play all/Queue all |
| `docs/screenshots/settings.png` | Appearance/playback/source settings, live theme source |
| `docs/screenshots/windowshade.png` | Compact 620×58 windowshade |
| `docs/screenshots/dependency-error.png` | NPLAY missing dependency remedy |
| `docs/screenshots/visualization-harness.png` | **Synthetic CAVA harness only**; verifies bars/refresh/disabled-state drawing, not audio capture |

The visualization screenshot is deliberately labelled as a harness. The host
did not have `cava` installed, so real CAVA capture/visualization is **not**
live-verified.

## Resource measurements

Method: `scripts/measure.py`, `/proc/PID/stat` CPU ticks and
`/proc/PID/status` VmRSS, sustained intervals. Frontend is the release build.
Backend is Python NPLAY. “fake mpv”/“fake cava” are harnesses, not audio
players/capture. Renderer is llvmpipe software GL under Xvfb; hardware GPU cost
was not measurable on this host.

| Scenario | Duration | Frontend CPU | Frontend RSS | Backend CPU/RSS | Helper CPU/RSS | Total CPU |
|---|---:|---:|---:|---:|---:|---:|
| Idle, no current track | 45 s | **0.022%** | 119.4 MiB | 0.044% / 33.6 MiB | — | **0.066%** |
| Paused, restored session | 30 s | 0.067% | 118.8 MiB | 0.033% / 30.2 MiB | — | 0.100% |
| Playing (fake mpv, CAVA off) | 45 s | 2.821% | 118.8 MiB | 0.044% / 35.3 MiB | fake mpv 0.022% / 12.5 MiB | 2.887% |
| Playing + visualization harness | 20 s | 30.079% | 119.2 MiB | 0.250% / 35.7 MiB | fake mpv 0% / 12.5 MiB; fake cava 0.150% / 10.8 MiB | 30.479% |
| Large library: 2,006 indexed tracks, 1,000 rows loaded | 30 s | 0.033% | 135.5 MiB | 0.033% / 34.3 MiB | — | 0.066% |

Interpretation:

- Idle and paused meet the ≤1% of one logical CPU target (frontend alone
  0.022%, total 0.066%). There is no continuous idle redraw loop; theme
  watching only wakes on a file change.
- Playing uses one bounded clock wake per second (2.8% under software GL). On a
  hardware GL desktop this cost should be substantially lower; that is not
  claimed as measured.
- Visualization is the known renderer-heavy outlier under llvmpipe: the whole
  620×560 window is software-rasterized at roughly 12 FPS (10 Hz polling cap).
  This is renderer cost, not an audio/provider thread. Real CAVA capture and
  hardware GPU cost are unmeasured.
- Large-list state adds ~16.7 MiB RSS across 1,000 loaded rows; rows are
  virtualized and not all painted.
- Documented operational budget for these scenarios: **≤200 MiB total** across
  frontend + backend + helpers under software rendering. The observed maximum
  was 178.2 MiB (playing + visualization harness). A universal memory limit is
  not claimed for arbitrary libraries/themes/monitors.
- Minimized/obscured and mixed-DPI multi-monitor measurements were not possible
  without a real WM/desktop; code checks minimized/hidden state before polling
  visualization but this was not measured.

## Sources coverage

| Source | Status |
|---|---|
| Local library + playback/queue/playlists/artwork | Implemented; playback exercised through real NPLAY controller with fake mpv IPC; real audio not tested (mpv absent) |
| Custom radio | Browse/add implemented; stream playback not live-tested |
| Navidrome / Subsonic | Browse/search/play contract implemented and redacted; no server/credentials available, **not live-tested** |
| Sveriges Radio | Browse/live/programme/search contract implemented; live network/audio **not live-tested** in this report |
| YouTube | Search contract + yt-dlp availability gate implemented; yt-dlp/audio **not live-tested** |
| Spotify | Settings/OAuth handoff, browse/devices/play contract implemented; Premium account/librespot unavailable, **not live-tested**; EQ unsupported |
| CAVA visualization | Disabled state and synthetic rendering harness verified; real CAVA capture **not live-tested** |
| MPRIS/statistics/notifications | Owned by NPLAY and covered by its existing suite; service attaches to same controller |

## Acceptance gate status

- [x] Release-built Rust GUI launches under X11 with NPLAY launched as service;
      desktop entry/icon installed and inspected. *A real desktop-session launch
      was not possible on the headless test host.*
- [x] Normal standalone `nplay`, diagnostics and existing regression suite work
      without GUI dependencies (80 tests pass).
- [x] One authoritative queue/playback/statistics/MPRIS path by construction;
      second service/GUI launches do not create a second owner. Real audible
      duplicate-audio testing was not possible without mpv/audio.
- [x] Winamp inspiration is obvious in the deck/playlist/windowshade screenshots
      with original vector artwork.
- [x] Live theme apply verified for builtin-no-file, light/dark, malformed
      recovery and standalone fallback; custom/sparse theme unit tests pass.
- [x] Everyday workflows (search, browse, queue edits, playlists, favorites,
      settings, reconnect, seek) verified; playback state via fake mpv.
- [ ] **Every major configured source exercised live.** Unavailable accounts,
      mpv/CAVA and provider credentials prevented this; not claimed.
- [x] Seek/clock/visualization/EQ disabled states are honest; EQ is absent rather
      than fake; CAVA unavailable state is explicit.
- [x] Layout checks at 800×600, 1366×768, 1920×1080 and 1.5× scaling; HiDPI
      mixed-monitor testing remains unverified.
- [x] Resource measurements for idle, paused, playing, visualization harness and
      a 2,006-track library; minimized/obscured and real-CAVA/hardware-GPU
      measurements remain unverified.
- [x] Idle frontend 0.022% CPU (total 0.066%) meets the ≤1% target; memory budget
      documented at ≤200 MiB for measured scenarios.
- [x] Clean-checkout install and uninstall documented/tested; uninstall preserves
      user data.

## Remaining concrete blockers

1. No authorized bare-metal/SSH machines were supplied, so audio devices, real
   NIRUX/NIRUWM, hardware GL, HiDPI, WM tiling and actual provider accounts could
   not be exercised.
2. This host has neither `mpv` nor `cava` installed; real audio and real CAVA
   capture are unverified. The deterministic mpv IPC harness tests NPLAY's real
   controller path but emits no sound.
3. Navidrome, Sveriges Radio network playback, YouTube/yt-dlp and Spotify
   Premium/librespot require accounts/services not available here.
4. Equalizer is intentionally not implemented because NPLAY exposes no EQ DSP
   for mpv and Spotify is not routed through mpv.

These gaps are stated instead of being redefined as success. The reproduced
commands in `docs/TESTING.md` allow them to be verified on the owner's machines.
