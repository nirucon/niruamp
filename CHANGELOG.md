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
