# NIRUAMP — implementation mandate

## Mission

Build NIRUAMP end to end: a polished, fast, resource-conscious **Rust graphical frontend for NPLAY**, visually unmistakable as a classic Winamp-inspired player, with beautiful native NIRUX theme integration.

This repository is an implementation handoff, not an existing application. Deliver a working, tested, installable product in one sustained engineering pass. Do not stop after scaffolding, a mockup, a design document, a transport demo or a local-file-only prototype. Do not ask the owner to approve routine technical decisions or each milestone. Make reasonable decisions, document significant tradeoffs and continue through integration and validation.

The owner will supply authorized bare-metal machines, SSH connection details and any machine-specific constraints when assigning the task. Those details are not embedded here. Never invent access or credentials. Start local work while awaiting unavailable external resources; ask only when genuinely blocked or when an action needs permission.

## Non-negotiable product boundaries

1. NIRUAMP is Rust UI code, not a Python GUI, Electron app or browser wrapper.
2. **NPLAY is a required runtime dependency.** NIRUAMP must not implement an independent mpv player, Spotify receiver, music scanner, provider collection, queue authority or statistics pipeline.
3. **NPLAY must remain fully functional standalone**, including its normal terminal launch, CLI diagnostics, themes, providers, MPRIS and existing data. Starting NPLAY alone must not require NIRUAMP, a GUI library or a running display server.
4. Introduce a small, versioned, headless local service boundary in NPLAY. Both UIs should reuse its established domain behavior, not fork it. Keep risky rewrites out of verified transport/provider code.
5. No duplicate listening events, competing queue models or double audio when clients connect. One backend session has one authoritative state and one MPRIS owner.
6. NIRUX integration is first-class, but NIRUAMP must also work on a non-NIRUX Linux desktop with a handsome fallback theme.
7. Preserve all existing user data, configuration and secrets. Never treat existing files or dirty worktrees as disposable.

## Repositories and scope of authority

- Frontend: `https://github.com/nirucon/niruamp` (this repository).
- Required backend: `https://github.com/nirucon/nplay`.
- Desktop integration/appearance references: `https://github.com/nirucon/nirux`.
- On the owner's initial workstation, check `/home/niru/Git/nplay` and `/home/niru/Git/nirux`; paths on test machines may differ.

Implement necessary backend changes in the **NPLAY repository**, with regression tests and documentation. This is explicitly part of this implementation assignment. Keep frontend and backend commits separate and provide exact compatible revisions/minimum versions. Inspect each repository's own agent instructions and working-tree state first. Read current source rather than relying on this brief's snapshot.

NIRUX is initially a reference, not permission to modify or release the whole desktop suite. Do not delete NIRUPLAY; its removal is a separate owner decision. Do not publish releases, push unrelated changes, replace a user's desktop configuration, install system-wide packages or modify remote system services without authorization. Normal focused implementation commits are expected; push implementation branches only when authorized in the assignment or by repository workflow.

## Architectural starting points — verify against current NPLAY

The inspected NPLAY 1.6.0 checkout uses Python, mpv JSON IPC, SQLite, optional Spotify/librespot, and provider adapters. Useful seams include:

- `nplay/core/playback.py`, `state.py`, `events.py`, `sources.py`, `session.py`.
- `nplay/services/`, including search, settings and serialized playback startup.
- `app.py` as a compatibility facade; `navigation.py` still contains domain/UI coupling.
- `player.py`, `spotify.py`, `providers.py`, `db.py`, `statistics/`, `mpris.py`.
- `ui.py` / `ui_render.py` for curses; these must not be required to run a headless service.

These seams are **not proof of a complete headless API**. Audit constructors, UI callbacks, event emission, background threads, startup/shutdown and imports. Existing event callbacks can originate on worker threads; do not perform blocking socket writes inside them. Preserve generation guards, startup serialization, Spotify activation/recovery, session persistence, cancellation and error reporting.

### Backend control interface

Prefer an explicit CLI mode such as `nplay --serve`, over a private bridge vendored into NIRUAMP. Use a versioned Unix-domain socket protocol (JSON-RPC 2.0 plus defined event notifications is a good default). Publish the actual contract in `docs/PROTOCOL.md` and test both ends.

