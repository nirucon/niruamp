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
  30 frames/s instead of 56–170.
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
