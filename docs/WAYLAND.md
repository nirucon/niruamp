# NIRUAMP on Omarchy and Wayland

Details behind the short summary in the [README](../README.md).

## Install on Omarchy

NIRUAMP 0.5 runs natively on Wayland and follows the Omarchy theme. Install on
Omarchy (Arch) like anywhere else; NPLAY comes first:

```sh
git clone https://github.com/nirucon/nplay.git && (cd nplay && ./install.sh)
git clone https://github.com/nirucon/niruamp.git && (cd niruamp && ./install.sh)
niruamp
```

Omarchy already ships the runtime libraries (`wayland`, `libxkbcommon`, Mesa)
and `cava`. On other Arch systems: `sudo pacman -S wayland libxkbcommon
libxkbcommon-x11 mesa`.

## What NIRUAMP does on Wayland

- **Backend.** Native Wayland whenever `WAYLAND_DISPLAY` is set outside the NIRU
  session, so scaling (including fractional scales such as 1.25) stays crisp.
  `NIRUAMP_BACKEND=x11` forces XWayland, `NIRUAMP_BACKEND=wayland` forces
  Wayland. The NIRU session always uses X11, as before.
- **Theme.** With **Theme → Auto** NIRUAMP follows the current Omarchy theme and
  switches live with `omarchy-theme-set`. It reads the theme's `colors.toml`
  (Omarchy 3.4+; `~/.local/state/omarchy/current/theme` in Omarchy 4,
  `~/.config/omarchy/current/theme` in 3.x), or `alacritty.toml` for older
  themes. `background`/`foreground` are used as given, `accent` drives focus,
  the LED and the spectrum, `selection` the borders, and the dimmed foreground
  the secondary text (lifted to at least 4:1 contrast when a theme makes it too
  faint). Light themes work like NIRUX's light themes. **Omarchy · current
  theme** and **NIRUX · current theme** pin one source.
- **Hyprland.** Wayland lets only the compositor place and stack windows, so
  NIRUAMP talks to Hyprland over its IPC socket. Before the window opens it
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
