# NIRUAMP validation report

## 0.5.0 Omarchy theme and native Wayland (2026-11-06)

**NIRU session regression (othala, Debian 13, NIRUWM 0.16.173, real session,
locked screen):** 0.4.0 and 0.5.0 run one after the other with the owner's
configuration (`theme = "auto"`, NIRUX theme Forest Vigil).
- The release-build screenshots (`NIRUAMP_SCREENSHOT`) are identical except
  the version text: 32 of 389 850 pixels differ, all inside the "v0.5.0"
  label; the six most frequent colours are the same.
- The window is covered by the lock, so the spectrum pause applies. Both draw
  1 frame/s (the clock) in steady state: CPU 0.10 % (0.4.0) and 0.07 %
  (0.5.0), 8.3 wakeups/s each, 124–125 MiB RSS. GLX as before; vsync on, as
  before.

**Omarchy (Omarchy 4.0 alpha, Hyprland 0.57.1, Intel laptop, 1920×1200 at
scale 1, real session), 0.5.0 build from the release commit:**
- Native Wayland client (`xwayland: false`), opened floating at the saved
  position (200, 150) and size 620×560 by the placement rule, without taking
  focus from the owner's window.
- Theme: Auto followed the owner's Forest Vigil port (identical to NIRUX's
  Forest Vigil palette). Live switches with `omarchy-theme-set` to Tokyo Night,
  Catppuccin Latte (light) and back applied within the 320 ms debounce;
  screenshots below.
- ANR: reproduced with the first build — playing while the window sat on
  another workspace blocked the UI thread in `eglSwapBuffers` (wchan `poll`)
  and the `xdg_wm_base` pings went unanswered for over a minute. Fixed by not
  waiting for vsync off a real X server. Soak with the fix, 16 min while
  playing (6 min hidden on another workspace, 6 min visible, 4 min moving
  between them every 10 s): 645 pings, 645 pongs, longest ping→pong 0.1 ms,
  no "Application not responding". CPU 0.02 % hidden, 0.10 % visible.
- Hidden on another workspace: 0 frames/s, CPU 0.00 %, 4 wakeups/s (Hyprland
  event socket, no polling).
- Tray: the StatusNotifierItem registers with Omarchy's shell; Activate from
  D-Bus brought the window back from `special:niruamp` to the shown workspace.

**Nested Hyprland 0.57.1 on the same machine, output 1600×1000 at scale
1.25:** 620×560 logical window rendered at 775×700 pixels (crisp, not
upscaled); key T pinned and unpinned the window (Hyprland pin) and saved the
preference; another nested workspace paused it (CPU 0.00 %); a second
`niruamp` start brought a parked window back and focused it; a compositor move
to (300, 200) was read back and saved within 30 s and restored on restart
(Hyprland clamps y to keep the window on the small output).

Not verified with real input: dragging the deck title (StartDrag →
`xdg_toplevel.move`) needs a physical pointer press; no input injection was
used on the owner's machine.

## 0.4.0 energy and efficiency (2026-11-05)

Host `othala` (Debian 13, 12 cores), isolated harness: Xvfb 1600×1000, no
compositor, Glow on llvmpipe (software GL), private D-Bus session, isolated
`HOME`/XDG dirs, NPLAY from its tag checkout, NPLAY's `tools/fake-mpv` (no
audio) and a synthetic CAVA (raw 8-bit, 30 fps; all-zero frames for the
"silence" row). Each row is a 15 s sample after 3 s settling. Frames/s come
from `NIRUAMP_DEBUG_FPS=1`; CPU and wakeups (voluntary + involuntary context
switches) from `/proc`. "before" is the published 0.3.3 binary, "after" is
0.4.0.

NIRUAMP frames/s (the frame rate is what NIRUAMP controls; under llvmpipe every
frame costs ~0.1 CPU-seconds spread over its rasterizer threads, so CPU % here
scales with frames and is far higher than with a GPU):

