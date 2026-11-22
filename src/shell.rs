//! The event loop: X events, wake-ups from worker threads (eventfd) and the
//! frame scheduler's deadline, all in one `poll`. Nothing runs between
//! events: a paused player sleeps in the kernel.
use crate::app::NiruampApp;
use crate::gui::{
    self, pos2, vec2, Context, CursorIcon, DroppedFile, Event, InputState, Key, Modifiers,
    PointerState, Rect, ViewportCommand, ViewportInfo,
};
use crate::x::X;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _, KeyButMask, Visibility as XVis};
use x11rb::protocol::Event as XEvent;

/// Wakes the event loop from any thread.
pub type Wake = Arc<dyn Fn() + Send + Sync>;

pub struct Waker {
    fd: Arc<OwnedFd>,
}

impl Waker {
    pub fn new() -> std::io::Result<Self> {
        // SAFETY: eventfd returns a new descriptor or -1.
        let fd = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK) };
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: fd is a fresh, owned descriptor.
        Ok(Waker {
            fd: Arc::new(unsafe { OwnedFd::from_raw_fd(fd) }),
        })
    }

    pub fn wake(&self) -> Wake {
        let fd = self.fd.clone();
        Arc::new(move || {
            let one: u64 = 1;
            // SAFETY: writing 8 bytes to an eventfd.
            unsafe { libc::write(fd.as_raw_fd(), &one as *const u64 as *const libc::c_void, 8) };
        })
    }

    fn drain(&self) -> bool {
        let mut v: u64 = 0;
        // SAFETY: reading 8 bytes from a non-blocking eventfd.
        let n = unsafe {
            libc::read(
                self.fd.as_raw_fd(),
                &mut v as *mut u64 as *mut libc::c_void,
                8,
            )
        };
        n == 8
    }
}

struct DragIn {
    source: u32,
    uris: bool,
}

pub struct Shell {
    x: X,
    ctx: Context,
    waker: Waker,
    pm: tiny_skia::Pixmap,
    raster: gui::raster::Raster,
    size: (u16, u16),
    pos: (i32, i32),
    pointer: PointerState,
    events: Vec<Event>,
    scroll: f32,
    mods: Modifiers,
    dropped: Vec<DroppedFile>,
    vis: Arc<AtomicU8>,
    minimized: bool,
    fullscreen: bool,
    manual_drag: Option<(i32, i32, i32, i32)>,
    press_root: (i32, i32, i32, i32),
    drag_in: Option<DragIn>,
    clipboard: String,
    cursor: CursorIcon,
    mapped_once: bool,
    timing: Option<Instant>,
    screenshot: Option<std::path::PathBuf>,
    prev_shapes: Vec<gui::ClippedShape>,
    painted: bool,
}

impl Shell {
    pub fn new(x: X, ctx: Context, waker: Waker, w: u16, h: u16, timing: Option<Instant>) -> Self {
        let pm = tiny_skia::Pixmap::new(w.max(1) as u32, h.max(1) as u32).expect("pixmap");
        Shell {
            x,
            ctx,
            waker,
            pm,
            raster: gui::raster::Raster::new(),
            size: (w, h),
            pos: (0, 0),
            pointer: PointerState::default(),
            events: Vec::new(),
            scroll: 0.0,
            mods: Modifiers::default(),
            dropped: Vec::new(),
            vis: Arc::new(AtomicU8::new(crate::visibility::UNMAPPED)),
            minimized: false,
            fullscreen: false,
            manual_drag: None,
            press_root: (0, 0, 0, 0),
            drag_in: None,
            clipboard: String::new(),
            cursor: CursorIcon::Default,
            mapped_once: false,
            timing,
            screenshot: None,
            prev_shapes: Vec::new(),
            painted: false,
        }
    }

    pub fn visibility_state(&self) -> Arc<AtomicU8> {
        self.vis.clone()
    }

    pub fn x(&self) -> &X {
        &self.x
    }