- Handshake: protocol version, backend version, capabilities and feature availability.
- Correlated requests, structured errors, request limits, timeouts and cancellation semantics where needed.
- Snapshot plus ordered state events; reconnect/resynchronization must not lose queue or playback state. Include a revision/sequence mechanism.
- Read/browse/search commands return stable IDs and bounded/paginated results. Never pass arbitrary Python names or executable commands through RPC.
- Playback/seek/volume/mute, queue editing, source browsing, favorites, playlists, repeat/shuffle, settings and artwork access must use backend operations.
- Slow provider calls and filesystem work must never block the Rust UI thread or service event dispatch.
- Use the user's private runtime directory, restrictive permissions, ownership validation and a per-user instance lock. Handle stale sockets without removing another live process's endpoint. Do not expose TCP/network control by default.
- Bound frames, buffers, client queues and concurrent requests. Slow/disconnected clients must not stall playback. Redact secrets and authenticated stream URLs from logs and protocol responses.
- Give artwork a safe, documented access model. Do not expose arbitrary filesystem reads.
- Capability changes, provider errors and unavailable optional components must produce useful UI states, not crashes or endless spinners.

### Lifecycle and standalone semantics

Write down and test a lifecycle matrix: backend absent/present, frontend start/exit/crash, backend crash/restart, terminal alone, terminal plus GUI, and multiple GUI launches.

- NIRUAMP discovers and connects to a compatible backend, or starts the installed `nplay` service automatically. Never download dependencies on launch.
- A second GUI launch should activate the existing instance by default, not start duplicate audio.
- Define backend ownership and shutdown explicitly. An attached GUI must never kill a backend it did not start. Do not silently leave a newly spawned service/audio running forever after normal GUI exit; make keep-playing/tray behavior explicit and documented.
- Prefer terminal and GUI clients sharing the same backend when simultaneously used. If a safe shared-terminal refactor is disproportionate, implement a clear, non-destructive conflict/handoff policy and document the limitation. Never silently run conflicting authoritative sessions against shared data.
- The backend alone owns MPRIS, listening-time accounting, notifications and playback state. Do not add a second frontend MPRIS identity for the same session. Distinct real players can have distinct valid MPRIS names; the problem is duplicate ownership of one session, not a global one-player restriction.
- Missing/incompatible NPLAY must yield a helpful graphical dependency error with an installation/version remedy, not a panic.

## Visual direction: immediately recognizable Winamp inspiration

Use **classic Winamp 2.x** as the visual/interactivity reference, not a generic sidebar-and-cards streaming app. At first glance, without reading the title, the owner should recognize the inspiration.

Required visual grammar:

- A compact, carefully composed player deck: narrow title strip, digital elapsed/remaining time, track title/metadata display, visualization area and recognizable grouped transport buttons.
- Distinct seek, volume, repeat/shuffle controls and clear playlist/equalizer toggles.
- A dense, beautifully styled playlist with track numbering, duration, current-track indication, selection, scrolling and queue manipulation.
- Subtle beveled/inset surfaces, precise separators and a deliberate hardware-panel feel. Original artwork/icons; no copied Winamp logos, bundled proprietary assets or misleading affiliation.
- A usable compact/windowshade mode, scaled normal mode, and optional always-on-top behavior integrated with the WM.
- An equalizer panel if supported by the backend, with real functional controls. Do not present a decorative fake EQ. Report transport limitations honestly: Spotify playback is not routed through mpv DSP.

Use classic proportions as a reference, not a mandatory physical `275×116` pixel prison. The result must remain legible and clickable on modern screens. Scaling must preserve crisp borders/icons and measured text; avoid tiny fixed hitboxes, blurry bitmap enlargement and clipped translated/Unicode titles. A coherent connected player/playlist/EQ layout matters more than forcing separate native windows. Choose attached panels or multiple windows based on reliability, NIRUWM compatibility and measured UX; document the choice.

Do not implement legacy `.wsz` skin compatibility in the first pass. It is not needed for recognizable Winamp character and must not displace theme integration or core functionality.

## NIRUX appearance integration

Inspect the current NIRUX appearance implementation and builtin theme definitions, not just example files. Important references:

