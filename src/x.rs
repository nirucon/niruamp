//! The X11 side: one undecorated window, keyboard map, EWMH requests
//! (always on top, full screen, iconify, activate, move), the clipboard for
//! text fields, file drops (XDND) and presenting the canvas with MIT-SHM
//! (one shared-memory copy per frame) or `PutImage` slices without it.
use x11rb::connection::{Connection, RequestConnection};
use x11rb::protocol::shm::ConnectionExt as _;
use x11rb::protocol::xproto::*;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

pub type Res<T> = Result<T, Box<dyn std::error::Error>>;

pub struct Atoms {
    pub wm_protocols: Atom,
    pub wm_delete: Atom,
    pub wm_state: Atom,
    pub wm_change_state: Atom,
    pub net_wm_name: Atom,
    pub net_wm_icon: Atom,
    pub net_wm_pid: Atom,
    pub net_wm_state: Atom,
    pub state_above: Atom,
    pub state_hidden: Atom,
    pub fullscreen: Atom,
    pub net_active_window: Atom,
    pub net_moveresize: Atom,
    pub net_supported: Atom,
    pub motif_hints: Atom,
    pub utf8: Atom,
    pub niruwm_theme: Atom,
    pub clipboard: Atom,
    pub targets: Atom,
    pub uri_list: Atom,
    pub text_plain: Atom,
    pub sel_prop: Atom,
    pub xdnd_aware: Atom,
    pub xdnd_enter: Atom,
    pub xdnd_position: Atom,
    pub xdnd_status: Atom,
    pub xdnd_leave: Atom,
    pub xdnd_drop: Atom,
    pub xdnd_finished: Atom,
    pub xdnd_selection: Atom,
    pub xdnd_type_list: Atom,
    pub xdnd_copy: Atom,
    pub dnd_prop: Atom,
}

struct Shm {
    seg: u32,
    addr: *mut u8,
    size: usize,
}

pub struct X {
    pub c: RustConnection,
    pub root: Window,
    pub win: Window,
    pub depth: u8,
    pub screen_w: u16,
    pub screen_h: u16,
    gc: Gcontext,
    pub atoms: Atoms,
    shm: Option<Shm>,
    shm_ok: bool,
    min_keycode: u8,
    per: u8,
    keysyms: Vec<u32>,
    cursors: [u32; 3],
    pub moveresize: bool,
    slices: Vec<u8>,
    shm_dims: (u32, u32),
}

fn atom(c: &RustConnection, name: &str) -> Res<Atom> {
    Ok(c.intern_atom(false, name.as_bytes())?.reply()?.atom)
}

pub struct Geometry {
    pub w: u16,
    pub h: u16,
    pub pos: Option<(i16, i16)>,
    pub min: (u16, u16),
}

