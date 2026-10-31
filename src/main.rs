#![allow(
    dead_code,
    deprecated,
    clippy::too_many_arguments,
    clippy::collapsible_if,
    clippy::type_complexity,
    clippy::collapsible_match
)]
mod app;
mod config;
mod model;
mod platform;
mod protocol;
mod theme;
mod views;

use crate::app::NiruampApp;
use crate::config::{Preferences, WindowState};
use crate::platform::InstanceKind;
use egui::Vec2;

const HELP: &str = "NIRUAMP — Winamp-inspired Rust frontend for NPLAY

Usage: niruamp [--version] [--help]

Environment:
  NPLAY_SERVICE_SOCKET  Path to an existing NPLAY control socket
  NPLAY_DEV_PATH        NPLAY checkout for development service startup
  NPLAY_BIN             Alternative NPLAY executable
  NIRUAMP_SCREENSHOT    Save a release-build screenshot to this path

NPLAY 1.7.0 or newer is required. It remains fully usable standalone.";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("NIRUAMP {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{HELP}");
        return;
    }
    if let Some(unknown) = args.iter().find(|a| a.starts_with('-')) {
        eprintln!("niruamp: unknown option {unknown}\n\n{HELP}");
        std::process::exit(2);
    }

    let prefs = Preferences::load();
    let window_state = WindowState::load();
    let instance = match platform::InstanceGuard::claim() {
        Ok(InstanceKind::Primary(guard)) => Some(guard),
        Ok(InstanceKind::Secondary) => {
            // The running instance was activated over its private socket.
            return;
        }
        Err(e) => {
            eprintln!("niruamp: cannot establish single-instance socket: {e}");
            None
        }
    };
    let backend = platform::ensure_backend(&prefs, true);

    let inner = if prefs.window_shade {
        Vec2::new(window_state.normal_width.max(420.0), 58.0)
    } else {
        Vec2::new(
            window_state.width.max(420.0),
            window_state.height.max(120.0),
        )
    };
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("NIRUAMP")
        .with_app_id("niruamp")
        .with_decorations(false)
        .with_resizable(true)
        .with_inner_size(inner)
        .with_min_inner_size(Vec2::new(400.0, 54.0));
    if let (Some(x), Some(y)) = (window_state.x, window_state.y) {
        viewport = viewport.with_position(egui::pos2(x, y));
    }
    let options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Glow,
        vsync: true,
        multisampling: 0,
        depth_buffer: 0,
        stencil_buffer: 0,
        ..Default::default()
    };
    let result = eframe::run_native(
        "NIRUAMP",
        options,
        Box::new(move |cc| {
            // The error state is drawn inside the product, not printed to a
            // terminal the user may never have opened.
            Ok(Box::new(NiruampApp::new(
                cc,
                prefs,
                backend,
                instance,
                window_state,
            )))
        }),
    );
    if let Err(e) = result {
        eprintln!("niruamp: display/window initialization failed: {e}");
        eprintln!("NIRUAMP needs a graphical X11 or XWayland session. NPLAY remains usable without it: run `nplay`.");
        std::process::exit(1);
    }
}