- `nirux/appearance.example.toml`, `nirux/docs/STYLE.md`, relevant config/theme selection code.
- `$XDG_CONFIG_HOME/niruwm/config.toml` (selected theme).
- `$XDG_CONFIG_HOME/niruwm/appearance.toml` (appearance contract).
- `$XDG_CONFIG_HOME/niruwm/themes/NAME.toml` (custom theme overrides).
- Respect XDG defaults when variables are unset.

The checked appearance contract is schema 1. Semantic roles include `background`, `surface`, `foreground`, `muted`, `border`, `focus`, `accent`, `selected`, `success`, `warning`, `error`, `disabled`, plus font, spacing, focus width and reduced motion. Custom theme files can be sparse and flat, with colors such as `background = "0c121b"` and `focus = "6f9bc7"`. Many active themes are built in and have **no custom TOML file**. Implement actual resolution/precedence rather than assuming that file exists. Do not scrape generated Kitty/Fish files as the primary appearance API.

- Follow the active desktop theme by default; watch changes with debounce, validate then atomically apply. Recover from partial writes, unknown schemas and missing files without replacing a working appearance with garbage.
- Match actual NIRUX precedence, including custom theme overrides. Derive missing roles consistently, preserving contrast and classic panel depth.
- Maintain a deliberate role mapping for LED digits, visualization, bevels, focus, hover, selection and disabled states. Do not just tint every widget the same accent color.
- Support manual appearance selection and a standalone NIRUAMP fallback without requiring NIRUX to be installed.
- Honor configured fonts/scale and reduced motion. State must remain clear without relying only on color.
- Validate dark and light themes, muted palettes and bright Hackerman-like palettes. Test actual builtin and custom themes, not only invented fixtures.
- Do not overwrite the owner's NIRUX settings to make screenshots look right.

## UX and functional completeness

Deliver all major NPLAY sources through the GUI when enabled/configured: local library, Navidrome/Subsonic, Sveriges Radio live/podcasts, custom radio, YouTube and Spotify. Use backend capability discovery and reuse its authentication/configuration flows safely. No promise of Spotify EQ or seek on live radio when unsupported.

Keep the deck compact; use a well-designed browser/search/settings panel where necessary. Required everyday flows:

- Start the app without a terminal, browse/search, play an album/playlist, inspect/edit/reorder the queue, seek, change volume, mute, favorite and manage playlists.
- Good metadata/Unicode handling, artwork placeholders, elapsed/remaining time, live-source labels and clear loading/error/empty states.
- Drag-and-drop supported media files/URLs through backend commands; context menus and useful tooltips.
- Keyboard-first operation: transport shortcuts, search, queue navigation, visible focus, Escape/back and sensible tab order. Shortcuts must not steal typing from text fields.
- Asynchronous search with stale-result protection; no focus stealing or selection jumps on routine state events.
- Persist window geometry/mode and frontend preferences in NIRUAMP's own XDG configuration/state. Playback session and music data stay owned by NPLAY.
- Connect/disconnect/retry states and actionable errors for missing optional tools, network failures, invalid credentials and missing files.
- Actual visualization only, not randomly animated bars. Prefer the existing backend/CAVA path where viable; keep visualization optional and shut it down or suspend delivery when hidden. Document source/capture limitations.

## Rust implementation and efficiency

Choose a mature Rust GUI stack after a short practical spike; `egui`/`eframe` is a reasonable candidate, not a mandate. Verify X11/NIRUWM behavior and the owner's supplied hardware before committing to rendering/window choices. Avoid writing a custom toolkit unless clearly justified.

- Separate app/state, views, protocol client, theme resolution, assets and platform integration. Keep typed protocol/domain models and frontend/backend responsibilities clear.
- Event-driven repainting: no unconditional repaint loop while idle. Active playback needs only bounded clock updates; animated visualization can use a capped refresh rate while visible. Respect reduced motion and obscured/minimized states.
- Backend provider polling must remain bounded; do not poll the entire library, queue or artwork every frame. Use notifications and cached snapshots.
- Bounded image/text caches, background decode/downscale, virtualized large lists and paginated browsing. Avoid thread/process-per-widget and unbounded tasks/channels.
- No extra audio capture or provider process merely because the GUI is open. Playback helpers remain under backend ownership.
- Use a lockfile, explicit minimum Rust version, sensible release profile and portable asset handling. No hard-coded home paths or build-machine paths.
- Do not optimize by guessing. Measure release builds on the supplied machines; include rendering/backend child processes in reported totals. Backend Python is expected and is not a hidden claim of an all-Rust engine.

