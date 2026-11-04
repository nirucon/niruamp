# Agent assignment: NIRUAMP 0.4.0, a one-shot efficiency and build update

## 0. Quick start

1. Read this file, `README.md`, `CHANGELOG.md`, `docs/`, `src/app.rs`, `src/platform.rs`,
   `src/protocol.rs` and `src/views/`. The audit baseline is `cf7dd33` (0.3.3). Revalidate
   each item against current HEAD and skip items that are already fixed, noting that you
   did.
2. **Dependency order:** item A3 uses NPLAY 1.8.0's `visualization.subscribe` (nirucon/nplay
   `AGENTS.md` P2/P3). Do A1, A2, A4–A7 first. Do A3 once NPLAY 1.8.0 is released, and keep
   the 0.3.x polling path as a fallback when the capability is missing.
3. Commit per item. Keep fmt, clippy `-D warnings` and tests green.
4. Pass the gates in §4, release in §5 and report in §6.

This is an **execution** assignment. Do not stop after a plan or a branch. Make sensible
decisions, document them, and continue without routine questions.

## 1. Invariants

- NIRUAMP is a frontend. NPLAY owns playback, the queue, providers, statistics and MPRIS.
  Add no independent player or provider logic.
- The look, themes, views and keyboard behavior stay. Visual changes are limited to what
  §3 lists.
- It works with NPLAY 1.7.1+ (the old protocol) and uses the newer features when they are
  advertised.
- Keep `rust-version = 1.80` (or raise it to at most 1.85 if eframe requires it; document
  that). Locked builds must work on Debian 13's rustc 1.85.

## 2. Findings (verified 2026-11-04 on niruxwork, running NIRUAMP 0.3.3 + NPLAY 1.7.3, music playing)

### A1: The locked build fails on Debian's Rust (high)
`Cargo.lock` needs rustc 1.86–1.88 (`calloop` 0.14.5, `icu_*` 2.4.0), while
`rust-version = "1.80"` and `rust-toolchain.toml` says `stable`. A source build on
Debian 13's 1.85 fails.
**Fix:** `CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback cargo update`. This was
verified to build and pass all 18 tests on 1.85. Also:
- add `.cargo/config.toml` with `resolver.incompatible-rust-versions = "fallback"`,
- pin `rust-toolchain.toml` to the declared version, or remove it,
- and add an MSRV CI job.

### A2: The UI loop runs far faster than it needs to (high, battery)
Live: NIRUAMP used about **6.5 % CPU** with **~1 230 wakeups/s** and 21 threads, and
**11:40 min of CPU over 2 h 49 min** (195 MB RSS). The main thread alone woke ~1 024
times/s. A 3 s syscall profile of the main thread showed ~500 event-loop iterations,
~4 600 `poll` + ~5 100 `recvmsg` on the X socket, and ~2 400 `writev`. That is about
**170 frames/s**, while the visualization is meant to run at 30 fps.
Causes in the code:
- `request_repaint_after(25 ms)` after every RPC response (`app.rs` ~529), which includes
  the 30 Hz visualization round trips,
- `request_repaint()` from the RPC reader for every incoming message
  (`set_wake`, `app.rs` ~393/560),
- several independent repaint requests (33 ms visualization, 1 s clock, 250 ms toasts).

**Fix:**
- One frame scheduler: compute the next needed frame time from the visualization (≤ 30 fps
  while visible and not silent), the clock (whole seconds while playing), toasts, search
  debounce and animations. Call `request_repaint_after` once per frame with that time.
- RPC responses mark state dirty and request **one** repaint, coalesced, instead of a
  25 ms poll.
- Verify with `NIRUAMP_DEBUG_FPS=1`.

**Done when:** with music playing and the spectrum visible, the frame rate is ≤ 31 fps,
the process uses ≤ 2.5 % CPU (software GL), and the main thread wakes ≤ 120/s. Paused
and without the spectrum, it uses ≤ 0.5 % CPU and ≤ 5 frames/s.