| State | 0.3.3 + NPLAY 1.8.0 | 0.4.0 + NPLAY 1.8.0 (stream) | 0.3.3 + NPLAY 1.7.3 | 0.4.0 + NPLAY 1.7.3 (poll fallback) |
|---|---:|---:|---:|---:|
| Playing, spectrum visible | 57–60 | **29.8** | 55–56 | **29.0–29.7** |
| Playing, silence | 57–59 | **1.0** (flat, clock only) | 56 | 29.0 (poll path keeps polling) |
| Playing, window unmapped (another workspace) | 58–60 | **0** (CAVA stopped) | 56–60 | **0** (CAVA stopped) |
| Paused, visible | 58–60 | 29.8 ¹ | 56–60 | **≈1** |
| Playing, hidden to tray | 1.0 | **0** | 1.0 | **0** |
| Paused, hidden to tray | 0 | 0 | 0 | 0 |

¹ The synthetic CAVA keeps producing bars while paused; real CAVA sees
silence, so NPLAY sends one zero frame and NIRUAMP settles to 0–1 fps (the
"silence" row).

Other processes, playing with the spectrum visible:

| Process | 0.3.3 (poll, 30 Hz) | 0.4.0 + NPLAY 1.8.0 (stream, b64) |
|---|---:|---:|
| NPLAY service CPU / wakeups/s | 1.7–1.8 % / 147–154 | **0.6 %** / **31** |
| NIRUAMP main-thread wakeups/s | 1 400–1 650 | 746–762 (≈25/frame inside llvmpipe) |
| NIRUAMP, unmapped / tray: CPU, wakeups/s | 154–198 % / 4 000+ ; 2.7–8.7 % / 87–100 | **0.0–0.07 %** / **2** ; **0.0 %** / **2** |
| NIRUAMP RSS | 118.7–119.0 MiB | 118.5–119.1 MiB |

Real NIRUWM 0.16.172 on the same Xvfb: `Super+2` unmapped the window
(`IsUnMapped`); within 1 s NIRUAMP unsubscribed and NPLAY had stopped CAVA;
NIRUAMP then used 0.0 % CPU and 2 wakeups/s. `Super+1` remapped it and the
spectrum and CAVA resumed (29.3 fps).

RSS breakdown (0.4.0, playing, 118.8 MiB): libLLVM 56.2 MiB and gallium
11.5 MiB (the software GL driver), heap 24.5 MiB, the binary 8.2 MiB, anonymous
6.2 MiB. Artwork is at most 8 × 256 KiB of decoded pixels (now also bounded to
16 MiB). The memory is the GL stack, not NIRUAMP's caches; it is not reduced
here without changing the renderer, which would change the look.