## Autonomous execution plan

1. Inspect repository instructions, branches/dirty state, current NPLAY architecture/tests, NIRUX appearance and supplied machines. Record assumptions and a short implementation plan; start executing immediately.
2. Establish backend headless startup, local protocol, ownership/lifecycle and regression tests. Preserve normal terminal operation before layering on GUI work.
3. Build the real Rust frontend around that protocol, with a recognizable final-quality player deck early. Do not defer visual polish until an unfinished final step.
4. Complete source browsing/search, queue/library flows, settings/auth handoffs, themes, scaling, visualization and desktop integration.
5. Exercise actual release builds on authorized bare-metal systems, fix defects, capture screenshots and measure resources. Run available tests before and after backend changes.
6. Finish install/uninstall/update guidance, compatibility documentation and a concise evidence-based handoff. Keep working through fix/test cycles instead of ending at the first successful compile.

Use specialist/parallel agents if available and useful for independent tasks such as protocol tests, theme research or UI review; coordinate shared contracts and integrate their results. Delegation is optional, not a reason to stop. Send concise milestone/blocker updates, not repeated permission questions. Never claim external testing you could not perform.

## Testing and acceptance gate

Automate protocol, Rust model/theme parsing and NPLAY service/lifecycle tests. Add CI for Rust format/lint/tests and appropriate Linux builds; backend tests belong in NPLAY too. Include malformed input, oversized frames, incompatible versions, dead/slow clients, stale sockets, reconnect, concurrent commands and shutdown during pending playback/search.

Before declaring completion, demonstrate:

- [ ] A release-built Rust GUI launches from a desktop entry with installed NPLAY as its explicit dependency.
- [ ] Normal standalone `nplay`, its diagnostics and existing regression suite still work without GUI dependencies.
- [ ] One authoritative queue/playback/statistics/MPRIS path; repeated launches and client crashes do not produce duplicate audio or leaked helpers.
- [ ] Winamp inspiration is obvious in screenshots, including the main deck and playlist; it is not merely a generic app with a retro title.
- [ ] Theme changes apply live, including a builtin theme with no custom file, sparse custom theme, malformed-file recovery, light/dark contrast and standalone fallback.
- [ ] Everyday mouse/keyboard workflows, queue edits, reconnect and session recovery work without blocking the UI.
- [ ] Every major configured source is exercised. Use mocks/contract tests for unavailable accounts and explicitly identify any live test not performed. Never fabricate credentials or successful Spotify/provider tests.
- [ ] Correct seek/EQ/visualization behavior and disabled states for sources that lack those capabilities.
- [ ] 800×600, 1366×768, 1920×1080 and available HiDPI/mixed-monitor setups have no inaccessible controls or clipped key content; actual test coverage is stated.
- [ ] Resource measurements cover stopped/idle, paused, playing, visualization on/off, minimized/hidden and a large-library case. Observe at least a short sustained run in each relevant mode, not a single misleading CPU sample.
- [ ] Resource report includes hardware, display/session, build mode, measurement tools/window, frontend/backend/helpers, CPU, RSS and known renderer costs. Idle should have no continuous redraw/visualization work and target approximately ≤1% of one logical CPU for the frontend over a sustained idle observation; explain/fix outliers rather than inventing results. Choose and document a measured memory budget instead of claiming an untested universal limit.
- [ ] Install from a clean checkout works with documented prerequisites and a stated NPLAY minimum version; uninstall preserves user data by default.

Add `docs/ARCHITECTURE.md`, `docs/PROTOCOL.md`, `docs/TESTING.md` and an evidence-based validation/resource report, with screenshots where possible. Update the README from planning status to accurate installation/use instructions only when runnable. Document remaining limitations honestly; do not quietly redefine acceptance to fit partial delivery.

## Final handoff format

Report what works, frontend/backend commit IDs and versions, install/run commands, tests and machines actually used, screenshots/resource evidence, and any concrete unresolved blocker. Do not call this complete if it is only a scaffold or if standalone NPLAY has regressed. If access/network/time genuinely prevents a test, leave reproducible commands and distinguish implemented, automatically tested and live-verified behavior.