    pub fn run(mut self, app: &mut NiruampApp) {
        let mut deadline: Option<Instant> = Some(Instant::now());
        let xfd = self.x.c.stream().as_raw_fd();
        let efd = self.waker.fd.as_raw_fd();
        let mut quit = false;
        while !quit {
            let mut fds = [
                libc::pollfd {
                    fd: xfd,
                    events: libc::POLLIN,
                    revents: 0,
                },
                libc::pollfd {
                    fd: efd,
                    events: libc::POLLIN,
                    revents: 0,
                },
            ];
            let timeout = match deadline {
                None => -1,
                Some(d) => {
                    let now = Instant::now();
                    if d <= now {
                        0
                    } else {
                        ((d - now).as_micros().div_ceil(1000)).min(i32::MAX as u128) as i32
                    }
                }
            };
            // Events may already sit in x11rb's buffer.
            let pending = self.x.c.poll_for_event().ok().flatten();
            let n = if pending.is_some() {
                0
            } else {
                // SAFETY: two valid pollfds.
                unsafe { libc::poll(fds.as_mut_ptr(), 2, timeout) }
            };
            let mut frame = false;
            let mut present_only = false;
            if n > 0 && fds[1].revents & libc::POLLIN != 0 && self.waker.drain() {
                frame = true;
            }
            let mut next = pending;
            loop {
                let ev = match next.take() {
                    Some(e) => e,
                    None => match self.x.c.poll_for_event() {
                        Ok(Some(e)) => e,
                        Ok(None) => break,
                        Err(_) => {
                            quit = true;
                            break;
                        }
                    },
                };
                match self.handle(ev) {
                    Handled::Frame => frame = true,
                    Handled::Present => present_only = true,
                    Handled::Quit => quit = true,
                    Handled::Nothing => {}
                }
            }
            if quit {
                break;
            }
            if deadline.is_some_and(|d| Instant::now() + Duration::from_micros(500) >= d) {
                frame = true;
            }
            if !frame {
                if present_only && self.mapped_once && self.painted {
                    let (w, h) = (self.pm.width(), self.pm.height());
                    let _ = self.x.present(&self.pm, (0, 0, w, h));
                }
                continue;
            }
            let input = self.take_input();
            self.ctx.begin_frame(input);
            app.update(&self.ctx);
            let shapes = self.ctx.end_frame();
            let (rep, at) = self.ctx.wants_repaint();
            deadline = if rep { Some(Instant::now()) } else { at };
            let cursor = self.ctx.cursor_icon();
            if cursor != self.cursor {
                self.cursor = cursor;
                self.x.set_cursor(cursor);
            }
            for cmd in self.ctx.take_commands() {
                if self.command(cmd, app) {
                    quit = true;
                }
            }
            let visible = crate::visibility::is_visible(self.vis.load(Ordering::Acquire));
            if visible || !self.mapped_once {
                self.paint(shapes);
            } else {
                // Repaint everything once it can be seen again.
                self.painted = false;
            }
            if !self.mapped_once {
                let _ = self.x.map();
                self.mapped_once = true;
            }
        }
        app.on_exit();
    }

    fn paint(&mut self, shapes: Vec<gui::ClippedShape>) {
        let (w, h) = (self.size.0.max(1) as u32, self.size.1.max(1) as u32);
        let mut full_damage = !self.painted;
        if self.pm.width() != w || self.pm.height() != h {
            if let Some(pm) = tiny_skia::Pixmap::new(w, h) {
                self.pm = pm;
                full_damage = true;
            }
        }
        let full = Rect::from_min_size(pos2(0.0, 0.0), vec2(w as f32, h as f32));
        let region = if full_damage {
            Some(full)
        } else {
            gui::damage(&self.prev_shapes, &shapes, full)
        };
        if let Some(region) = region {
            self.ctx
                .fonts_mut(|f| self.raster.paint_region(&mut self.pm, f, &shapes, region));
            if self.mapped_once {
                let r = (
                    region.min.x as u32,
                    region.min.y as u32,
                    region.width() as u32,
                    region.height() as u32,
                );
                let _ = self.x.present(&self.pm, r);
            }
            self.painted = true;
        }
        self.prev_shapes = shapes;
        if let Some(path) = self.screenshot.take() {
            save_png(&self.pm, &path);
        }
        if let Some(t0) = self.timing.take() {
            eprintln!(
                "niruamp: first frame after {:.1} ms",
                t0.elapsed().as_secs_f64() * 1000.0
            );
        }
    }

