//! Hyprland integration over its UNIX sockets (no `hyprctl` subprocess).
//!
//! Wayland gives a client no say over its position, stacking or workspace,
//! so on Hyprland NIRUAMP asks the compositor directly:
//!
//! * before the window exists, a window rule makes it open floating, at the
//!   saved size and position, pinned when "Always on top" is on — so it never
//!   flashes tiled first;
//! * "Always on top" toggles Hyprland's pin, hiding to the tray moves the
//!   window to the special workspace `special:niruamp`, and showing brings it
//!   back to the focused workspace;
//! * the window position is read back from the compositor when it is saved;
//! * the event socket tells the visibility watcher when the window leaves the
//!   shown workspace, so the spectrum pauses exactly as under NIRUWM.
//!
//! Hyprland 0.55+ speaks Lua (`eval`, `dispatch hl.dsp…`). Older releases get
//! the classic dispatchers (`setfloating`, `pin`, `movewindowpixel` …) after
//! the window has mapped.
use serde::Deserialize;
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

pub const CLASS: &str = "niruamp";
/// Where a window hidden to the tray waits.
pub const HIDDEN_WORKSPACE: &str = "special:niruamp";

#[derive(Clone, Debug)]
pub struct Hyprland {
    dir: PathBuf,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct WorkspaceRef {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub name: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Client {
    pub address: String,
    #[serde(default)]
    pub mapped: bool,
    #[serde(default)]
    pub hidden: bool,
    pub at: [f32; 2],
    pub size: [f32; 2],
    #[serde(default)]
    pub workspace: WorkspaceRef,
    #[serde(default)]
    pub floating: bool,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub fullscreen: i64,
    #[serde(default)]
    pub monitor: i64,
    #[serde(default)]
    pub class: String,
    #[serde(default, rename = "initialClass")]
    pub initial_class: String,
    #[serde(default)]
    pub pid: i64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Monitor {
    pub id: i64,
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    #[serde(default = "one")]
    pub scale: f32,
    #[serde(default, rename = "activeWorkspace")]
    pub active_workspace: WorkspaceRef,
    #[serde(default, rename = "specialWorkspace")]
    pub special_workspace: WorkspaceRef,
}

fn one() -> f32 {
    1.0
}

impl Monitor {
    /// Logical (layout) size; `width`/`height` are physical pixels.
    pub fn logical_size(&self) -> (f32, f32) {
        let s = if self.scale > 0.0 { self.scale } else { 1.0 };
        (self.width / s, self.height / s)
    }
    pub fn contains(&self, x: f32, y: f32) -> bool {
        let (w, h) = self.logical_size();
        x >= self.x && y >= self.y && x < self.x + w && y < self.y + h
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Workspace {
    pub id: i64,
    #[serde(default, rename = "hasfullscreen")]
    pub has_fullscreen: bool,
}

/// Lua string literal.
pub fn lua_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Hyprland addresses are `0x` + hex; anything else is never sent back.
pub fn valid_address(address: &str) -> bool {
    address.len() > 2
        && address.len() <= 20
        && address.starts_with("0x")
        && address[2..].bytes().all(|b| b.is_ascii_hexdigit())
}

impl Hyprland {
    /// The running Hyprland instance NIRUAMP's Wayland session belongs to.
    pub fn detect() -> Option<Self> {
        let sig = crate::session::hyprland()?;
        let runtime = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .filter(|p| !p.as_os_str().is_empty());
        let candidates = runtime
            .into_iter()
            .map(|r| r.join("hypr").join(&sig))
            .chain(std::iter::once(PathBuf::from("/tmp/hypr").join(&sig)));
        for dir in candidates {
            if dir.join(".socket.sock").exists() {
                return Some(Self { dir });
            }
        }
        None
    }

    pub fn event_socket(&self) -> PathBuf {
        self.dir.join(".socket2.sock")
    }

    pub fn request(&self, command: &str) -> io::Result<String> {
        let mut stream = UnixStream::connect(self.dir.join(".socket.sock"))?;
        stream.set_read_timeout(Some(Duration::from_millis(1500)))?;
        stream.set_write_timeout(Some(Duration::from_millis(1500)))?;
        stream.write_all(command.as_bytes())?;
        let mut out = Vec::new();
        stream.read_to_end(&mut out)?;
        Ok(String::from_utf8_lossy(&out).into_owned())
    }

    fn json<T: for<'de> Deserialize<'de>>(&self, what: &str) -> Option<T> {
        let text = self.request(&format!("j/{what}")).ok()?;
        serde_json::from_str(&text).ok()
    }

    pub fn clients(&self) -> Option<Vec<Client>> {
        self.json("clients")
    }
    pub fn monitors(&self) -> Option<Vec<Monitor>> {
        self.json("monitors")
    }
    pub fn workspaces(&self) -> Option<Vec<Workspace>> {
        self.json("workspaces")
    }
    pub fn active_workspace(&self) -> Option<WorkspaceRef> {
        self.json("activeworkspace")
    }

    /// This process's NIRUAMP window.
    pub fn own_client(&self) -> Option<Client> {
        let pid = std::process::id() as i64;
        self.clients()?.into_iter().find(|c| {
            c.pid == pid
                && (c.class == CLASS || c.initial_class == CLASS)
                && valid_address(&c.address)
        })
    }

    fn ok(reply: io::Result<String>) -> Result<(), String> {
        match reply {
            Ok(r) if r.trim() == "ok" => Ok(()),
            Ok(r) => Err(r.trim().chars().take(200).collect()),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Lua configuration (Hyprland 0.55+): `eval` answers `ok`.
    pub fn speaks_lua(&self) -> bool {
        Self::ok(self.request("eval return true")).is_ok()
    }

    pub fn eval(&self, lua: &str) -> Result<(), String> {
        Self::ok(self.request(&format!("eval {lua}")))
    }

    pub fn dispatch(&self, args: &str) -> Result<(), String> {
        Self::ok(self.request(&format!("dispatch {args}")))
    }

    /// The monitor holding a saved top-left corner, and the corner in that
    /// monitor's local coordinates (window rules move monitor-locally).
    pub fn placement(&self, x: f32, y: f32, w: f32, h: f32) -> Option<(String, f32, f32)> {
        let monitors = self.monitors()?;
        // The window must stay reachable: its title strip on the monitor.
        let probe = (x + w.min(120.0) * 0.5, y + 8.0_f32.min(h));
        let m = monitors.iter().find(|m| m.contains(probe.0, probe.1))?;
        Some((m.name.clone(), x - m.x, y - m.y))
    }

    /// Register (or replace) the rule that opens NIRUAMP floating at its
    /// saved geometry. Rules live in the compositor's memory only; a config
    /// reload drops them and the next start registers them again.
    pub fn register_placement_rule(
        &self,
        size: (f32, f32),
        position: Option<(f32, f32)>,
        pinned: bool,
    ) -> Result<(), String> {
        let mut fields = vec![
            format!("match = {{ class = {} }}", lua_str(&format!("^{CLASS}$"))),
            "float = true".to_string(),
            format!(
                "size = {}",
                lua_str(&format!("{} {}", size.0.round(), size.1.round()))
            ),
        ];
        if let Some((x, y)) = position {
            if let Some((monitor, lx, ly)) = self.placement(x, y, size.0, size.1) {
                fields.push(format!("monitor = {}", lua_str(&monitor)));
                fields.push(format!(
                    "move = {}",
                    lua_str(&format!("{} {}", lx.round(), ly.round()))
                ));
            }
        }
        if pinned {
            fields.push("pin = true".to_string());
        }
        let lua = format!(
            "if niruamp_placement_rule then niruamp_placement_rule:set_enabled(false) end \
             niruamp_placement_rule = hl.window_rule({{ name = {}, {} }})",
            lua_str("niruamp-placement"),
            fields.join(", ")
        );
        self.eval(&lua)
    }

    fn selector(address: &str) -> String {
        lua_str(&format!("address:{address}"))
    }

    /// Bring a window that mapped tiled into the floating geometry
    /// (Hyprland without Lua, or a rule that did not apply).
    pub fn float_existing(
        &self,
        client: &Client,
        size: (f32, f32),
        position: Option<(f32, f32)>,
        pinned: bool,
        lua: bool,
    ) {
        let a = &client.address;
        if !valid_address(a) {
            return;
        }
        let place = position.filter(|&(x, y)| self.placement(x, y, size.0, size.1).is_some());
        if lua {
            let w = Self::selector(a);
            if !client.floating {
                let _ = self.dispatch(&format!(
                    "hl.dsp.window.float({{ action = \"enable\", window = {w} }})"
                ));
                let _ = self.dispatch(&format!(
                    "hl.dsp.window.resize({{ x = {}, y = {}, relative = false, window = {w} }})",
                    size.0.round(),
                    size.1.round()
                ));
                match place {
                    Some((x, y)) => {
                        let _ = self.dispatch(&format!(
                            "hl.dsp.window.move({{ x = {}, y = {}, relative = false, window = {w} }})",
                            x.round(),
                            y.round()
                        ));
                    }
                    None => {
                        let _ = self.dispatch(&format!("hl.dsp.window.center({{ window = {w} }})"));
                    }
                }
            }
        } else {
            if !client.floating {
                let _ = self.dispatch(&format!("setfloating address:{a}"));
                let _ = self.dispatch(&format!(
                    "resizewindowpixel exact {} {},address:{a}",
                    size.0.round(),
                    size.1.round()
                ));
                match place {
                    Some((x, y)) => {
                        let _ = self.dispatch(&format!(
                            "movewindowpixel exact {} {},address:{a}",
                            x.round(),
                            y.round()
                        ));
                    }
                    None => {
                        let _ = self.dispatch(&format!("centerwindow address:{a}"));
                    }
                }
            }
        }
        if pinned != client.pinned {
            self.set_pinned(client, pinned, lua);
        }
    }

    pub fn set_pinned(&self, client: &Client, pinned: bool, lua: bool) {
        let a = &client.address;
        if !valid_address(a) {
            return;
        }
        if lua {
            let _ = self.dispatch(&format!(
                "hl.dsp.window.pin({{ action = {}, window = {} }})",
                lua_str(if pinned { "enable" } else { "disable" }),
                Self::selector(a)
            ));
        } else if client.pinned != pinned {
            // The classic dispatcher only toggles.
            let _ = self.dispatch(&format!("pin address:{a}"));
        }
    }

    /// Hide to the tray: park the window on NIRUAMP's special workspace.
    pub fn hide(&self, client: &Client, lua: bool) {
        let a = &client.address;
        if !valid_address(a) {
            return;
        }
        if client.pinned {
            // A pinned window cannot change workspace.
            self.set_pinned(client, false, lua);
        }
        if lua {
            let _ = self.dispatch(&format!(
                "hl.dsp.window.move({{ workspace = {}, follow = false, window = {} }})",
                lua_str(HIDDEN_WORKSPACE),
                Self::selector(a)
            ));
        } else {
            let _ = self.dispatch(&format!(
                "movetoworkspacesilent {HIDDEN_WORKSPACE},address:{a}"
            ));
        }
    }

    /// Show from the tray: back on the focused workspace, focused, and
    /// pinned again when "Always on top" is on.
    pub fn show(&self, client: &Client, pinned: bool, lua: bool) {
        let a = &client.address;
        if !valid_address(a) {
            return;
        }
        let target = self.active_workspace().map(|w| w.id).filter(|&id| id > 0);
        if client.workspace.name == HIDDEN_WORKSPACE || client.workspace.id < 0 {
            if let Some(id) = target {
                if lua {
                    let _ = self.dispatch(&format!(
                        "hl.dsp.window.move({{ workspace = {id}, follow = false, window = {} }})",
                        Self::selector(a)
                    ));
                } else {
                    let _ = self.dispatch(&format!("movetoworkspacesilent {id},address:{a}"));
                }
            }
        }
        if lua {
            let _ = self.dispatch(&format!(
                "hl.dsp.focus({{ window = {} }})",
                Self::selector(a)
            ));
        } else {
            let _ = self.dispatch(&format!("focuswindow address:{a}"));
        }
        if pinned {
            if let Some(now) = self.own_client() {
                if !now.pinned {
                    self.set_pinned(&now, true, lua);
                }
            }
        }
    }
}

/// Whether the window can be seen, from Hyprland's view of it.
pub fn client_visible(client: &Client, monitors: &[Monitor], workspaces: &[Workspace]) -> bool {
    if !client.mapped || client.hidden {
        return false;
    }
    let shown = client.pinned
        || monitors.iter().any(|m| {
            m.id == client.monitor
                && (m.active_workspace.id == client.workspace.id
                    || (client.workspace.id < 0 && m.special_workspace.id == client.workspace.id))
        });
    if !shown {
        return false;
    }
    // A fullscreen window on the same workspace covers everything else.
    let covered = client.fullscreen == 0
        && workspaces
            .iter()
            .any(|w| w.id == client.workspace.id && w.has_fullscreen);
    !covered
}

/// Event-socket lines that can change whether NIRUAMP is visible.
pub fn affects_visibility(line: &str) -> bool {
    let event = line.split(">>").next().unwrap_or("");
    matches!(
        event,
        "workspace"
            | "workspacev2"
            | "focusedmon"
            | "focusedmonv2"
            | "activespecial"
            | "activespecialv2"
            | "movewindow"
            | "movewindowv2"
            | "openwindow"
            | "closewindow"
            | "fullscreen"
            | "changefloatingmode"
            | "pin"
            | "minimized"
            | "monitoradded"
            | "monitoraddedv2"
            | "monitorremoved"
            | "monitorremovedv2"
            | "moveworkspace"
            | "moveworkspacev2"
            | "destroyworkspace"
            | "destroyworkspacev2"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(id: i64, active: i64, special: i64) -> Monitor {
        Monitor {
            id,
            name: format!("M{id}"),
            x: 1920.0 * id as f32,
            y: 0.0,
            width: 2880.0,
            height: 1800.0,
            scale: 1.5,
            active_workspace: WorkspaceRef {
                id: active,
                name: active.to_string(),
            },
            special_workspace: WorkspaceRef {
                id: special,
                name: String::new(),
            },
        }
    }

    fn client(ws: i64, mon: i64) -> Client {
        serde_json::from_str(&format!(
            r#"{{"address":"0x55a9","mapped":true,"hidden":false,"at":[10,20],"size":[640,420],
            "workspace":{{"id":{ws},"name":"{ws}"}},"floating":true,"pinned":false,"fullscreen":0,
            "monitor":{mon},"class":"niruamp","initialClass":"niruamp","pid":1,"extra":"ignored"}}"#
        ))
        .unwrap()
    }

    #[test]
    fn workspace_visibility() {
        let monitors = [monitor(0, 2, 0), monitor(1, 5, 0)];
        assert!(client_visible(&client(2, 0), &monitors, &[]));
        assert!(!client_visible(&client(3, 0), &monitors, &[]));
        assert!(client_visible(&client(5, 1), &monitors, &[]));
        let mut pinned = client(3, 0);
        pinned.pinned = true;
        assert!(client_visible(&pinned, &monitors, &[]));
    }

    #[test]
    fn special_workspace_and_fullscreen() {
        let hidden = client(-98, 0);
        assert!(!client_visible(&hidden, &[monitor(0, 2, 0)], &[]));
        assert!(client_visible(&hidden, &[monitor(0, 2, -98)], &[]));
        let covered = [Workspace {
            id: 2,
            has_fullscreen: true,
        }];
        assert!(!client_visible(
            &client(2, 0),
            &[monitor(0, 2, 0)],
            &covered
        ));
        let mut own_fullscreen = client(2, 0);
        own_fullscreen.fullscreen = 2;
        assert!(client_visible(
            &own_fullscreen,
            &[monitor(0, 2, 0)],
            &covered
        ));
    }

    #[test]
    fn logical_monitor_geometry() {
        let m = monitor(1, 1, 0);
        assert_eq!(m.logical_size(), (1920.0, 1200.0));
        assert!(m.contains(1920.0 + 100.0, 50.0));
        assert!(!m.contains(100.0, 50.0));
    }

    #[test]
    fn visibility_events() {
        assert!(affects_visibility("workspacev2>>3,3"));
        assert!(affects_visibility("movewindowv2>>55a9,3,3"));
        assert!(!affects_visibility("activewindowv2>>55a9"));
        assert!(!affects_visibility("windowtitlev2>>55a9,x"));
    }

    #[test]
    fn lua_quoting_and_addresses() {
        assert_eq!(lua_str("a\"b\\c"), "\"a\\\"b\\\\c\"");
        assert_eq!(lua_str("x\u{1}y"), "\"xy\"");
        assert!(valid_address("0x55a97327d200"));
        assert!(!valid_address("0x55a9 ; rm"));
        assert!(!valid_address("55a9"));
    }
}