Not verified here (needs the owner's session, deferred to niruxwork): the
≤ 2.5 % CPU and ≤ 120 main-thread wakeups/s targets with a hardware GL driver,
real CAVA, and the tray icon host. Under llvmpipe the GL driver alone wakes the
main thread ~25 times per frame.

## 0.3.3 report

Date: 2026-10-31
Frontend: NIRUAMP 0.3.3, tag `v0.3.3` (Rust, `eframe`/`egui` 0.29.1, Glow, ksni 0.3.7)
Backend: NPLAY 1.7.3, control protocol 1
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
| NPLAY revision | `2cfaa4c` (release: NPLAY 1.7.3 CAVA recovery) |
| Release asset | `niruamp-0.3.3-x86_64-linux-gnu.tar.gz`, SHA-256 `81fd59f75dcc7d103e60677bbcddfb5bdc144ab4cb46c1d207f67e704d239559` (Debian 12 build) |
| Live target | `niruxwork`, Debian 13.7 (trixie), kernel 7.2.11+deb13-amd64, AMD Ryzen AI 9 HX PRO 375, 24 logical CPUs, 62 GiB RAM, Radeon 880M/890M, X11/NIRUX |

The owner supplied a Tailscale/SSH target (`niruxwork`) late in the pass. Live
Navidrome playback, Sveriges Radio browse, real mpv and real CAVA were then
verified there, plus a prebuilt-binary installation. No credentials or test
results were fabricated; Spotify/YouTube accounts remained unavailable.

## Automated tests

| Suite | Result |
|---|---|
| Rust format (`cargo fmt --all -- --check`) | pass |
| Rust clippy (`cargo clippy --all-targets -- -D warnings`) | pass |
| Rust unit tests (`cargo test`) | **18 passed** |
| Rust release build (`cargo build --release --locked`) | pass |
| NPLAY full Python suite (`python3 -m unittest discover -s tests`) | **82 passed** (68 pre-existing + 14 service) |
| NPLAY service tests | handshake, events, playlists, queue, bulk queue, external queueing, artwork redaction, malformed/oversized frames, permissions, second service, graceful shutdown, idle exit, local playback via real `Player` boundary with mpv IPC harness |
| Install/uninstall | release binary, desktop entry, SVG icon installed; `--version` works; uninstall preserves preferences and NPLAY data |
| Second GUI launch | exits 0; first window remains and is activated; no second backend/player |

The pre-existing NPLAY suite was run before and after the service changes and
remained green. Standalone terminal NPLAY imports no GUI toolkit and needs no
display server.

## Live verification on niruxwork (Debian 13.7, NIRUAMP 0.1.0)

- Prebuilt `v0.1.0` x86_64 release asset installed with `./install.sh --prebuilt`
  (no Rust toolchain used at install time); `niruamp --version` returns 0.1.0.
- NPLAY 1.7.0 running as the service; the real GUI attached to the existing
  session without starting a second engine.
- Navidrome configured and live: browsing `newest` returned albums, and a real
  Navidrome stream was playing through mpv (`transport=mpv`, `source=navidrome`,
  position advanced over repeated snapshots).
- Sveriges Radio configured and live: `sr/live` returned P1/P2/P3/P4 entries.
- Real CAVA was running and delivering visualization data; the UI displayed
  live spectrum bars (confirmed process `cava`, capability `visualization=true`).
- NIRUX theme in use was `niru-forest-vigil`; the new binary rendered the real
  desktop theme (green palette) correctly.
- Real mpv and CAVA processes were present alongside the frontend and backend.
  Audible output was not independently recorded, but mpv was decoding a live
  Navidrome stream, position advanced, and CAVA captured the output.
- A screenshot of the real desktop was inspected locally. It is intentionally
  **not committed** because it contains the owner's personal library metadata,
  artwork and queue. The provided screenshots use synthetic test library data.

Real-hardware resource sample while playing with real CAVA, 30 s, 24 logical
CPUs:

| Process | CPU | RSS |
|---|---:|---:|
| NIRUAMP frontend | 4.596% | 192.6 MiB |
| NPLAY backend | 0.966% | 59.5 MiB |
| mpv | 3.064% | 110.4 MiB |
| cava | 1.732% | 15.8 MiB |

This is the most representative measurement in this report: real playback,
real capture and a real desktop session. Frontend cost is 4.6% of one logical
CPU (of 24), not the much higher software-GL/Xvfb figure.

## 0.2.0 local validation (Xvfb, NPLAY 1.7.1)

- Tab traversal focuses custom controls with a visible ring; Enter opened
  Settings from a focused titlebar button. Search typing did not trigger
  transport shortcuts.
- Drag-and-drop queue reorder changed `[02, 03, 04, …]` to `[03, 04, 02, …]`
  through `queue.reorder` and updated the selected row.
- The resize grip changed the undecorated window from 620×552 to 775×604 under
  Xvfb without a WM.
- Optional tray: under `dbus-run-session`, the service registered
  `org.kde.StatusNotifierItem-<pid>-1`; minimize hid the window and a synthetic
  SNI `Activate` call restored it. Actual NIRUBAR icon/menu rendering was not
  live-tested because that desktop was shut down; it is the first thing to check
  on the next boot.
- With `ksni` available but no SNI watcher, the app continued normally; without
  D-Bus, tray spawn fails softly and minimize falls back to a normal window.
- CAVA smoothing was exercised with the labelled synthetic CAVA harness: bars
  change between consecutive frames, are log-shaped, and ease in/out rather than
  stepping.
- Artwork test decodes a 1024×768 PNG to a 256×192 texture; the texture cache is
  capped at 8.
- Browser/search lists now use `show_rows` virtualization.
- The prebuilt 0.2.0 binary was installed on niruxwork with NPLAY 1.7.1 and
  started in the real NIRUX/X11 session. The SNI item registered with NIRUBAR's
  StatusNotifierWatcher, the original icon was visible in the bar, left-click
  restored a minimized window, and the right-click menu showed Show NIRUAMP,
  Pause, Next track, Previous track and Quit NIRUAMP. NPLAY was really playing
  a Navidrome track through mpv with cava running. A desktop screenshot was
  inspected but is not committed because it contains the owner's personal
  session, credentials in a terminal and library metadata.
- Real-hardware 0.2.0 measurement while playing with real CAVA and the tray
  active: frontend **8.491%** of one logical CPU / **188.2 MiB** RSS, backend
  **2.397% / 49.1 MiB**, mpv **2.764% / 108.2 MiB**, cava **1.698% / 15.8 MiB**.
  The higher frontend/backend cost than 0.1.0 is the requested 30 Hz CAVA polling
  and is still well below one of the 24 logical CPUs.

## 0.3.0 local validation

- The owner-supplied MP3 was converted to Ogg Vorbis (48 kHz stereo, 4.51 s,
  76 KiB) and embedded in the binary. The sound is a one-shot UI effect with a
  settings toggle; it is written atomically to the user cache and played through
  `pw-play`, `paplay`, `ffplay` or `mpv` with fallback if a player fails.
- The tray icon is generated from the active palette (panel/border/LED colours)
  and refreshed by both manual theme reloads and the live NIRUX watcher. There
  is no hard-coded amber/green tray asset any more.
- The titlebar logo is now only a play triangle inside a frame; the underline was
  removed.
- A second in-frontend CAVA was deliberately rejected: it would duplicate audio
  capture/DSP, break the NPLAY boundary, and would not remove the dominant cost
  (full-window redraw). The existing CAVA path remains.
- `cargo fmt`, clippy and 18 Rust tests pass; NPLAY remains 1.7.1.
- Live on niruxwork after installing 0.3.0: `pw-play --media-role=Notification
  --volume=0.65 ~/.cache/niruamp/startup.ogg` ran at launch and the 76,511-byte
  embedded asset was materialised with mode 0600. It did not alter NPLAY's
  current/paused media state.
- Live tray colour on niruxwork: the framed play icon in NIRUBAR changed from the
  previous fixed amber/green design to the forest-vigil palette (dark tile with a
  light green play mark), and the titlebar logo showed only the play triangle in
  its frame. The tray item still registered and the menu/restore behaviour from
  0.2.0 remained.
- The 0.3.0 desktop/app screenshots were inspected but are not committed because
  they contain the owner's personal library, session and terminal content.

## 0.3.1 container-action validation

- Reproduced the reported bug against the previous build: container rows
  (album/playlist/folder/programme) were sent directly to `queue.add` and shown
  as pseudo tracks, and Play merely navigated.
- With 0.3.1, right-click → **Add album to queue** on a local album added the
  album's **6 real tracks** and no album row. Right-click → **Play album** set a
  6-track playback context and played the first track.
- Right-click → **Add album to playlist → New playlist…** created a playlist with
  all 6 tracks, confirming expansion before `playlists.add_many`.
- Play All / Queue All are hidden on the Local Albums list because it contains no
  playable track rows; they remain on track lists.
- Container actions are labelled explicitly (Play album, Open album, Open artist,
  Connect Spotify device, etc.); Favorite is only offered for tracks.
- NPLAY 1.7.2 rejects container rows at playback/queue/playlist/favorite
  boundaries, adds `playlists.add_many`, and cleans legacy non-playable queue,
  context, playlist and favorite rows at service startup.
- Automated service test covers local album discovery, container rejection,
  normal track playback references and bulk playlist insertion.
- NIRUX 0.12.158 pins NPLAY 1.7.2 and NIRUAMP 0.3.1 as the current music extras.

## 0.3.2 CAVA recovery validation

- Reproduced the report on niruxwork: after a service restart the frontend showed
  `VISUALIZER UNAVAILABLE`. CAVA was not running, but the old service returned
  `visualization.start: available=true` and then data/process failures left the
  frontend permanently disabled.
- NPLAY 1.7.3 now verifies that CAVA actually started, reports a structured error
  otherwise, detects CAVA exit, and restarts it on the next visualization frame.
- NIRUAMP 0.3.2 retries failed visualization after 4 seconds, on track change,
  after reconnect and when re-enabled, instead of remaining stuck.
- Live on niruxwork: killed the running `cava` process; the backend started a new
  `cava` process on the next frame and returned non-zero bars. The GUI continued
  playing with the visualizer visible, and the screenshot showed bars rather than
  the unavailable message.
- Full niruxwork versions after the update: NIRUAMP 0.3.2 / NPLAY 1.7.3.

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
| **0.2.0** real playback + real CAVA + tray (niruxwork, 24 logical CPUs) | 30 s | **8.491%** | **188.2 MiB** | 2.397% / 49.1 MiB | mpv 2.764% / 108.2 MiB; cava 1.698% / 15.8 MiB | 15.350% |

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
- Minimized/obscured and mixed-DPI multi-monitor measurements were not measured;
  the tray-hidden path suspends visualization and CAVA delivery, and
  hide/restore was verified live.
- 0.2.0 CPU/RSS above was measured on niruxwork while genuinely playing with
  real CAVA and the tray active.

## Sources coverage

| Source | Status |
|---|---|
| Local library + playback/queue/playlists/artwork | Implemented; playback exercised through real NPLAY controller with fake mpv IPC; real audio not tested (mpv absent) |
| Custom radio | Browse/add implemented; stream playback not live-tested |
| Navidrome / Subsonic | **Live-verified on niruxwork**: browse and real mpv playback of a Navidrome stream |
| Sveriges Radio | **Live-verified on niruxwork**: configured, `sr/live` returned P1–P4; live audio not independently recorded |
| YouTube | Search contract + yt-dlp availability gate implemented; yt-dlp/audio **not live-tested** |
| Spotify | Settings/OAuth handoff, browse/devices/play contract implemented; Premium account/librespot unavailable, **not live-tested**; EQ unsupported |
| CAVA visualization | **Live-verified on niruxwork**: real cava process, `visualization=true`, live bars; synthetic harness also exercised on the build host |
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
- [ ] **Every major configured source exercised live.** Local, Navidrome and SR
      browse/play plus real mpv/CAVA were live-verified on niruxwork; Spotify and
      YouTube accounts were unavailable, so full coverage is not claimed.
- [x] Seek/clock/visualization/EQ disabled states are honest; EQ is absent rather
      than fake; CAVA unavailable state is explicit.
- [x] Layout checks at 800×600, 1366×768, 1920×1080 and 1.5× scaling; HiDPI
      mixed-monitor testing remains unverified.
- [x] Keyboard focus/Tab+Enter, queue drag reorder, resize grip, browser
      virtualization, artwork downscaling and tray hide/restore were verified
      locally and the real NIRUBAR tray icon, restore click and menu were verified
      on niruxwork with NIRUAMP 0.2.0.
- [x] 0.3.0 theme-driven tray icon and startup sound were live-verified on
      niruxwork; startup sound is embedded, optional and outside NPLAY state.
- [x] Resource measurements for idle, paused, playing, visualization harness and
      a 2,006-track library; minimized/obscured and real-CAVA/hardware-GPU
      measurements remain unverified.
- [x] Idle frontend 0.022% CPU (total 0.066%) meets the ≤1% target; memory budget
      documented at ≤200 MiB for measured scenarios.
- [x] Clean-checkout install and uninstall documented/tested; uninstall preserves
      user data.

## Remaining concrete blockers

1. The build host has neither `mpv` nor `cava`, so build-host playback and CAVA
   used labelled harnesses. Real mpv playback and real CAVA were verified later
   on niruxwork.
2. YouTube/yt-dlp and Spotify Premium/librespot accounts were unavailable, so
   those sources remain not live-tested. Real Navidrome and SR paths were
   verified.
3. HiDPI/mixed-monitor, WM tiling and audible-output recording were not
   independently exercised; the live target used a normal X11 desktop.
4. Equalizer is intentionally not implemented because NPLAY exposes no EQ DSP
   for mpv and Spotify is not routed through mpv.

These gaps are stated instead of being redefined as success. The reproduced
commands in `docs/TESTING.md` allow them to be verified on the owner's machines.
