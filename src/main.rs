#![allow(
    dead_code,
    clippy::too_many_arguments,
    clippy::collapsible_if,
    clippy::type_complexity,
    clippy::collapsible_match
)]
mod app;
mod config;
mod gui;
mod hyprland;
mod model;
mod omarchy;
mod platform;
mod protocol;
mod schedule;
mod session;
mod shell;
mod skin;
mod sound;
mod spectrum;
mod theme;
mod tray;
mod views;
mod visibility;
mod x;

use crate::app::NiruampApp;
use crate::config::{Preferences, WindowState};
use crate::platform::InstanceKind;

const HELP: &str = "NIRUAMP: Winamp-inspired frontend for NPLAY

Usage: niruamp [--version] [--help] [--class NAME]

  --class NAME          WM_CLASS instance and class (default: niruamp)

Environment:
  NPLAY_SERVICE_SOCKET  Path to an existing NPLAY control socket
  NPLAY_DEV_PATH        NPLAY checkout for development service startup
  NIRUAMP_SCREENSHOT    Save a screenshot of the window to this path
  NIRUAMP_DEBUG_FPS     Print frames/s and NPLAY messages/s every 5 s
  NIRUAMP_TIMING        Print the time to the first frame

NIRUAMP draws with X11 (XWayland under Wayland compositors).
NPLAY 1.7.3 or newer is required (1.8.0+ for the visualization stream).
It remains fully usable standalone.";

fn main() {
    let started = std::time::Instant::now();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("NIRUAMP {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{HELP}");
        return;
    }
    let mut class = "niruamp".to_string();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if let Some(v) = a.strip_prefix("--class=") {
            class = v.to_string();
        } else if a == "--class" || a == "--name" {
            match it.next() {
                Some(v) if !v.is_empty() => class = v.clone(),
                _ => {
                    eprintln!("niruamp: {a} needs a value\n\n{HELP}");
                    std::process::exit(2);
                }
            }
        } else if a.starts_with('-') {
            eprintln!("niruamp: unknown option {a}\n\n{HELP}");
            std::process::exit(2);
        }
    }
    if std::env::var("NIRUAMP_BACKEND").is_ok_and(|v| v.eq_ignore_ascii_case("wayland")) {
        eprintln!("niruamp: native Wayland was replaced by X11 in 0.6; running through XWayland");
    }

    let prefs = Preferences::load();
    let window_state = WindowState::load();
    let instance = match platform::InstanceGuard::claim() {
        Ok(InstanceKind::Primary(guard)) => Some(guard),
        Ok(InstanceKind::Secondary) => return,
        Err(e) => {
            eprintln!("niruamp: cannot establish single-instance socket: {e}");
            None
        }
    };
    let backend = platform::ensure_backend(&prefs, true);

    let (w, h) = if prefs.window_shade {
        (window_state.normal_width.max(420.0), 58.0)
    } else {
        (
            window_state.width.max(420.0),
            window_state.height.max(120.0),
        )
    };
    if let Some(hypr) = hyprland::Hyprland::detect() {
        if hypr.speaks_lua() {
            let position = window_state.x.zip(window_state.y);
            if let Err(e) = hypr.register_placement_rule((w, h), position, prefs.always_on_top) {
                eprintln!("niruamp: Hyprland placement rule not registered: {e}");
            }
        }
    }
    let geometry = x::Geometry {
        w: w.round() as u16,
        h: h.round() as u16,
        pos: window_state
            .x
            .zip(window_state.y)
            .map(|(x, y)| (x.round() as i16, y.round() as i16)),
        min: (400, 54),
    };
    let xwin = match x::X::open(&geometry, app::APP_TITLE, &class) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("niruamp: display/window initialization failed: {e}");
            eprintln!("NIRUAMP needs an X11 display (XWayland under Wayland). NPLAY remains usable without it: run `nplay`.");
            std::process::exit(1);
        }
    };
    xwin.set_initial_above(prefs.always_on_top);
    let (iw, ih, icon) = views::common::window_icon();
    let _ = xwin.set_icon(iw, ih, &icon);
    let waker = match shell::Waker::new() {
        Ok(w) => w,
        Err(e) => {
            eprintln!("niruamp: eventfd: {e}");
            std::process::exit(1);
        }
    };
    let wake = waker.wake();
    let ctx = gui::Context::new();
    let timing = std::env::var_os("NIRUAMP_TIMING").map(|_| started);
    let shell = shell::Shell::new(xwin, ctx.clone(), waker, geometry.w, geometry.h, timing);
    let mut app = NiruampApp::new(
        ctx,
        wake,
        prefs,
        backend,
        instance,
        window_state,
        shell.visibility_state(),
    );
    shell.run(&mut app);
}
