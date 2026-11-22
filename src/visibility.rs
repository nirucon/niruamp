//! Whether the window can actually be seen.
//!
//! The window manager may unmap the window (NIRUWM does that for every client
//! on a workspace that is not shown), mark it hidden, or it may be fully
//! covered. The main loop selects structure, property and visibility events
//! on its own window and folds them into this state; under Hyprland a thread
//! asks the compositor instead.
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::thread;

pub const UNMAPPED: u8 = 1;
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
    /// The state the main loop keeps from its own window's events
    /// (map/unmap, `VisibilityNotify`, `_NET_WM_STATE_HIDDEN`).
    pub fn shared(state: Arc<AtomicU8>) -> Self {
        Self { state }
    }

    /// Watch NIRUAMP's window through Hyprland's event socket. The thread
    /// sleeps in a blocking read; a burst of events (a workspace switch emits
    /// several) is coalesced into one look at the compositor state.
    pub fn spawn_hyprland(
        hypr: crate::hyprland::Hyprland,
        ctx: crate::shell::Wake,
    ) -> Option<Self> {
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
                ctx();
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
}
