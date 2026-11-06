//! Whether the window can actually be seen.
//!
//! egui reports minimized windows, but not a window that the window manager
//! unmapped (NIRUWM does that for every client on a workspace that is not
//! shown) or one that is fully covered. A small thread with its own X
//! connection selects structure, property and visibility events on our window
//! and blocks in `wait_for_event`; it wakes only when one of those changes.
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::thread;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    self, AtomEnum, ChangeWindowAttributesAux, ConnectionExt, EventMask, MapState,
};
use x11rb::protocol::Event;

const UNMAPPED: u8 = 1;
const WM_HIDDEN: u8 = 2;
const OBSCURED: u8 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisEvent {
    Mapped,
    Unmapped,
    /// `VisibilityNotify`: true when fully obscured.
    Obscured(bool),
    /// `_NET_WM_STATE` contains `_NET_WM_STATE_HIDDEN`.
    WmHidden(bool),
}

/// Pure state transition, kept separate so it can be tested without X.
pub fn apply(state: u8, event: VisEvent) -> u8 {
    match event {
        // A freshly mapped window is reported unobscured until the server
        // says otherwise.
        VisEvent::Mapped => state & !(UNMAPPED | OBSCURED),
        VisEvent::Unmapped => state | UNMAPPED,
        VisEvent::Obscured(true) => state | OBSCURED,
        VisEvent::Obscured(false) => state & !OBSCURED,
        VisEvent::WmHidden(true) => state | WM_HIDDEN,
        VisEvent::WmHidden(false) => state & !WM_HIDDEN,
    }
}

pub fn is_visible(state: u8) -> bool {
    state == 0
}

pub struct Visibility {
    state: Arc<AtomicU8>,
}

impl Visibility {
    /// Watch the X11 window `window`. Returns `None` without an X display
    /// (the caller then treats the window as visible, the 0.3 behaviour).
    pub fn spawn(window: u32, ctx: egui::Context) -> Option<Self> {
        let (conn, _) = x11rb::connect(None).ok()?;
        let mask =
            EventMask::STRUCTURE_NOTIFY | EventMask::PROPERTY_CHANGE | EventMask::VISIBILITY_CHANGE;
        conn.change_window_attributes(window, &ChangeWindowAttributesAux::new().event_mask(mask))
            .ok()?
            .check()
            .ok()?;
        let wm_state = atom(&conn, b"_NET_WM_STATE")?;
        let wm_hidden = atom(&conn, b"_NET_WM_STATE_HIDDEN")?;
        let mut initial = 0;
        if let Ok(attrs) = conn.get_window_attributes(window).ok()?.reply() {
            if attrs.map_state != MapState::VIEWABLE {
                initial = apply(initial, VisEvent::Unmapped);
            }
        }
        initial = apply(
            initial,
            VisEvent::WmHidden(has_hidden(&conn, window, wm_state, wm_hidden)),
        );
        let state = Arc::new(AtomicU8::new(initial));
        let shared = state.clone();
        thread::Builder::new()
            .name("niruamp-visibility".into())
            .spawn(move || loop {
                let event = match conn.wait_for_event() {
                    Ok(event) => event,
                    Err(_) => return,
                };
                let change = match event {
                    Event::MapNotify(e) if e.window == window => VisEvent::Mapped,
                    Event::UnmapNotify(e) if e.window == window => VisEvent::Unmapped,
                    Event::VisibilityNotify(e) if e.window == window => {
                        VisEvent::Obscured(e.state == xproto::Visibility::FULLY_OBSCURED)
                    }
                    Event::PropertyNotify(e) if e.window == window && e.atom == wm_state => {
                        VisEvent::WmHidden(has_hidden(&conn, window, wm_state, wm_hidden))
                    }
                    Event::DestroyNotify(e) if e.window == window => return,
                    _ => continue,
                };
                let old = shared.load(Ordering::Acquire);
                let new = apply(old, change);
                if new != old {
                    shared.store(new, Ordering::Release);
                    ctx.request_repaint();
                }
            })
            .ok()?;
        Some(Self { state })
    }

