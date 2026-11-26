# NIRUAMP 0.7.1

- **The clock follows the music.** The LED clock counts on locally between
  NPLAY's messages. After a track change it could start at the previous
  track's position (fixed in NPLAY 2.0.2) or run ahead while a stream still
  buffered. NIRUAMP now checks the position with NPLAY 1.5 seconds after a
  track change and every 10 seconds while playing, and takes NPLAY's value.
- **Hi-Fi Deck VU meters fill their faces.** The scale was sized by the width
  only, so a tall face had a large empty area above it. The arc, numbers and
  needle now use the whole face, with the pivot hub and the LOW/HIGH label
  below the needle, and the meter stays centred when the window is resized.

# NIRUAMP 0.7.0

Skins, livelier spectrum levels, a quieter welcome sound and a UX pass.

- **Skins.** Three original skins, all drawn in code and coloured by the
  current NIRUX (or Omarchy) theme's accent and foreground:
  - **Classic**, the hardware panel, polished: a soft panel gradient, a
    larger LED clock and spectrum, three-zone bars with peak caps, and
    shuffle/repeat lamps beside the transport.
  - **Phosphor**, a green CRT operator console: scanlines, a vignette and
    glass sheen over the tube, a glowing seven-segment clock, a monochrome
    cover and a monospaced readout.
  - **Hi-Fi Deck**, a 1980s stereo rack: brushed metal, a cassette in a
    smoked window with the track on its label and reels that follow the
    position, a VFD clock with a 16-band analyser, and two analogue VU meters
    (the low and high halves of the spectrum; CAVA is mono).
  Switch with `K`, the new skin button in the deck (where the always disabled
  equalizer button was) or Settings → Appearance → Skin; saved as `skin`.