    fn take_input(&mut self) -> InputState {
        let screen =
            Rect::from_min_size(pos2(0.0, 0.0), vec2(self.size.0 as f32, self.size.1 as f32));
        let inner = Rect::from_min_size(
            pos2(self.pos.0 as f32, self.pos.1 as f32),
            vec2(self.size.0 as f32, self.size.1 as f32),
        );
        let pointer = self.pointer.clone();
        self.pointer.delta = gui::Vec2::ZERO;
        self.pointer.primary_pressed = false;
        self.pointer.primary_released = false;
        self.pointer.secondary_pressed = false;
        self.pointer.secondary_released = false;
        InputState {
            pointer,
            events: std::mem::take(&mut self.events),
            modifiers: self.mods,
            scroll_delta: vec2(0.0, std::mem::take(&mut self.scroll)),
            time: self.ctx.0.start.elapsed().as_secs_f64(),
            predicted_dt: 0.0,
            raw: gui::RawInput {
                dropped_files: std::mem::take(&mut self.dropped),
            },
            viewport: ViewportInfo {
                inner_rect: Some(inner),
                outer_rect: Some(inner),
                monitor_size: Some(vec2(self.x.screen_w as f32, self.x.screen_h as f32)),
                minimized: Some(self.minimized),
                fullscreen: Some(self.fullscreen),
            },
            screen_rect: screen,
        }
    }

    fn set_vis(&self, e: crate::visibility::VisEvent) -> bool {
        let old = self.vis.load(Ordering::Acquire);
        let new = crate::visibility::apply(old, e);
        self.vis.store(new, Ordering::Release);
        old != new
    }

