# NIRUAMP architecture

## Boundary

NIRUAMP is a graphical presentation layer. NPLAY is the required playback
engine and owns the authoritative queue, transport, providers, library,
artwork acquisition, listening statistics, notifications and MPRIS. The
frontend never starts mpv, librespot, a scanner or a statistics pipeline.

The two programs meet at NPLAY's versioned local service (protocol 1, shipped in
NPLAY 1.7.1). See [PROTOCOL.md](PROTOCOL.md).

```
NIRUAMP (Rust/egui)                    NPLAY (Python)
┌────────────────────┐   Unix socket  ┌─────────────────────────────┐
│ views / app state  │◄──────────────►│ nplay/service (JSON-RPC)    │
│ protocol client    │  snapshot+events│ App / queue / providers     │
│ theme resolver     │                │ mpv IPC · Spotify · SQLite  │
│ artwork decoder    │                │ MPRIS · statistics · CAVA   │
└────────────────────┘                └─────────────────────────────┘
```

One backend session has one queue and one MPRIS owner. A terminal NPLAY and
NIRUAMP can attach to the same service. NPLAY remains fully standalone.

## Rust modules

| Module | Responsibility |
|---|---|
| `main.rs` | Arguments, single-instance claim, backend acquisition, eframe startup |
| `app.rs` | State, commands, event handling, optimistic loading, keyboard, top-level layout, modals/toasts |
| `protocol.rs` | Newline-delimited JSON-RPC client, handshake, reader/writer threads, bounded wakeups |
| `platform.rs` | Socket discovery, `nplay --serve` launch, ownership, instance socket |
| `model.rs` | Typed wire models (tracks, playback, browse/search/settings) |
| `config.rs` | NIRUAMP preferences and window state in XDG directories (0600, atomic) |
| `theme.rs` | NIRUX appearance resolution, builtin/custom themes, font/spacing, watcher |
| `views/deck.rs` | Title strip, artwork, LED clock, spectrum, seek/volume, transport, shade |
| `views/playlist.rs` | Dense rows, playlist/queue panels, context menus |
| `views/browser.rs` | Async search, source sections, landing and result lists |
| `views/settings.rs` | Appearance/playback/source/library/system settings, about |
| `views/common.rs` | Original bevel/inset primitives, vector icons, 7-segment digits, marquee, slider |

## State synchronisation

1. Handshake (`initialize`) returns a full snapshot and validates protocol 1.
2. The client sends `state.subscribe`; the response is another snapshot that
   closes the gap between handshake and subscription.
3. `state.changed` carries a monotonic revision and a full playback/source/
   capability view. Out-of-order or older revisions are ignored.
4. Responses are correlated by request id. Provider calls run in the backend's
   worker threads, never on the UI thread.
5. A lost connection replaces the UI with an actionable dependency screen; Retry
   re-discovers or relaunches NPLAY. The frontend never opens its own player.
6. Playback commands are optimistic only for the requested track/context. A
   success event commits the authoritative state; a backend error requests a
   fresh snapshot and shows the error.

Search and browse requests carry generation numbers and queries. Late responses
for an older query are discarded. Routine state events never move keyboard
selection or steal text focus.

## Rendering and repaint policy

- One attached window contains a custom deck and a panel (playlist, queue,
  browser, settings, about). Attached panels avoid cross-window focus and WM
  policy differences.
- The titlebar is client-drawn for the narrow Winamp strip; dragging uses the
  native move request, and always-on-top uses the viewport window level.
- Idle/paused: no periodic repaint. Input, backend notifications, theme changes
  and instance activation wake the UI.
- Playing: one bounded clock wake per second. The LED only renders whole
  seconds; position is interpolated locally between authoritative samples.
- Visualization: 30 Hz sampling with fast-attack/slow-release display easing and
  log-shaped actual CAVA values; reduced motion lowers the rate. CAVA is stopped
  when hidden, disabled, minimized to tray, or after the service's idle timeout.
  No synthetic bars are generated.