    /// Watch NIRUAMP's window through Hyprland's event socket. The thread
    /// sleeps in a blocking read; a burst of events (a workspace switch emits
    /// several) is coalesced into one look at the compositor state.
    pub fn spawn_hyprland(hypr: crate::hyprland::Hyprland, ctx: egui::Context) -> Option<Self> {
        use crate::hyprland::{affects_visibility, client_visible};
        use std::io::{BufRead, BufReader};
        let stream = std::os::unix::net::UnixStream::connect(hypr.event_socket()).ok()?;
        let state = Arc::new(AtomicU8::new(0));
        let shared = state.clone();
        let evaluate = move |shared: &AtomicU8| {
            let visible = match (hypr.own_client(), hypr.monitors(), hypr.workspaces()) {
                (Some(client), Some(monitors), Some(workspaces)) => {
                    client_visible(&client, &monitors, &workspaces)
                }
                // Not mapped yet, or the compositor did not answer: draw.
                _ => true,
            };
            let new = if visible { 0 } else { UNMAPPED };
            if shared.swap(new, Ordering::AcqRel) != new {
                ctx.request_repaint();
            }
        };
        thread::Builder::new()
            .name("niruamp-visibility".into())
            .spawn(move || {
                evaluate(&shared);
                let mut reader = BufReader::new(stream);
                let mut line = String::new();
                loop {
                    line.clear();
                    let _ = reader.get_ref().set_read_timeout(None);
                    match reader.read_line(&mut line) {
                        Ok(0) | Err(_) => return,
                        Ok(_) => {}
                    }
                    if !affects_visibility(line.trim_end()) {
                        continue;
                    }
                    // Drain the rest of the burst, then look once.
                    let _ = reader
                        .get_ref()
                        .set_read_timeout(Some(std::time::Duration::from_millis(40)));
                    loop {
                        line.clear();
                        match reader.read_line(&mut line) {
                            Ok(0) => return,
                            Ok(_) => continue,
                            Err(_) => break,
                        }
                    }
                    evaluate(&shared);
                }
            })
            .ok()?;
        Some(Self { state })
    }

    pub fn visible(&self) -> bool {
        is_visible(self.state.load(Ordering::Acquire))
    }
}

fn atom(conn: &impl Connection, name: &[u8]) -> Option<u32> {
    Some(conn.intern_atom(false, name).ok()?.reply().ok()?.atom)
}

fn has_hidden(conn: &impl Connection, window: u32, wm_state: u32, hidden: u32) -> bool {
    conn.get_property(false, window, wm_state, AtomEnum::ATOM, 0, 64)
        .ok()
        .and_then(|c| c.reply().ok())
        .and_then(|r| r.value32().map(|mut v| v.any(|a| a == hidden)))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_switch_unmaps_and_restores() {
        let mut s = 0;
        assert!(is_visible(s));
        s = apply(s, VisEvent::Unmapped);
        assert!(!is_visible(s));
        s = apply(s, VisEvent::Mapped);
        assert!(is_visible(s));
    }

    #[test]
    fn fully_obscured_is_not_visible_until_exposed_or_remapped() {
        let mut s = apply(0, VisEvent::Obscured(true));
        assert!(!is_visible(s));
        s = apply(s, VisEvent::Obscured(false));
        assert!(is_visible(s));
        s = apply(s, VisEvent::Obscured(true));
        s = apply(s, VisEvent::Unmapped);
        s = apply(s, VisEvent::Mapped);
        assert!(is_visible(s), "a remapped window starts unobscured");
    }

    #[test]
    fn wm_hidden_state_is_independent_of_mapping() {
        let mut s = apply(0, VisEvent::WmHidden(true));
        s = apply(s, VisEvent::Mapped);
        assert!(!is_visible(s));
        s = apply(s, VisEvent::WmHidden(false));
        assert!(is_visible(s));
    }

    #[test]
    fn x11_unmap_is_observed_on_a_real_display() {
        // Runs only where an X server is reachable (Xvfb in the harness).
        let Ok((conn, screen)) = x11rb::connect(None) else {
            return;
        };
        let root = conn.setup().roots[screen].root;
        let win = conn.generate_id().unwrap();
        conn.create_window(
            0,
            win,
            root,
            0,
            0,
            64,
            64,
            0,
            xproto::WindowClass::INPUT_OUTPUT,
            0,
            &xproto::CreateWindowAux::new(),
        )
        .unwrap();
        conn.map_window(win).unwrap();
        conn.flush().unwrap();
        conn.get_input_focus().unwrap().reply().unwrap();
        let vis = Visibility::spawn(win, egui::Context::default()).expect("watcher");
        let wait = |want: bool| {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            while vis.visible() != want && std::time::Instant::now() < deadline {
                thread::sleep(std::time::Duration::from_millis(10));
            }
            vis.visible() == want
        };
        assert!(wait(true));
        conn.unmap_window(win).unwrap();
        conn.flush().unwrap();
        assert!(wait(false));
        conn.map_window(win).unwrap();
        conn.flush().unwrap();
        assert!(wait(true));
        conn.destroy_window(win).unwrap();
        conn.flush().unwrap();
    }
}