    fn handle(&mut self, ev: XEvent) -> Handled {
        use crate::visibility::VisEvent;
        let win = self.x.win;
        match ev {
            XEvent::Expose(e) if e.count == 0 => Handled::Present,
            XEvent::ConfigureNotify(e) if e.window == win => {
                let mut changed = false;
                if (e.width, e.height) != self.size {
                    self.size = (e.width, e.height);
                    changed = true;
                }
                if let Some(p) = self.x.root_position() {
                    if p != self.pos {
                        self.pos = p;
                        changed = true;
                    }
                }
                if changed {
                    Handled::Frame
                } else {
                    Handled::Nothing
                }
            }
            XEvent::MapNotify(e) if e.window == win => {
                self.set_vis(VisEvent::Mapped);
                self.minimized = false;
                Handled::Frame
            }
            XEvent::UnmapNotify(e) if e.window == win => {
                self.set_vis(VisEvent::Unmapped);
                Handled::Frame
            }
            XEvent::VisibilityNotify(e) if e.window == win => {
                if self.set_vis(VisEvent::Obscured(e.state == XVis::FULLY_OBSCURED)) {
                    Handled::Frame
                } else {
                    Handled::Nothing
                }
            }
            XEvent::PropertyNotify(e) => {
                if e.window == win && e.atom == self.x.atoms.net_wm_state {
                    let hidden = self.x.has_state(self.x.atoms.state_hidden);
                    self.fullscreen = self.x.has_state(self.x.atoms.fullscreen);
                    self.set_vis(VisEvent::WmHidden(hidden));
                    Handled::Frame
                } else if e.window == win && e.atom == self.x.atoms.wm_state {
                    self.minimized = self.x.iconic();
                    Handled::Frame
                } else {
                    Handled::Nothing
                }
            }
            XEvent::MappingNotify(_) => {
                self.x.refresh_keymap();
                Handled::Nothing
            }
            XEvent::MotionNotify(e) => {
                if let Some((sx, sy, wx, wy)) = self.manual_drag {
                    self.x
                        .move_to(wx + e.root_x as i32 - sx, wy + e.root_y as i32 - sy);
                    return Handled::Nothing;
                }
                let p = pos2(e.event_x as f32, e.event_y as f32);
                if let Some(old) = self.pointer.pos {
                    self.pointer.delta = self.pointer.delta + (p - old);
                }
                self.pointer.pos = Some(p);
                self.mods = mods(e.state);
                Handled::Frame
            }
            XEvent::LeaveNotify(_) => {
                if self.pointer.primary_down || self.manual_drag.is_some() {
                    return Handled::Nothing;
                }
                self.pointer.pos = None;
                Handled::Frame
            }
            XEvent::ButtonPress(e) => {
                self.pointer.pos = Some(pos2(e.event_x as f32, e.event_y as f32));
                if e.detail == 1 {
                    self.press_root = (e.root_x as i32, e.root_y as i32, self.pos.0, self.pos.1);
                }
                self.mods = mods(e.state);
                match e.detail {
                    1 => {
                        self.pointer.primary_pressed = true;
                        self.pointer.primary_down = true;
                    }
                    3 => {
                        self.pointer.secondary_pressed = true;
                        self.pointer.secondary_down = true;
                    }
                    4 => self.scroll += 50.0,
                    5 => self.scroll -= 50.0,
                    _ => return Handled::Nothing,
                }
                Handled::Frame
            }
            XEvent::ButtonRelease(e) => {
                if self.manual_drag.take().is_some() {
                    let _ = self.x.c.ungrab_pointer(x11rb::CURRENT_TIME);
                    if let Some(p) = self.x.root_position() {
                        self.pos = p;
                    }
                    self.pointer.primary_down = false;
                    self.pointer.primary_released = true;
                    return Handled::Frame;
                }
                self.pointer.pos = Some(pos2(e.event_x as f32, e.event_y as f32));
                match e.detail {
                    1 => {
                        self.pointer.primary_released = true;
                        self.pointer.primary_down = false;
                    }
                    3 => {
                        self.pointer.secondary_released = true;
                        self.pointer.secondary_down = false;
                    }
                    _ => return Handled::Nothing,
                }
                Handled::Frame
            }
            XEvent::KeyPress(e) => {
                self.mods = mods(e.state);
                let sym = self.x.keysym(e.detail, e.state);
                let m = self.mods;
                if let Some(key) = key_of(sym) {
                    self.events.push(Event::Key {
                        key,
                        pressed: true,
                        modifiers: m,
                    });
                }
                if !m.ctrl && !m.alt {
                    if let Some(c) = char_of(sym) {
                        self.events.push(Event::Text(c.to_string()));
                    }
                }
                Handled::Frame
            }
            XEvent::KeyRelease(e) => {
                self.mods = mods(e.state);
                Handled::Nothing
            }
            XEvent::FocusOut(_) => {
                self.mods = Modifiers::default();
                Handled::Nothing
            }
            XEvent::ClientMessage(e) => {
                let a = &self.x.atoms;
                let d = e.data.as_data32();
                if e.type_ == a.wm_protocols && d[0] == a.wm_delete {
                    return Handled::Quit;
                }
                self.xdnd(e.type_, d)
            }
            XEvent::SelectionNotify(e) => {
                if e.selection == self.x.atoms.xdnd_selection {
                    let data = if e.property != x11rb::NONE {
                        self.x.take_property(self.x.atoms.dnd_prop)
                    } else {
                        None
                    };
                    let ok = data.is_some();
                    for item in data
                        .map(|d| crate::x::parse_uri_list(&d))
                        .unwrap_or_default()
                    {
                        if item.starts_with('/') {
                            let path = std::path::PathBuf::from(&item);
                            self.dropped.push(DroppedFile {
                                name: path
                                    .file_name()
                                    .map(|n| n.to_string_lossy().into_owned())
                                    .unwrap_or_default(),
                                path: Some(path),
                                bytes: None,
                            });
                        } else {
                            self.dropped.push(DroppedFile {
                                path: None,
                                name: item,
                                bytes: None,
                            });
                        }
                    }
                    if let Some(din) = self.drag_in.take() {
                        let action = if ok { self.x.atoms.xdnd_copy } else { 0 };
                        self.x.send_client(
                            din.source,
                            self.x.atoms.xdnd_finished,
                            [self.x.win, u32::from(ok), action, 0, 0],
                        );
                    }
                    Handled::Frame
                } else if e.selection == self.x.atoms.clipboard {
                    if e.property != x11rb::NONE {
                        if let Some(d) = self.x.take_property(self.x.atoms.sel_prop) {
                            let text = String::from_utf8_lossy(&d).replace(['\n', '\r'], " ");
                            self.events.push(Event::Paste(text));
                        }
                    }
                    Handled::Frame
                } else {
                    Handled::Nothing
                }
            }
            XEvent::SelectionRequest(e) => {
                let text = self.clipboard.clone();
                self.x.answer_text(&e, &text);
                Handled::Nothing
            }
            _ => Handled::Nothing,
        }
    }