- **Spectrum levels.** CAVA's autosens scales to the loudest bar, which left
  most bars near the floor. NIRUAMP now shows a decibel scale around a slowly
  adapting reference (fast up, slow down, frozen in silence) with a soft knee
  at the top, so quiet and loud masters look alike and nothing pins. Bars
  fall with gravity and leave peak caps that hold briefly. Measured with real
  CAVA (NPLAY's configuration) on four test tracks from -28 to -8 LUFS: the
  median bar height went from 0.24 to 0.34 of the display to 0.35 to 0.57
  (0.48 to 0.57 for full mixes), the 90th percentile from 0.56 to 0.76 to
  0.79 to 0.81, and bars at the very top from up to 6.4 % to at most 0.5 %.
- **Welcome sound.** 35 % by default instead of 65 % (and `paplay` got no
  volume at all before), at the same loudness in `pw-play`, `paplay`,
  `ffplay` and `mpv`. It plays only on the first NIRUAMP start after login
  (a marker in `$XDG_RUNTIME_DIR/niruamp`). Settings → Appearance → Welcome
  sound: Off, 20, 35, 50, 75 or 100 %, with TRY to preview. New key
  `startup_sound_volume`.
- **UX fixes.**
  - The windowshade button in the title strip did nothing; it works now.
  - The mute button can be activated with Enter like the other controls.
  - `B`, `/` and `Ctrl+F` opened an empty Browse panel on first use; they
    load the landing list now. `Esc` back to the playlist is remembered.
  - Themes with a grey focus colour (Niru Noir) made the playing row look
    disabled; the accent is raised until it reads, and the playing title
    only takes the accent where that is brighter than secondary text.
  - The lowercase "playing · OFF · OFF" line beside the transport is
    replaced by SHUFFLE and REPEAT lamps (REPEAT ONE / ALL).
  - Windowshade: the play button shows pause while playing, a previous
    button was added, and toasts no longer cover the strip.
  - Queue: PLAY, REMOVE, CLEAR and SAVE are disabled on an empty queue, and
    the redundant QUEUE button is gone from the queue itself.
  - The footer hint no longer runs under the NPLAY version in a narrow
    window, and lists `K` and `F11`; the version drops the protocol and
    revision noise (still in Settings → System).
  - Settings rows wrap instead of clipping; tooltips name their keys.
  - The About page lists the skins and the full keyboard.
- **Frugal.** Damage is now a few separate rects (a reel, an analyser and a
  needle no longer repaint everything between them), shape counts stay
  constant per frame, bars snap to whole pixels, skin textures are generated
  once per size. Playing with the spectrum: Classic 3.6 to 3.9 % of a core in
  Xvfb (0.6.0: 3.9 to 4.0 %), Phosphor 5.8 %, Hi-Fi Deck 7.5 %; paused,
  silent or hidden 0 %. `NIRUAMP_DEBUG_FPS` also prints the repainted pixels
  and paint time.
- New `config.example.toml` with every key and its default (a test keeps it
  in step with the code).

# NIRUAMP 0.6.0

A rewrite of the drawing layer: egui, eframe, winit and OpenGL are gone.
NIRUAMP now draws with its own small canvas GUI (x11rb, tiny-skia, MIT-SHM),
in the same style as NIRUFM and NIRUTERM. The look, views, themes, keys and
the NPLAY protocol are unchanged.

- Memory 8 MB instead of 120 MB (Xvfb; 124 MB on real hardware, mostly GL
  mappings), the window shows in about 30 ms instead of 120 ms, and 6 threads
  instead of 33.
- No CPU while nothing changes: one `poll` on the X connection, an eventfd
  for the worker threads and the frame scheduler's deadline. Only the changed
  part of the window is redrawn: playing with the 30 fps spectrum costs about
  2 to 3 % of one core in Xvfb (0.5.2: 305 % with llvmpipe).
- A static musl binary: no glibc, GL, EGL or xkbcommon libraries at run time.
  `install.sh` downloads it and checks it against the release's `SHA256SUMS`
  (`--source`/`--build` still builds with cargo).
- The egui default faces (Ubuntu Light, Hack) are embedded, so text looks as
  before; missing symbols (arrows) now come from a system font instead of
  boxes. A NIRUX `font_path` is still used.
- `--class NAME` sets WM_CLASS (default `niruamp`, instance and class).
- The tray starts after the first frame, so a slow D-Bus never delays the
  window.
- Fixes: typing in the search field no longer triggers shortcuts (N, P, M
  and friends); Ctrl+F no longer also favorites the current track; the back
  button beside the search field is no longer covered; Enter saves a
  playlist name dialog.
- Wayland: NIRUAMP is an X11 program now and runs through XWayland. The
  Hyprland integration (floating rule, pin, tray workspace, visibility from
  the compositor) is kept and used under XWayland. `NIRUAMP_BACKEND` is
  gone.
- New `tests/xvfb.sh`: an end-to-end run in a private Xvfb against a
  sandboxed NPLAY (first frame, memory, idle CPU, keys, context menu, queue,
  windowshade, search, settings, window drag, hidden spectrum, close). CI
  and the release workflow run it.

# NIRUAMP 0.5.2

Local Spotify playback authorization (needs NPLAY 1.8.1, protocol 1.2).

- Settings → Sources → Spotify shows a **Local playback** row: authorized /
  not authorized / authorizing, with **AUTHORIZE LOCAL** or **REAUTHORIZE
  LOCAL**. It calls `spotify.local_authorize`; NPLAY opens Spotify's sign-in
  page in the browser and reports the result (`spotify.local_auth`) as a
  toast. "install librespot" is shown when NPLAY has no librespot.
- A Spotify play that fails because local playback is not authorized
  (`service.error`, `code: spotify_local_unauthorized`) shows NPLAY's message
  in a dialog with **AUTHORIZE LOCAL PLAYBACK**, instead of failing silently.
  Other asynchronous NPLAY errors (`service.error`) appear as error toasts.
- With NPLAY ≤ 1.8.0 nothing changes: the capability is missing, the row and
  dialog are hidden. X11/Wayland behavior and the NIRUX theme coupling are
  unchanged.

# NIRUAMP 0.5.1

- X11: WM_CLASS is `"niruamp", "niruamp"` again, as in 0.4. With the Wayland
  backend compiled in, egui-winit set the app id through winit's shared name
  attribute and left the X11 instance name empty. The app id is now applied
  only under Wayland.

# NIRUAMP 0.5.0

Omarchy theme and native Wayland. In the NIRU session (NIRUWM, X11) nothing
changes: same backend, same theme resolution, same look.

- Native Wayland: built with winit's Wayland and X11 backends. NIRUAMP uses
  Wayland whenever `WAYLAND_DISPLAY` is set outside the NIRU session (crisp at
  fractional scales); `NIRUAMP_BACKEND=x11|wayland` overrides it.
- Omarchy theme: **Theme → Auto** follows the current Omarchy theme
  (`colors.toml`, or `alacritty.toml` for old themes; Omarchy 4 and 3.x
  layouts) and switches live. New choices **Omarchy · current theme** and
  **NIRUX · current theme**. The NIRU session always follows NIRUX first.
- Hyprland: an in-memory window rule opens NIRUAMP floating at its saved size
  and position; Always on top pins it; minimize to tray uses
  `special:niruamp`; the window position is read back from Hyprland; the
  spectrum pauses when the window's workspace is not shown (event socket, no
  polling). Uses Hyprland's IPC socket directly, Lua (0.55+) or classic
  dispatchers.
- Fix: no blocking vsync off a real X server. Under Wayland (and XWayland) a
  hidden window gets no frame callbacks, so a frame drawn while hidden (the
  clock while playing) blocked the UI thread in eglSwapBuffers and pings went
  unanswered: Hyprland showed "Application not responding".
- Wayland window size is saved too (Wayland reports no window position, which
  had also hidden the size).
- A theme that fails at start falls back to Classic instead of aborting.

# NIRUAMP 0.4.0

Efficiency and build release. The look, themes, views and keys are unchanged.