### A3: The spectrum is polled over JSON (efficiency, needs NPLAY 1.8.0)
`maybe_visualization` sends `visualization.data` every 33 ms. Together with the NPLAY
service (~1.8 % CPU, ~170 wakeups/s) and `cava` (~1.4 %), the visualizer costs about
30× more than mpv's actual playback (0.3 %).
**Fix:**
- When the server advertises `visualization_stream`, call `visualization.subscribe` once,
  render the pushed frames, and `unsubscribe` when the spectrum is not visible.
- Draw a flat spectrum and stop animating during silence.
- Without the capability, keep the current poll, still capped by the A2 scheduler.

### A4: The spectrum keeps running when nobody can see it
`should` (`app.rs` ~1105) stops the visualization only when the window is minimized or
hidden to the tray. When NIRUWM shows another workspace, the window is unmapped but not
minimized, so polling and CAVA continue. Unmap/occlusion handling is in the code but was
not tested on another workspace.
**Fix:** treat unmapped/occluded as not visible: winit `Occluded`/visibility, or egui's
viewport info if it exposes it, with an X11 `UnmapNotify`/`_NET_WM_STATE_HIDDEN`
fallback.
**Done when:** switching to another workspace stops the spectrum requests within 1 s, and
NPLAY stops CAVA after its idle window. That is verified natively on niruxwork.

### A5: The instance listener busy-polls
The `niruamp-instance` thread uses a non-blocking `accept` plus a 120 ms sleep (~8
wakeups/s forever). **Fix:** a blocking accept on a dedicated thread, unblocked on
shutdown by connecting to itself or closing the listener.

### A6: Several threads use fixed sleeps
`theme.rs` (80–1 300 ms sleeps) and `platform.rs` (100/250 ms) watch for changes by
sleeping. **Fix:** use inotify on the theme files where they watch files, and blocking
waits elsewhere, or document why a timer is required.

### A7: Memory
195 MB RSS for a player UI. Measure what dominates: GL buffers, decoded artwork, fonts or
the image cache. Bound the artwork cache (for example LRU, ≤ 64 MB of decoded pixels) and
decode artwork at display size. Report before and after. Do not trade away the look.

## 3. UX

### U1: Show what the visualizer costs
In Settings → Visualization, add a "Pause spectrum when the window is not visible"
option (default **on**, the new A4 behavior) and an fps choice (30/20/15, default 30).
`reduce_motion` keeps its current meaning.

### U2: A quiet tray mode
When the window is hidden to the tray, NIRUAMP must not repaint at all (0 frames/s) and
should wake at most once per second for tray state. Verify this.

Out of scope: new views, a skin format, and changes to the playback model.

## 4. Gates

Automated:
- fmt, clippy `-D warnings`, `cargo test --locked`, a release build, `scripts/` checks if
  present, and the MSRV job.
- New tests: the frame scheduler (pure: given the state, compute the next deadline), the
  subscribe/poll fallback selection against a fake NPLAY socket, visibility state
  handling, and artwork cache bounds.

Native, on **niruxwork** (the owner allows local testing; NIRUAMP and NPLAY are installed
and running):
- Before and after, record for NIRUAMP, the NPLAY service, cava and mpv: CPU, wakeups,
  frames/s (`NIRUAMP_DEBUG_FPS=1`) and RSS, in these states:
  - playing with the spectrum visible,
  - playing on another workspace,
  - hidden to the tray,
  - paused.
- Do not change the owner's playlists, library or settings beyond toggling and restoring
  the options under test.

## 5. Release

Bump to **0.4.0** and update `CHANGELOG.md` and the README (the minimum NPLAY version for
the stream, the fallback). Tag the release through the normal workflow and verify the
artifacts. NIRUAMP is a NIRUX extra. Update its pin in `nirucon/nirux` through the extras
mechanism used before (check the nirux git log for "extras: pin"), after NPLAY 1.8.0. Install
on niruxwork and verify the running version.

## 6. Final report

For each item A1–A7 and U1–U2: the change, the tests and the measured table per state,
before and after. Include release URLs, the installed versions of NIRUAMP and NPLAY, and
remaining limits. Mark anything unfinished as **PARTIAL** and give the exact remaining
steps.