- Lists use `show_rows` virtualization in playlist, queue, browse and grouped
  search results.
- Custom controls participate in egui's focus chain and draw a visible focus
  ring; Enter activates the focused control. Rows are selected with Up/Down and
  are deliberately not part of the tab chain.
- Windowshade changes the viewport size and draws a compact 58-point deck.
- A bottom-right grip resizes the undecorated window.
- An optional StatusNotifierItem tray lives on its own D-Bus thread. Menu
  actions cross a bounded channel to the GUI thread; the GUI never blocks on
  D-Bus. Hiding to tray suspends visualization and stops CAVA delivery.
- Artwork decoding/downscaling runs in a worker thread to at most 256×256;
  textures are capped at 8 entries. The backend artwork cache is bounded
  separately.

## Appearance resolution

Resolution follows NIRUX:

1. `theme` in `$XDG_CONFIG_HOME/niruwm/config.toml`.
2. `$XDG_CONFIG_HOME/niruwm/themes/NAME.toml` if present (custom theme wins).
3. Otherwise a compiled builtin palette (Noir, Nord, Satie, Carl Larsson,
   Forest Vigil, Blood of Urdr, Root and Stone, Blue Horizon, The Norns, Winter
   Nights).
4. A valid schema-1 `appearance.toml` can override semantic roles and supply
   font size/path, spacing, focus width and reduced-motion. Colour overrides
   are not applied when a custom theme file exists, matching NIRUX precedence.
5. Missing derived roles are computed with contrast-preserving blends. Light
   themes get dark success/warning/error roles; dark themes get light ones.
6. Invalid `appearance.toml` is ignored with a warning. A malformed custom theme
   keeps the previous working appearance. A watcher debounces filesystem changes
   and applies a new valid theme live.

Manual `classic` is the standalone NIRUAMP fallback and deliberately ignores
NIRUX colour overrides.

## Lifecycle and ownership

- Single GUI instance: a private instance socket activates the existing window.
- Backend discovery first. If absent, NIRUAMP launches installed `nplay --serve`
  with an idle-exit boundary. It never downloads dependencies at launch.
- `owned=true` only when this process launched the service. On normal exit an
  owned service receives `service.shutdown` unless keep-playing is enabled.
  Attached services are never stopped by the GUI.
- A service exits on its own when launched with `--idle-exit` and the GUI
  disconnects/crashes, preventing orphaned audio.
- A second service cannot take the endpoint lock and exits without disturbing
  the live socket. Stale sockets are replaced only after ownership checks.
- Minimize-to-tray is best-effort: without a D-Bus SNI host it falls back to a
  normal minimized window. Closing the window quits NIRUAMP; quit is also in the
  tray menu.
- Preferences and window state live in `~/.config/niruamp` and
  `~/.local/state/niruamp`; playback/session/library data remain NPLAY's.

## Security

- Unix socket under the user's private runtime directory; mode 0600; directory
  0700; per-user advisory lock; ownership validation.
- Protocol frames are bounded. Client queues and outstanding requests are
  bounded. Slow clients cannot stall playback.
- Track objects are redacted: no paths, stream URLs, tokens, cover URLs or
  arbitrary metadata. Playback/artwork use bounded backend-track references.
- Artwork is limited to catalogue tracks and capped in size; no arbitrary file
  read or URL fetch is exposed.
- Credentials/OAuth live only in NPLAY's 0600 secrets file and never appear in
  protocol responses, logs or the GUI.

## Deliberate tradeoffs

- No fake equalizer and no `.wsz` skins: both would displace real functionality
  or theme integration without improving recognizability.
- Spotify remains a Connect transport; EQ/ReplayGain/seek semantics are those
  NPLAY supports. Disabled states are exposed instead of pretending.
- Local playlists/favorites/ratings reuse NPLAY's SQLite store so terminal and
  GUI remain consistent.
- The visualizer deliberately has an honest unavailable state when CAVA is not
  installed. Resource testing may use a synthetic CAVA harness, clearly labelled.
