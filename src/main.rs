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
mod hyprland;
mod model;
mod omarchy;
mod platform;
mod protocol;
mod schedule;
mod session;
mod sound;
mod theme;
mod tray;
mod views;
mod visibility;

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
  NIRUAMP_DEBUG_FPS     Print frames/s and NPLAY messages/s every 5 s
  NIRUAMP_BACKEND       x11 or wayland: override the windowing backend
                        (default: X11 in the NIRU session, native Wayland
                        wherever WAYLAND_DISPLAY is set otherwise)

NPLAY 1.7.3 or newer is required (1.8.0+ for the visualization stream).
It remains fully usable standalone.";

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
    let backend_kind = session::backend();
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("NIRUAMP")
        .with_decorations(false)
        .with_resizable(true)
        .with_inner_size(inner)
        .with_min_inner_size(Vec2::new(400.0, 54.0));
    if let (Some(x), Some(y)) = (window_state.x, window_state.y) {
        viewport = viewport.with_position(egui::pos2(x, y));
    }
    // egui-winit applies the app id through winit's shared `with_name(app_id, "")`,
    // which on X11 would leave WM_CLASS with an empty instance. Only Wayland needs
    // the app id; on X11 winit derives WM_CLASS ("niruamp", "niruamp") from the
    // executable, exactly as in 0.4.
    if backend_kind == session::Backend::Wayland {
        viewport = viewport.with_app_id("niruamp");
        // Wayland lets only the compositor place windows. On Hyprland a rule
        // registered before the window exists opens it floating at the saved
        // geometry; elsewhere the compositor decides.
        if let Some(hypr) = hyprland::Hyprland::detect() {
            if hypr.speaks_lua() {
                let position = window_state.x.zip(window_state.y);
                if let Err(e) =
                    hypr.register_placement_rule((inner.x, inner.y), position, prefs.always_on_top)
                {
                    eprintln!("niruamp: Hyprland placement rule not registered: {e}");
                }
            }
        }
    }
    let options = eframe::NativeOptions {
        event_loop_builder: Some(Box::new(move |builder| {
            use winit::platform::wayland::EventLoopBuilderExtWayland;
            use winit::platform::x11::EventLoopBuilderExtX11;
            match backend_kind {
                session::Backend::X11 => {
                    builder.with_x11();
                }
                session::Backend::Wayland => {
                    builder.with_wayland();
                }
            }
        })),
        viewport,
        renderer: eframe::Renderer::Glow,
        // A compositor stops frame callbacks for a window it does not show
        // (another workspace). With vsync, Mesa's eglSwapBuffers then waits
        // for the next callback inside the UI thread, which stops answering
        // xdg_wm_base pings (Hyprland: "Application not responding"), and the
        // same holds for XWayland. NIRUAMP paces its own frames, so vsync is
        // only kept on a real X server (NIRUWM), where it never blocks.
        vsync: session::vsync(backend_kind),
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
        eprintln!("NIRUAMP needs a graphical X11 or Wayland session (NIRUAMP_BACKEND=x11 forces XWayland). NPLAY remains usable without it: run `nplay`.");
        std::process::exit(1);
    }
}