    fn xdnd(&mut self, t: u32, d: [u32; 5]) -> Handled {
        let a = &self.x.atoms;
        let win = self.x.win;
        if t == a.xdnd_enter {
            let source = d[0];
            let mut types: Vec<u32> = d[2..5].iter().copied().filter(|x| *x != 0).collect();
            if d[1] & 1 == 1 {
                if let Some(v) = self
                    .x
                    .c
                    .get_property(false, source, a.xdnd_type_list, AtomEnum::ATOM, 0, 64)
                    .ok()
                    .and_then(|c| c.reply().ok())
                {
                    types = v.value32().map(|i| i.collect()).unwrap_or_default();
                }
            }
            self.drag_in = Some(DragIn {
                source,
                uris: types.contains(&a.uri_list),
            });
        } else if t == a.xdnd_position {
            if let Some(din) = self.drag_in.as_ref() {
                let accept = u32::from(din.uris);
                self.x
                    .send_client(din.source, a.xdnd_status, [win, accept, 0, 0, a.xdnd_copy]);
            }
        } else if t == a.xdnd_leave {
            self.drag_in = None;
        } else if t == a.xdnd_drop {
            let time = d[2];
            if self.drag_in.as_ref().is_some_and(|x| x.uris) {
                let _ =
                    self.x
                        .c
                        .convert_selection(win, a.xdnd_selection, a.uri_list, a.dnd_prop, time);
                let _ = self.x.c.flush();
            } else if let Some(din) = self.drag_in.take() {
                self.x
                    .send_client(din.source, a.xdnd_finished, [win, 0, 0, 0, 0]);
            }
        }
        Handled::Nothing
    }

    /// Apply a viewport command. True means quit.
    fn command(&mut self, cmd: ViewportCommand, app: &mut NiruampApp) -> bool {
        let _ = app;
        match cmd {
            ViewportCommand::Close => return true,
            ViewportCommand::InnerSize(v) => {
                self.x
                    .resize(v.x.round().max(1.0) as u32, v.y.round().max(1.0) as u32);
            }
            ViewportCommand::WindowLevel(l) => {
                self.x.set_above(l == gui::WindowLevel::AlwaysOnTop);
            }
            ViewportCommand::Visible(true) => {
                let _ = self.x.map();
            }
            ViewportCommand::Visible(false) => {
                self.x.unmap();
                // Nothing is drawn while hidden; give the buffers back.
                self.x.release_buffers();
                self.painted = false;
            }
            ViewportCommand::Minimized(true) => self.x.iconify(),
            ViewportCommand::Minimized(false) => {
                let _ = self.x.map();
                self.x.activate();
            }
            ViewportCommand::Focus => self.x.activate(),
            ViewportCommand::StartDrag => {
                let root = self
                    .x
                    .c
                    .query_pointer(self.x.win)
                    .ok()
                    .and_then(|c| c.reply().ok());
                if let Some(r) = root {
                    if self.x.moveresize {
                        self.x.start_wm_move(r.root_x, r.root_y);
                        // The WM owns the pointer now; our button is up.
                        self.pointer.primary_down = false;
                    } else if self.pointer.primary_down {
                        // Follow the pointer from where the button went down,
                        // so the drag threshold is not lost.
                        let (sx, sy, wx, wy) = self.press_root;
                        self.manual_drag = Some((sx, sy, wx, wy));
                        self.x
                            .move_to(wx + r.root_x as i32 - sx, wy + r.root_y as i32 - sy);
                    }
                }
            }
            ViewportCommand::Fullscreen(on) => {
                self.fullscreen = on;
                self.x.set_fullscreen(on);
            }
            ViewportCommand::Screenshot(path) => {
                self.screenshot = Some(path);
                self.ctx.request_repaint();
            }
            ViewportCommand::Paste => self.x.request_clipboard_text(),
            ViewportCommand::Copy(text) => {
                self.clipboard = text;
                self.x.own_clipboard();
            }
        }
        false
    }
}

enum Handled {
    Nothing,
    Frame,
    Present,
    Quit,
}

