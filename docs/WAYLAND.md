# NIRUAMP on Omarchy and Wayland

Details behind the short summary in the [README](../README.md).

## Install on Omarchy

NIRUAMP follows the Omarchy theme. Since 0.6 it is an X11 program and runs
through XWayland (Omarchy and Hyprland ship it). Install on Omarchy (Arch)
like anywhere else; NPLAY comes first:

```sh
git clone https://github.com/nirucon/nplay.git && (cd nplay && ./install.sh)
git clone https://github.com/nirucon/niruamp.git && (cd niruamp && ./install.sh)
niruamp
```

The prebuilt binary is static and needs no extra libraries; `cava` is
optional for the spectrum.

## What NIRUAMP does on Wayland

- **Backend.** X11 everywhere: XWayland on a Wayland desktop. (0.5 had a
  native Wayland backend through winit and OpenGL; 0.6 replaced that stack
  with a software canvas of about 8 MB, so `NIRUAMP_BACKEND` is gone. Under
  fractional scaling XWayland may scale the window.) The NIRU session uses
  X11, as before.
- **Theme.** With **Theme → Auto** NIRUAMP follows the current Omarchy theme and
  switches live with `omarchy-theme-set`. It reads the theme's `colors.toml`
  (Omarchy 3.4+; `~/.local/state/omarchy/current/theme` in Omarchy 4,
  `~/.config/omarchy/current/theme` in 3.x), or `alacritty.toml` for older
  themes. `background`/`foreground` are used as given, `accent` drives focus,
  the LED and the spectrum, `selection` the borders, and the dimmed foreground
  the secondary text (lifted to at least 4:1 contrast when a theme makes it too
  faint). Light themes work like NIRUX's light themes. **Omarchy · current
  theme** and **NIRUX · current theme** pin one source.
- **Hyprland.** Only the compositor places and stacks windows, so NIRUAMP
  talks to Hyprland over its IPC socket (also through XWayland). Before the window opens it
  registers an in-memory window rule (gone after a Hyprland reload, registered
  again at the next start) so NIRUAMP opens floating at its saved size and
  position, without flashing tiled first. **Always on top** pins the window
  (Hyprland's pin: above other windows on every workspace). Minimize to tray
  parks it on the special workspace `special:niruamp`; the tray, or starting
  `niruamp` again, brings it back. The spectrum pauses when the window is on a
  workspace that is not shown, exactly as under NIRUWM. Hyprland 0.55+ (Lua
  config) is used fully; older Hyprland gets the classic dispatchers after the
  window has mapped.
- **Other Wayland compositors** decide position and stacking themselves; the
  **Always on top** setting is disabled there, and the spectrum keeps running
  while the window is hidden (the compositor still stops drawing it).
- **Tray.** The tray icon is a StatusNotifierItem; Omarchy's shell, Waybar and
  other SNI hosts show it.
- **Responsiveness.** Off a real X server NIRUAMP does not wait for vsync: a
  compositor sends no frame callbacks to a hidden window, and a swap that
  waited for one would stop the app from answering the compositor's pings
  (Hyprland's "Application not responding"). NIRUAMP paces its own frames.

## Theme resolution

By default (**Theme → Auto**) NIRUAMP follows the desktop: in the NIRU session
always NIRUX, elsewhere the current Omarchy theme when there is one (see
above), otherwise NIRUX's configuration if it
exists. The NIRUX rules are:

1. `$XDG_CONFIG_HOME/niruwm/config.toml` selects the theme.
2. A custom `$XDG_CONFIG_HOME/niruwm/themes/NAME.toml` wins, exactly like NIRUX.
3. Otherwise the compiled builtin palette is used and a valid schema-1
   `appearance.toml` may override semantic roles plus font, spacing, focus width
   and reduced motion.
4. Invalid appearance files are ignored; a malformed custom theme is reported
   without replacing a working appearance.

Choose **Settings → Theme → NIRUAMP Classic** for the standalone fallback. Theme
and appearance changes apply live after a debounce.
