# NIRUAMP

Winamp-inspired graphical frontend for NPLAY, written in Rust.

**Status: alpha** · Part of the [NIRUX](https://github.com/nirucon/nirux) suite · [MIT License](LICENSE)

![NIRUAMP](docs/screenshots/main.png)

More screenshots: [light theme](docs/screenshots/main-light.png),
[search](docs/screenshots/search.png), [settings](docs/screenshots/settings.png),
[windowshade](docs/screenshots/windowshade.png).

## About

NIRUAMP is the classic, compact player window I missed: a hardware-panel deck,
LED time, spectrum display and a dense playlist, in the spirit of Winamp but
with original vector artwork and live NIRUX theming. It is not affiliated
with Winamp.

**NIRUAMP is a frontend only and cannot run without
[NPLAY](https://github.com/nirucon/nplay).** NPLAY owns playback, queue,
providers, library, statistics, notifications and MPRIS; NIRUAMP talks to
NPLAY's local control service over a private Unix socket. NPLAY remains a
complete standalone terminal player with or without NIRUAMP and NIRUX.

It runs on X11 in the NIRUX session (NIRUWM) and natively on Wayland, for
example on Omarchy/Hyprland.

## Features

- Winamp-style deck: LED clock, seek and volume, transport, shuffle/repeat,
  attached playlist and queue panels, windowshade mode, always-on-top, a
  compact title bar and remembered geometry.
- Fullscreen visual mode (`F11`): black screen, centred track, artist and
  album, themed CAVA bars and progress.
- Real CAVA visualization at up to 30 fps (30/20/15 in Settings), pushed by
  NPLAY 1.8's visualization stream; it pauses (and NPLAY stops CAVA) while
  the window is hidden, covered, minimized or in the tray. No fake bars: a
  clear "unavailable" state otherwise.
- Async federated search across local music, Navidrome, Sveriges Radio,
  YouTube, Spotify and custom radio, with stale-result protection.
- Browse artists, albums, folders, playlists, favorites, history, radio,
  provider albums/playlists/artists and Spotify devices.
- Queue play/add/remove/drag-reorder/clear/save, local playlists, favorites,
  ratings, custom radio, and drag-and-drop of local media and streams.
  Containers (albums, playlists, folders) always resolve to playable tracks.
- Optional theme-aware tray icon (StatusNotifierItem): minimize to tray,
  menu, middle-click play/pause, scroll for volume.
- Theme **Auto**: follows NIRUX in the NIRUX session and Omarchy elsewhere,
  live; NIRUAMP Classic as the standalone fallback.
- Frugal: one frame scheduler, no repaint while hidden, no polling threads;
  measured in [docs/VALIDATION.md](docs/VALIDATION.md).
- Optional Winamp-inspired startup sound (embedded, can be disabled).

## Requirements

- Linux, x86_64, X11 or Wayland. Verified live on Debian 13 with NIRUX and on
  Omarchy 4 (Hyprland).
- **NPLAY 1.7.3 or newer** with its prerequisites (`mpv`, Python 3).
  NPLAY 1.8.0+ is recommended (visualization stream instead of polling, about
  a third of NPLAY's CPU), and 1.8.1+ adds **Settings → Sources → Spotify →
  AUTHORIZE LOCAL**.
- The prebuilt binary needs glibc 2.35+ (Debian 12+) and `libxkbcommon-x11`,
  `libx11`, `libgl`/`libegl` at runtime. Building from source needs Rust 1.85+.
- Optional: `cava` (visualization), `yt-dlp`, `librespot`, Navidrome
  credentials, Spotify Premium, a StatusNotifierItem tray host.

## Installation

### With NIRUX

NIRUAMP is an optional extra, not part of a normal `nirux install`:

```sh
nirux extras install niruamp
nirux extras update            # later: update selected extras to the pinned version
```

This installs or updates the pinned NPLAY first, then NIRUAMP.

### Standalone

Install NPLAY first, then:

```sh
git clone https://github.com/nirucon/niruamp.git
cd niruamp
./install.sh
niruamp
```

The installer refuses to continue when NPLAY is missing or older than 1.7.3
(`NIRUAMP_SKIP_NPLAY_CHECK=1` skips that, for packaging experiments only). It
downloads the published x86_64 release binary when possible and builds with
cargo otherwise, then installs `~/.local/bin/niruamp`, a desktop entry and an
icon.

```sh
./install.sh --prebuilt  # force the published binary
./install.sh --source    # always build with cargo
```

Manual install from the release archive:

```sh
tar -xzf niruamp-0.5.2-x86_64-linux-gnu.tar.gz
install -m755 niruamp ~/.local/bin/niruamp
```

On Omarchy/Arch the steps are the same; see [docs/WAYLAND.md](docs/WAYLAND.md).

## Usage

```sh
niruamp            # start, or activate the running window
niruamp --version
niruamp --help
```

NIRUAMP connects to `$XDG_RUNTIME_DIR/nplay/control.sock` and starts the
installed `nplay --serve` itself when no compatible service is running. A GUI
never stops a service it did not start; a service it started stops on normal
exit unless **Keep playing after GUI exit** is on (applies to the next service
launch). You can also run the backend by hand:

```sh
nplay --serve
nplay --serve --service-socket /path/to/socket --idle-exit 25
```

| Key | Action |
|---|---|
| `Space` | Play / pause |
| `N` / `P` | Next / previous |
| `S` | Stop |
| `Left` / `Right` | Seek 5 s (`Shift`: 30 s) |
| `+` / `-` / `M` | Volume / mute |
| `R` / `H` | Repeat / shuffle |
| `/` or `Ctrl+F` | Search |
| `B` | Browse |
| `F` | Favorite the current track |
| `1` … `5` | Playlist / queue / browse / settings / about |
| `W` / `T` | Windowshade / always-on-top |
| `F11` | Fullscreen visual mode (`Esc` or `F11` leaves it) |
| `Tab` / `Shift+Tab`, `Enter` | Move focus, activate the focused control |
| `Up` / `Down` | Move selection |
| `Alt+Up` / `Alt+Down` | Reorder the queue |
| `Delete` | Remove the selected queue item |
| `Esc` | Back / leave a text field |

Shortcuts never steal typing from text fields. Right-click a track for queue,
playlist, favorite and detail actions. Drag local media onto the window to
play it; extra files are queued. Minimize hides to the tray when a tray host
exists; left-click the tray icon to restore.

## Configuration

Settings are changed in the app (`4` or the settings tab) and saved to
`~/.config/niruamp/config.toml`: theme, scale, always-on-top, keep playing,
visualization on/off and frame rate, pausing the hidden visualization,
startup sound, tray, reduced motion, remaining time, and the `nplay_bin` to
start.

With **Theme → Auto** the palette comes from NIRUX (`~/.config/niruwm/config.toml`,
custom `~/.config/niruwm/themes/NAME.toml`, `appearance.toml` overrides) or,
outside the NIRUX session, the current Omarchy theme. The full resolution
order is in [docs/WAYLAND.md](docs/WAYLAND.md#theme-resolution).

Environment variables:

| Variable | Effect |
|---|---|
| `NIRUAMP_BACKEND=x11` / `wayland` | Force XWayland or native Wayland (default: Wayland when `WAYLAND_DISPLAY` is set outside the NIRUX session) |
| `NPLAY_SERVICE_SOCKET=PATH` | Connect to an existing NPLAY control socket |
| `NPLAY_BIN=PATH` | Start an alternative NPLAY executable |
| `NPLAY_DEV_PATH=/path/to/nplay` | Use an NPLAY source checkout instead of the installed one |
| `NIRUAMP_SKIP_STARTUP_SOUND` | Skip the startup sound (any value) |
| `NIRUAMP_DEBUG_FPS` | Print frames/s and NPLAY messages/s every 5 s |

## Files and paths

| Path | Purpose |
|---|---|
| `~/.local/bin/niruamp` | The binary |
| `~/.local/share/applications/niruamp.desktop` | Desktop entry |
| `~/.local/share/icons/hicolor/scalable/apps/niruamp.svg` | Icon |
| `~/.config/niruamp/config.toml` | Preferences |
| `~/.local/state/niruamp/window.toml` | Window geometry and state |
| `$XDG_RUNTIME_DIR/nplay/control.sock` | NPLAY control service socket |

Navidrome and Spotify credentials stay in NPLAY's `0600` secrets file; NIRUAMP
never reads or displays them.

## Troubleshooting

- **"NPLAY is missing or too old".** Install or update NPLAY (1.7.3+); with
  NIRUX, `nirux extras install niruamp` does it for you.
- **No spectrum.** Install `cava`. Visualization depends on what CAVA can
  capture from your audio setup; live radio and YouTube depend on that too.
- **No tray icon.** Tray support needs a StatusNotifierItem host; without one,
  minimize gives a normal minimized window.
- **Position or always-on-top is not kept on Wayland.** Only Hyprland is
  supported for that; other compositors decide placement themselves.
  Details in [docs/WAYLAND.md](docs/WAYLAND.md).

Known limits:

- No equalizer (NPLAY has no EQ DSP for mpv, and Spotify does not go through
  mpv), and no `.wsz` skins.
- Live radio cannot seek; some source-specific operations are backend limits
  and show as disabled.

## Uninstall

```sh
./uninstall.sh          # keeps NIRUAMP preferences and all NPLAY data
./uninstall.sh --purge  # also removes ~/.config/niruamp and ~/.local/state/niruamp
```

NPLAY is never modified by the uninstaller.

## Building from source

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release --locked
```

NPLAY's service and regression tests live in the NPLAY repository
(`python3 -m unittest discover -s tests -v`). Layout:

- `src/app.rs`: application state, commands, lifecycle, keyboard, layout
- `src/protocol.rs`: JSON-RPC client, framing, reconnect-safe event stream
- `src/platform.rs`: backend discovery/spawn, ownership, single instance
- `src/theme.rs`: NIRUX and Omarchy appearance, watcher
- `src/hyprland.rs`, `src/omarchy.rs`: Wayland/Hyprland and Omarchy integration
- `src/views/`: deck, playlist/queue, browser, settings, immersive mode

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md), [docs/PROTOCOL.md](docs/PROTOCOL.md),
[docs/TESTING.md](docs/TESTING.md) (fake mpv/CAVA harness, Xvfb screenshots)
and [docs/VALIDATION.md](docs/VALIDATION.md) (what was and was not live-tested,
CPU/RSS measurements). Release history: [CHANGELOG.md](CHANGELOG.md).

## Status, disclaimer and how it's made

**Status: alpha.** It installs, runs and is integrated on NIRUX (X11) and on Omarchy/Hyprland (Wayland), but it is young: live playback with real accounts and providers has been tested less than the automated harness, and some features (EQ, skins) are deliberately missing.

NIRUX and every niru* project are made by me, Nicklas Rudolfsson
([@nirucon](https://github.com/nirucon)), for me. They are tailored to my own
needs, preferences and workflows, on my own machines. If they fit yours too,
that's a nice bonus.

- **Free and open source.** Anyone is free to use, study, change and share
  it under the [MIT License](LICENSE).
- **No guarantees.** There is no warranty of any kind. Use it at your own
  risk, read what you install and keep backups of anything that matters.
- **How it's made.** Some of these projects I built from scratch by hand. In
  more recent releases I have used Claude, Anthropic's AI, to do much of the
  heavy lifting under my direction: I decide what to build and how it should
  behave, and I test it on my own machines. It saves me a lot of time, and
  the result is quite OK.
- **Alpha, beta, dev, and maybe forever.** Most things here are alpha, beta
  or dev, and may stay that way. Maybe one day there will be a real stable
  release, but given how fast development moves, and how I am as a person,
  maybe never :)

| Label | Meaning |
|---|---|
| dev | Under active construction; may not build, install or work yet. |
| alpha | Works on my machines, but incomplete or rough; expect breaking changes. |
| beta | In daily use on my machines and mostly complete; rough edges remain. |

## License

MIT License, see [LICENSE](LICENSE). Copyright (c) 2026 Nicklas Rudolfsson.
Original artwork and icons are drawn in code.