fn mods(state: KeyButMask) -> Modifiers {
    let ctrl = state.contains(KeyButMask::CONTROL);
    Modifiers {
        shift: state.contains(KeyButMask::SHIFT),
        ctrl,
        alt: state.contains(KeyButMask::MOD1),
        command: ctrl,
    }
}

fn key_of(sym: u32) -> Option<Key> {
    Some(match sym {
        0x20 => Key::Space,
        0xff0d | 0xff8d => Key::Enter,
        0xff1b => Key::Escape,
        0xff09 | 0xfe20 => Key::Tab,
        0xff08 => Key::Backspace,
        0xffff | 0xff9f => Key::Delete,
        0xff52 | 0xff97 => Key::ArrowUp,
        0xff54 | 0xff99 => Key::ArrowDown,
        0xff51 | 0xff96 => Key::ArrowLeft,
        0xff53 | 0xff98 => Key::ArrowRight,
        0xff50 | 0xff95 => Key::Home,
        0xff57 | 0xff9c => Key::End,
        0xff55 | 0xff9a => Key::PageUp,
        0xff56 | 0xff9b => Key::PageDown,
        0xffc8 => Key::F11,
        0x2f | 0xffaf => Key::Slash,
        0x2b | 0xffab => Key::Plus,
        0x3d => Key::Equals,
        0x2d | 0xffad => Key::Minus,
        0x30..=0x39 => [
            Key::Num0,
            Key::Num1,
            Key::Num2,
            Key::Num3,
            Key::Num4,
            Key::Num5,
            Key::Num6,
            Key::Num7,
            Key::Num8,
            Key::Num9,
        ][(sym - 0x30) as usize],
        0x41..=0x5a | 0x61..=0x7a => {
            const L: [Key; 26] = [
                Key::A,
                Key::B,
                Key::C,
                Key::D,
                Key::E,
                Key::F,
                Key::G,
                Key::H,
                Key::I,
                Key::J,
                Key::K,
                Key::L,
                Key::M,
                Key::N,
                Key::O,
                Key::P,
                Key::Q,
                Key::R,
                Key::S,
                Key::T,
                Key::U,
                Key::V,
                Key::W,
                Key::X,
                Key::Y,
                Key::Z,
            ];
            L[((sym | 0x20) - 0x61) as usize]
        }
        _ => return None,
    })
}

/// The character a keysym types: Latin-1 directly, Unicode keysyms
/// (0x01000000 + code point), keypad digits.
fn char_of(sym: u32) -> Option<char> {
    match sym {
        0x20..=0x7e | 0xa0..=0xff => char::from_u32(sym),
        0xffb0..=0xffb9 => char::from_u32(sym - 0xffb0 + 0x30),
        0xffaa => Some('*'),
        0xffab => Some('+'),
        0xffad => Some('-'),
        0xffae => Some('.'),
        0xffaf => Some('/'),
        s if s & 0xff00_0000 == 0x0100_0000 => char::from_u32(s & 0x00ff_ffff),
        _ => None,
    }
}

fn save_png(pm: &tiny_skia::Pixmap, path: &std::path::Path) {
    let mut rgba = Vec::with_capacity(pm.data().len());
    for px in pm.data().chunks_exact(4) {
        rgba.extend_from_slice(&[px[0], px[1], px[2], 255]);
    }
    match image::RgbaImage::from_raw(pm.width(), pm.height(), rgba) {
        Some(img) => match img.save(path) {
            Ok(()) => eprintln!("niruamp: screenshot saved {}", path.display()),
            Err(e) => eprintln!("niruamp: screenshot save failed: {e}"),
        },
        None => eprintln!("niruamp: screenshot failed"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keysyms_map_to_keys_and_text() {
        assert_eq!(key_of(0x6e), Some(Key::N));
        assert_eq!(key_of(0x4e), Some(Key::N));
        assert_eq!(key_of(0x2f), Some(Key::Slash));
        assert_eq!(key_of(0xffc8), Some(Key::F11));
        assert_eq!(char_of(0xe5), Some('å'));
        assert_eq!(char_of(0x0100_20ac), Some('€'));
        assert_eq!(char_of(0xff0d), None);
    }

    #[test]
    fn waker_wakes_and_drains() {
        let w = Waker::new().unwrap();
        assert!(!w.drain());
        (w.wake())();
        (w.wake())();
        assert!(w.drain());
        assert!(!w.drain());
    }
}