- Build: the locked build works on Rust 1.85 (Debian 13). The lockfile was
  re-resolved with the MSRV-aware resolver (kept on in `.cargo/config.toml`),
  `rust-version` is 1.85 (ksni 0.3.7 needs async closures), the
  `rust-toolchain.toml` pin to `stable` is gone, and CI has an MSRV job. A
  release workflow builds the prebuilt x86_64 binary on Ubuntu 22.04.
- One frame scheduler: the LED clock, spectrum, toasts, search debounce and
  retries are reduced to one repaint deadline per frame, NPLAY messages wake the
  window once per frame, and egui's predicted-frame-time subtraction no longer
  turns a 33 ms step into bursts. Playing with the spectrum visible draws about
  30 frames/s instead of 56 to 170.
- With NPLAY 1.8.0 the spectrum uses `visualization.subscribe` (b64 frames
  pushed by NPLAY) instead of polling `visualization.data` 30 times a second.
  During silence NIRUAMP draws a flat spectrum and stops animating. NPLAY 1.7.x
  keeps the polling path as a fallback (no polls while paused).
- The spectrum pauses when the window cannot be seen: another NIRUWM workspace
  (unmapped), `_NET_WM_STATE_HIDDEN`, fully covered, minimized or in the tray.
  NPLAY then stops CAVA. A window in the tray repaints not at all.
- Settings → Visualization: "Pause spectrum when the window is not visible"
  (default on) and a frame rate of 30/20/15 fps.
- No more polling threads: the single-instance socket blocks in `accept`, the
  theme watcher uses inotify.
- Settings no longer rewrites `config.toml` on every frame while open.
- The artwork texture cache is an LRU bounded by 8 entries and 16 MiB.
- Requires NPLAY 1.7.3; NPLAY 1.8.0 recommended.

# NIRUAMP 0.3.3

- Add a distraction-free fullscreen visual mode: black screen, centred track,
  artist and album text, themed CAVA bars, progress/time and an English
  "Press Esc or F11 to close fullscreen" hint.
- Open it from the title-bar fullscreen button or F11; close with Esc or F11.
  The normal visualizer preference is not changed; fullscreen always requests
  CAVA while it is open.
- Requires NPLAY 1.7.3.

# NIRUAMP 0.3.2

- Recover from a transient CAVA startup/exit failure: visualization failures now
  retry every few seconds, on track change, after reconnect, and when the user
  re-enables the visualizer instead of staying stuck on “VISUALIZER UNAVAILABLE”.
- Requires NPLAY 1.7.3, which reports CAVA start failures accurately and restarts
  CAVA after an unexpected exit.

# NIRUAMP 0.3.1

- Fix album/playlist/folder/programme context actions: Play, Play next,
  Add to queue, Add to playlist and New playlist now resolve the container into
  its tracks through NPLAY before acting. Playing an album plays its tracks
  instead of opening a pseudo row.
- Add explicit Open album/playlist/folder/programme and Open artist/Connect
  Spotify device actions; hide unsupported Favorite actions on containers.
- Play/Queue All only operate on playable tracks, and the buttons are hidden
  when a list contains no tracks (for example an album list).
- Frontend filters container rows from playback context and bulk queueing;
  NPLAY 1.7.2 rejects containers at the service boundary and adds
  `playlists.add_many`.
- Legacy container rows in the persisted queue/context, local playlists and
  favorites are cleaned at service startup.
- Requires NPLAY 1.7.2 or newer.

# NIRUAMP 0.3.0

- NIRUX-theme-aware tray icon: the framed play mark follows the active theme's
  panel, border and LED/accent colours and updates live with the theme watcher.
- Optional Winamp-inspired startup sound, embedded as Ogg Vorbis and played once
  through PipeWire/PulseAudio/FFplay/mpv. It is a UI effect only, never NPLAY
  media, and can be disabled under Settings → Appearance.
- The logo beside the NIRUAMP name is now just a play mark inside a frame.
- Remaining resource work is bounded-cache/virtualization/event-driven redraw;
  a second in-frontend CAVA is intentionally not implemented.

# NIRUAMP 0.2.0

- Keyboard focus/tab navigation for custom controls; Enter activates the
  focused control while Space remains the global play/pause shortcut.
- Virtualized browser and search results for large libraries.
- Drag-and-drop queue reordering through the new NPLAY `queue.reorder` method.
- A visible window resize grip for the undecorated titlebar.
- Artwork is decoded to a maximum 256×256 texture and the texture cache is
  reduced to 8 entries.
- CAVA visualization now samples at 30 Hz, uses log-shaped bars and fast-attack/
  slow-release display smoothing for a more responsive but less jittery meter.
- Optional freedesktop StatusNotifierItem system tray: minimize to tray, restore
  by left click, middle-click play/pause, scroll-wheel volume and a menu with
  show/play/next/previous/quit.
- Requires NPLAY 1.7.1 or newer.

# NIRUAMP 0.1.0

- First public release: Winamp-inspired deck, playlist/queue, async search and
  browsing, NIRUX live theming, artwork, windowshade, installer and Debian 12+
  prebuilt binary.