impl X {
    pub fn open(g: &Geometry, title: &str, class: &str) -> Res<Self> {
        let (c, screen) = x11rb::connect(None)?;
        let s = &c.setup().roots[screen];
        let root = s.root;
        let depth = s.root_depth;
        let visual = s.root_visual;
        let (screen_w, screen_h) = (s.width_in_pixels, s.height_in_pixels);
        let atoms = Atoms {
            wm_protocols: atom(&c, "WM_PROTOCOLS")?,
            wm_delete: atom(&c, "WM_DELETE_WINDOW")?,
            wm_state: atom(&c, "WM_STATE")?,
            wm_change_state: atom(&c, "WM_CHANGE_STATE")?,
            net_wm_name: atom(&c, "_NET_WM_NAME")?,
            net_wm_icon: atom(&c, "_NET_WM_ICON")?,
            net_wm_pid: atom(&c, "_NET_WM_PID")?,
            net_wm_state: atom(&c, "_NET_WM_STATE")?,
            state_above: atom(&c, "_NET_WM_STATE_ABOVE")?,
            state_hidden: atom(&c, "_NET_WM_STATE_HIDDEN")?,
            fullscreen: atom(&c, "_NET_WM_STATE_FULLSCREEN")?,
            net_active_window: atom(&c, "_NET_ACTIVE_WINDOW")?,
            net_moveresize: atom(&c, "_NET_WM_MOVERESIZE")?,
            net_supported: atom(&c, "_NET_SUPPORTED")?,
            motif_hints: atom(&c, "_MOTIF_WM_HINTS")?,
            utf8: atom(&c, "UTF8_STRING")?,
            niruwm_theme: atom(&c, "_NIRUWM_THEME")?,
            clipboard: atom(&c, "CLIPBOARD")?,
            targets: atom(&c, "TARGETS")?,
            uri_list: atom(&c, "text/uri-list")?,
            text_plain: atom(&c, "text/plain")?,
            sel_prop: atom(&c, "_NIRUAMP_SELECTION")?,
            xdnd_aware: atom(&c, "XdndAware")?,
            xdnd_enter: atom(&c, "XdndEnter")?,
            xdnd_position: atom(&c, "XdndPosition")?,
            xdnd_status: atom(&c, "XdndStatus")?,
            xdnd_leave: atom(&c, "XdndLeave")?,
            xdnd_drop: atom(&c, "XdndDrop")?,
            xdnd_finished: atom(&c, "XdndFinished")?,
            xdnd_selection: atom(&c, "XdndSelection")?,
            xdnd_type_list: atom(&c, "XdndTypeList")?,
            xdnd_copy: atom(&c, "XdndActionCopy")?,
            dnd_prop: atom(&c, "_NIRUAMP_DND")?,
        };
        let win = c.generate_id()?;
        let (x, y) = g.pos.unwrap_or((0, 0));
        c.create_window(
            depth,
            win,
            root,
            x,
            y,
            g.w.max(1),
            g.h.max(1),
            0,
            WindowClass::INPUT_OUTPUT,
            visual,
            &CreateWindowAux::new()
                .background_pixel(s.black_pixel)
                .bit_gravity(Gravity::NORTH_WEST)
                .event_mask(
                    EventMask::EXPOSURE
                        | EventMask::KEY_PRESS
                        | EventMask::KEY_RELEASE
                        | EventMask::BUTTON_PRESS
                        | EventMask::BUTTON_RELEASE
                        | EventMask::POINTER_MOTION
                        | EventMask::LEAVE_WINDOW
                        | EventMask::STRUCTURE_NOTIFY
                        | EventMask::PROPERTY_CHANGE
                        | EventMask::VISIBILITY_CHANGE
                        | EventMask::FOCUS_CHANGE,
                ),
        )?;
        // The NIRU theme property on the root window changes with the theme.
        c.change_window_attributes(
            root,
            &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE),
        )?;
        let mut class_prop = Vec::new();
        class_prop.extend_from_slice(class.as_bytes());
        class_prop.push(0);
        class_prop.extend_from_slice(class.as_bytes());
        class_prop.push(0);
        c.change_property8(
            PropMode::REPLACE,
            win,
            AtomEnum::WM_CLASS,
            AtomEnum::STRING,
            &class_prop,
        )?;
        c.change_property32(
            PropMode::REPLACE,
            win,
            atoms.xdnd_aware,
            AtomEnum::ATOM,
            &[5],
        )?;
        c.change_property32(
            PropMode::REPLACE,
            win,
            atoms.wm_protocols,
            AtomEnum::ATOM,
            &[atoms.wm_delete],
        )?;
        c.change_property32(
            PropMode::REPLACE,
            win,
            atoms.net_wm_pid,
            AtomEnum::CARDINAL,
            &[std::process::id()],
        )?;
        // Undecorated: NIRUAMP draws its own title strip.
        c.change_property32(
            PropMode::REPLACE,
            win,
            atoms.motif_hints,
            atoms.motif_hints,
            &[2, 0, 0, 0, 0],
        )?;
        // WM_NORMAL_HINTS: minimum size, and the saved position.
        let mut flags = 16u32; // PMinSize
        if g.pos.is_some() {
            flags |= 1 | 4; // USPosition | PPosition
        }
        let mut hints = [0u32; 18];
        hints[0] = flags;
        hints[1] = x as u32;
        hints[2] = y as u32;
        hints[5] = g.min.0 as u32;
        hints[6] = g.min.1 as u32;
        c.change_property32(
            PropMode::REPLACE,
            win,
            AtomEnum::WM_NORMAL_HINTS,
            AtomEnum::WM_SIZE_HINTS,
            &hints,
        )?;
        let gc = c.generate_id()?;
        c.create_gc(gc, win, &CreateGCAux::new().graphics_exposures(0))?;
        let shm_ok = c
            .extension_information(x11rb::protocol::shm::X11_EXTENSION_NAME)?
            .is_some()
            && c.shm_query_version()
                .ok()
                .and_then(|r| r.reply().ok())
                .is_some()
            && std::env::var_os("NIRUAMP_NO_SHM").is_none();
        let setup = c.setup();
        let (min_keycode, max_keycode) = (setup.min_keycode, setup.max_keycode);
        let map = c
            .get_keyboard_mapping(min_keycode, max_keycode - min_keycode + 1)?
            .reply()?;
        // Cursors from the core cursor font: arrow, bottom-right corner, fleur, xterm.
        let font = c.generate_id()?;
        let mut cursors = [0u32; 3];
        if c.open_font(font, b"cursor").is_ok() {
            for (i, glyph) in [14u16, 52, 152].iter().enumerate() {
                if let Ok(id) = c.generate_id() {
                    if c.create_glyph_cursor(
                        id,
                        font,
                        font,
                        *glyph,
                        glyph + 1,
                        0,
                        0,
                        0,
                        0xffff,
                        0xffff,
                        0xffff,
                    )
                    .is_ok()
                    {
                        cursors[i] = id;
                    }
                }
            }
            let _ = c.close_font(font);
        }
        let moveresize = c
            .get_property(false, root, atoms.net_supported, AtomEnum::ATOM, 0, 1024)
            .ok()
            .and_then(|ck| ck.reply().ok())
            .and_then(|r| {
                r.value32()
                    .map(|mut v| v.any(|a| a == atoms.net_moveresize))
            })
            .unwrap_or(false);
        let x = X {
            c,
            root,
            win,
            depth,
            screen_w,
            screen_h,
            gc,
            atoms,
            shm: None,
            shm_ok,
            min_keycode,
            per: map.keysyms_per_keycode,
            keysyms: map.keysyms,
            cursors,
            moveresize,
            slices: Vec::new(),
            shm_dims: (0, 0),
        };
        x.set_title(title)?;
        Ok(x)
    }

    pub fn set_title(&self, title: &str) -> Res<()> {
        self.c.change_property8(
            PropMode::REPLACE,
            self.win,
            self.atoms.net_wm_name,
            self.atoms.utf8,
            title.as_bytes(),
        )?;
        self.c.change_property8(
            PropMode::REPLACE,
            self.win,
            AtomEnum::WM_NAME,
            AtomEnum::STRING,
            title.as_bytes(),
        )?;
        Ok(())
    }

    /// `_NET_WM_ICON`: width, height, then ARGB pixels.
    pub fn set_icon(&self, w: u32, h: u32, argb: &[u32]) -> Res<()> {
        let mut data = vec![w, h];
        data.extend_from_slice(argb);
        self.c.change_property32(
            PropMode::REPLACE,
            self.win,
            self.atoms.net_wm_icon,
            AtomEnum::CARDINAL,
            &data,
        )?;
        Ok(())
    }

    /// Initial `_NET_WM_STATE` before mapping (always on top).
    pub fn set_initial_above(&self, above: bool) {
        if above {
            let _ = self.c.change_property32(
                PropMode::REPLACE,
                self.win,
                self.atoms.net_wm_state,
                AtomEnum::ATOM,
                &[self.atoms.state_above],
            );
        }
    }

    pub fn map(&self) -> Res<()> {
        self.c.map_window(self.win)?;
        self.c.flush()?;
        Ok(())
    }

    pub fn unmap(&self) {
        let _ = self.c.unmap_window(self.win);
        let _ = self.c.flush();
    }

    pub fn root_theme(&self) -> Option<String> {
        let r = self
            .c
            .get_property(
                false,
                self.root,
                self.atoms.niruwm_theme,
                AtomEnum::ANY,
                0,
                64,
            )
            .ok()?
            .reply()
            .ok()?;
        let s = String::from_utf8(r.value).ok()?;
        (!s.is_empty()).then_some(s)
    }

    /// The keysym of a key, with Shift (column 1) or AltGr (columns 4/5).
    pub fn keysym(&self, keycode: u8, state: KeyButMask) -> u32 {
        let idx = (keycode.saturating_sub(self.min_keycode)) as usize * self.per as usize;
        let shift = state.contains(KeyButMask::SHIFT) ^ state.contains(KeyButMask::LOCK);
        let altgr = state.contains(KeyButMask::MOD5);
        let col = match (altgr, shift) {
            (true, false) => 4,
            (true, true) => 5,
            (false, true) => 1,
            (false, false) => 0,
        };
        let get = |c: usize| self.keysyms.get(idx + c).copied().unwrap_or(0);
        let k = get(col);
        if k != 0 {
            return k;
        }
        let base = get(if altgr { 4 } else { 0 });
        if base != 0 {
            if shift && (0x61..=0x7a).contains(&base) {
                return base - 0x20;
            }
            return base;
        }
        get(0)
    }

    pub fn refresh_keymap(&mut self) {
        let setup = self.c.setup();
        let (min, max) = (setup.min_keycode, setup.max_keycode);
        if let Ok(map) = self
            .c
            .get_keyboard_mapping(min, max - min + 1)
            .map_err(|_| ())
            .and_then(|c| c.reply().map_err(|_| ()))
        {
            self.min_keycode = min;
            self.per = map.keysyms_per_keycode;
            self.keysyms = map.keysyms;
        }
    }

    fn ensure_shm(&mut self, bytes: usize) -> bool {
        if !self.shm_ok {
            return false;
        }
        if self
            .shm
            .as_ref()
            .is_some_and(|s| s.size >= bytes && s.size <= bytes * 2 + (1 << 20))
        {
            return true;
        }
        self.drop_shm();
        let size = bytes.next_multiple_of(1 << 16);
        unsafe {
            let id = libc::shmget(libc::IPC_PRIVATE, size, libc::IPC_CREAT | 0o600);
            if id < 0 {
                self.shm_ok = false;
                return false;
            }
            let addr = libc::shmat(id, std::ptr::null(), 0);
            if addr as isize == -1 {
                libc::shmctl(id, libc::IPC_RMID, std::ptr::null_mut());
                self.shm_ok = false;
                return false;
            }
            let Ok(seg) = self.c.generate_id() else {
                return false;
            };
            let attached = self
                .c
                .shm_attach(seg, id as u32, false)
                .ok()
                .and_then(|ck| ck.check().ok())
                .is_some();
            libc::shmctl(id, libc::IPC_RMID, std::ptr::null_mut());
            if !attached {
                libc::shmdt(addr);
                self.shm_ok = false;
                return false;
            }
            self.shm = Some(Shm {
                seg,
                addr: addr as *mut u8,
                size,
            });
        }
        true
    }

    fn drop_shm(&mut self) {
        if let Some(s) = self.shm.take() {
            let _ = self.c.shm_detach(s.seg);
            unsafe { libc::shmdt(s.addr as *const libc::c_void) };
        }
    }

    /// Copy `region` (x, y, w, h) of the pixmap to the window and wait
    /// until the server has read it. The shared buffer keeps the rest of
    /// the frame from before, so only the damaged part is converted.
    pub fn present(&mut self, pm: &tiny_skia::Pixmap, region: (u32, u32, u32, u32)) -> Res<()> {
        let (w, h) = (pm.width(), pm.height());
        let (rx, ry, rw, rh) = region;
        let (rw, rh) = (rw.min(w.saturating_sub(rx)), rh.min(h.saturating_sub(ry)));
        if rw == 0 || rh == 0 {
            return Ok(());
        }
        let bytes = pm.data().len();
        let full = (rx, ry, rw, rh) == (0, 0, w, h);
        let fresh = self
            .shm
            .as_ref()
            .is_none_or(|s| s.size < bytes || s.size > bytes * 2 + (1 << 20))
            || self.shm_dims != (w, h);
        if self.ensure_shm(bytes) {
            let s = self.shm.as_ref().unwrap();
            // SAFETY: the segment is at least `bytes` long and only we write it.
            let out = unsafe { std::slice::from_raw_parts_mut(s.addr, bytes) };
            let (cx, cy, cw, ch) = if fresh {
                (0, 0, w, h)
            } else {
                (rx, ry, rw, rh)
            };
            crate::gui::raster::to_bgrx_region(pm, out, (cx, cy, cw, ch));
            self.shm_dims = (w, h);
            let (px, py, pw, ph) = if fresh || full {
                (0, 0, w, h)
            } else {
                (rx, ry, rw, rh)
            };
            self.c.shm_put_image(
                self.win,
                self.gc,
                w as u16,
                h as u16,
                px as u16,
                py as u16,
                pw as u16,
                ph as u16,
                px as i16,
                py as i16,
                self.depth,
                ImageFormat::Z_PIXMAP.into(),
                false,
                s.seg,
                0,
            )?;
        } else {
            let row_bytes = rw as usize * 4;
            self.slices.resize(row_bytes * rh as usize, 0);
            let mut buf = std::mem::take(&mut self.slices);
            let src = pm.data();
            for y in 0..rh as usize {
                let so = ((ry as usize + y) * w as usize + rx as usize) * 4;
                let d = &mut buf[y * row_bytes..(y + 1) * row_bytes];
                for (sp, dp) in src[so..so + row_bytes]
                    .chunks_exact(4)
                    .zip(d.chunks_exact_mut(4))
                {
                    dp[0] = sp[2];
                    dp[1] = sp[1];
                    dp[2] = sp[0];
                    dp[3] = 0;
                }
            }
            let max = self.c.maximum_request_bytes().saturating_sub(64).max(4096);
            let rows = (max / row_bytes).max(1);
            let mut y = 0usize;
            while y < rh as usize {
                let n = rows.min(rh as usize - y);
                self.c.put_image(
                    ImageFormat::Z_PIXMAP,
                    self.win,
                    self.gc,
                    rw as u16,
                    n as u16,
                    rx as i16,
                    (ry as usize + y) as i16,
                    0,
                    self.depth,
                    &buf[y * row_bytes..(y + n) * row_bytes],
                )?;
                y += n;
            }
            if buf.len() <= 4 << 20 {
                self.slices = buf;
            }
        }
        self.c.get_input_focus()?.reply()?;
        Ok(())
    }

    pub fn release_buffers(&mut self) {
        self.drop_shm();
        self.shm_dims = (0, 0);
        self.slices = Vec::new();
    }

    pub fn set_cursor(&self, icon: crate::gui::CursorIcon) {
        use crate::gui::CursorIcon;
        let c = match icon {
            CursorIcon::Default => 0,
            CursorIcon::ResizeSouthEast => self.cursors[0],
            CursorIcon::Grabbing => self.cursors[1],
            CursorIcon::Text => self.cursors[2],
        };
        let _ = self
            .c
            .change_window_attributes(self.win, &ChangeWindowAttributesAux::new().cursor(c));
        let _ = self.c.flush();
    }

    fn send_root(&self, kind: Atom, data: [u32; 5]) {
        let ev = ClientMessageEvent::new(32, self.win, kind, data);
        let _ = self.c.send_event(
            false,
            self.root,
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
            ev,
        );
        let _ = self.c.flush();
    }

    pub fn set_above(&self, on: bool) {
        self.send_root(
            self.atoms.net_wm_state,
            [u32::from(on), self.atoms.state_above, 0, 1, 0],
        );
    }

    pub fn set_fullscreen(&self, on: bool) {
        self.send_root(
            self.atoms.net_wm_state,
            [u32::from(on), self.atoms.fullscreen, 0, 1, 0],
        );
    }

    pub fn iconify(&self) {
        // ICCCM: WM_CHANGE_STATE with IconicState (3).
        self.send_root(self.atoms.wm_change_state, [3, 0, 0, 0, 0]);
    }

    pub fn activate(&self) {
        self.send_root(
            self.atoms.net_active_window,
            [1, x11rb::CURRENT_TIME, 0, 0, 0],
        );
        let _ = self.c.configure_window(
            self.win,
            &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE),
        );
        let _ = self.c.flush();
    }

    /// Hand the move to the window manager (`_NET_WM_MOVERESIZE`).
    pub fn start_wm_move(&self, root_x: i16, root_y: i16) {
        let _ = self.c.ungrab_pointer(x11rb::CURRENT_TIME);
        self.send_root(
            self.atoms.net_moveresize,
            [root_x as u32, root_y as u32, 8, 1, 1],
        );
    }

    pub fn move_to(&self, x: i32, y: i32) {
        let _ = self
            .c
            .configure_window(self.win, &ConfigureWindowAux::new().x(x).y(y));
        let _ = self.c.flush();
    }

    pub fn resize(&self, w: u32, h: u32) {
        let _ = self.c.configure_window(
            self.win,
            &ConfigureWindowAux::new().width(w.max(1)).height(h.max(1)),
        );
        let _ = self.c.flush();
    }

    /// The window's position on the root window.
    pub fn root_position(&self) -> Option<(i32, i32)> {
        let r = self
            .c
            .translate_coordinates(self.win, self.root, 0, 0)
            .ok()?
            .reply()
            .ok()?;
        Some((r.dst_x as i32, r.dst_y as i32))
    }

    pub fn has_state(&self, a: Atom) -> bool {
        self.c
            .get_property(
                false,
                self.win,
                self.atoms.net_wm_state,
                AtomEnum::ATOM,
                0,
                64,
            )
            .ok()
            .and_then(|c| c.reply().ok())
            .and_then(|r| r.value32().map(|mut v| v.any(|x| x == a)))
            .unwrap_or(false)
    }

    /// ICCCM WM_STATE is IconicState.
    pub fn iconic(&self) -> bool {
        self.c
            .get_property(
                false,
                self.win,
                self.atoms.wm_state,
                self.atoms.wm_state,
                0,
                2,
            )
            .ok()
            .and_then(|c| c.reply().ok())
            .and_then(|r| r.value32().and_then(|mut v| v.next()))
            == Some(3)
    }

    // ------------------------------------------------------- clipboard

    pub fn own_clipboard(&self) {
        let _ = self
            .c
            .set_selection_owner(self.win, self.atoms.clipboard, x11rb::CURRENT_TIME);
        let _ = self.c.flush();
    }

    pub fn request_clipboard_text(&self) {
        let _ = self.c.convert_selection(
            self.win,
            self.atoms.clipboard,
            self.atoms.utf8,
            self.atoms.sel_prop,
            x11rb::CURRENT_TIME,
        );
        let _ = self.c.flush();
    }

    pub fn take_property(&self, prop: Atom) -> Option<Vec<u8>> {
        let r = self
            .c
            .get_property(true, self.win, prop, AtomEnum::ANY, 0, 1 << 22)
            .ok()?
            .reply()
            .ok()?;
        (!r.value.is_empty()).then_some(r.value)
    }

    /// Answer another program's paste request with `text`.
    pub fn answer_text(&self, e: &SelectionRequestEvent, text: &str) {
        let a = &self.atoms;
        let mut prop = if e.property == x11rb::NONE {
            e.target
        } else {
            e.property
        };
        if e.target == a.targets {
            let t = [a.targets, a.utf8, u32::from(AtomEnum::STRING), a.text_plain];
            let _ =
                self.c
                    .change_property32(PropMode::REPLACE, e.requestor, prop, AtomEnum::ATOM, &t);
        } else if e.target == a.utf8
            || e.target == u32::from(AtomEnum::STRING)
            || e.target == a.text_plain
        {
            let _ = self.c.change_property8(
                PropMode::REPLACE,
                e.requestor,
                prop,
                e.target,
                text.as_bytes(),
            );
        } else {
            prop = x11rb::NONE;
        }
        let ev = SelectionNotifyEvent {
            response_type: SELECTION_NOTIFY_EVENT,
            sequence: 0,
            time: e.time,
            requestor: e.requestor,
            selection: e.selection,
            target: e.target,
            property: prop,
        };
        let _ = self
            .c
            .send_event(false, e.requestor, EventMask::NO_EVENT, ev);
        let _ = self.c.flush();
    }

    pub fn send_client(&self, to: Window, kind: Atom, data: [u32; 5]) {
        let ev = ClientMessageEvent::new(32, to, kind, data);
        let _ = self.c.send_event(false, to, EventMask::NO_EVENT, ev);
        let _ = self.c.flush();
    }

    pub fn to_window(&self, x: i16, y: i16) -> (i32, i32) {
        self.c
            .translate_coordinates(self.root, self.win, x, y)
            .ok()
            .and_then(|c| c.reply().ok())
            .map(|r| (r.dst_x as i32, r.dst_y as i32))
            .unwrap_or((-1, -1))
    }
}

/// Paths and URLs from text/uri-list (file URIs decoded, others kept).
pub fn parse_uri_list(data: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(data);
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("file://") {
            let rest = rest.strip_prefix("localhost").unwrap_or(rest);
            out.push(percent_decode(rest));
        } else {
            out.push(line.to_string());
        }
    }
    out
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

impl Drop for X {
    fn drop(&mut self) {
        self.drop_shm();
        let _ = self.c.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_lists_decode_files_and_keep_streams() {
        let v = parse_uri_list(
            b"file:///home/a/My%20Song.flac\r\nhttps://radio.example/live\r\n# c\r\n",
        );
        assert_eq!(
            v,
            vec!["/home/a/My Song.flac", "https://radio.example/live"]
        );
    }
}
