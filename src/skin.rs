//! Deck skins: Classic, Phosphor and Hi-Fi Deck.
//!
//! A skin is a palette transform plus a few drawing helpers. Every skin takes
//! its colours from the active NIRUX (or Omarchy) theme: the theme's accent
//! drives highlights, LEDs and focus, its foreground the text. All artwork is
//! original and drawn in code; the only bitmaps are small generated textures
//! (gradients, brushed metal, the CRT glass) that are built once per size and
//! palette and then reused, so an idle deck costs nothing.
use crate::gui::{
    pos2, vec2, Align2, Color32, ColorImage, Context, FontId, Painter, Pos2, Rect, Rounding, Shape,
    Stroke, TextureHandle, TextureOptions,
};
use crate::theme::Palette;
use std::hash::{Hash, Hasher};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Skin {
    /// The Winamp-inspired hardware panel.
    #[default]
    Classic,
    /// A green CRT operator console.
    Phosphor,
    /// A 1980s stereo cassette deck.
    HiFi,
}

impl Skin {
    pub const ALL: [Skin; 3] = [Skin::Classic, Skin::Phosphor, Skin::HiFi];

    pub fn key(self) -> &'static str {
        match self {
            Skin::Classic => "classic",
            Skin::Phosphor => "phosphor",
            Skin::HiFi => "hifi",
        }
    }

    pub fn from_key(key: &str) -> Self {
        match key.trim().to_ascii_lowercase().as_str() {
            "phosphor" => Skin::Phosphor,
            "hifi" | "hi-fi" | "hifi-deck" | "hi-fi-deck" => Skin::HiFi,
            _ => Skin::Classic,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Skin::Classic => "Classic",
            Skin::Phosphor => "Phosphor",
            Skin::HiFi => "Hi-Fi Deck",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Skin::Classic => Skin::Phosphor,
            Skin::Phosphor => Skin::HiFi,
            Skin::HiFi => Skin::Classic,
        }
    }

    /// The whole window's palette under this skin, derived from the theme.
    pub fn palette(self, theme: &Palette) -> Palette {
        match self {
            Skin::Classic => Palette {
                skin: Skin::Classic,
                ..*theme
            },
            Skin::Phosphor => phosphor_palette(theme),
            Skin::HiFi => hifi_palette(theme),
        }
    }
}

// ------------------------------------------------------------------ colour

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(
        f(a.r(), b.r()),
        f(a.g(), b.g()),
        f(a.b(), b.b()),
        f(a.a(), b.a()),
    )
}

pub fn alpha(c: Color32, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (a.clamp(0.0, 1.0) * 255.0) as u8)
}

fn luma(c: Color32) -> f32 {
    (0.2126 * c.r() as f32 + 0.7152 * c.g() as f32 + 0.0722 * c.b() as f32) / 255.0
}

/// 0 for greys, up to 1 for pure hues.
fn saturation(c: Color32) -> f32 {
    let max = c.r().max(c.g()).max(c.b()) as f32;
    let min = c.r().min(c.g()).min(c.b()) as f32;
    if max <= 0.0 {
        0.0
    } else {
        (max - min) / max
    }
}

/// The colour with its brightest channel raised to `peak`.
fn brighten_to(c: Color32, peak: f32) -> Color32 {
    let max = c.r().max(c.g()).max(c.b()).max(1) as f32;
    let k = peak / max;
    let f = |x: u8| (x as f32 * k).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgb(f(c.r()), f(c.g()), f(c.b()))
}

/// A tint pulled toward the theme accent by how colourful that accent is:
/// a grey accent leaves `base` alone, a vivid one shifts it by up to `amount`.
fn themed_tint(base: Color32, accent: Color32, amount: f32) -> Color32 {
    let pull = amount * saturation(accent).clamp(0.0, 1.0);
    brighten_to(mix(base, brighten_to(accent, 255.0), pull), 250.0)
}

/// Classic P1-style phosphor green.
const PHOSPHOR_GREEN: Color32 = Color32::from_rgb(70, 255, 130);
/// VFD blue-green.
const VFD_CYAN: Color32 = Color32::from_rgb(110, 245, 225);

/// The phosphor colour for this theme (green, tinted by a vivid accent).
pub fn phosphor(theme: &Palette) -> Color32 {
    themed_tint(PHOSPHOR_GREEN, theme.accent, 0.35)
}

/// The vacuum-fluorescent display colour of the Hi-Fi deck.
pub fn vfd(theme: &Palette) -> Color32 {
    themed_tint(VFD_CYAN, theme.accent, 0.45)
}

fn phosphor_palette(theme: &Palette) -> Palette {
    let phos = phosphor(theme);
    let black = Color32::BLACK;
    let glass = mix(phos, black, 0.94);
    let surface = mix(phos, black, 0.89);
    let accent = if saturation(theme.accent) > 0.25 {
        brighten_to(theme.accent, 245.0)
    } else {
        mix(phos, Color32::WHITE, 0.45)
    };
    let foreground = mix(phos, mix(theme.foreground, Color32::WHITE, 0.3), 0.30);
    Palette {
        background: glass,
        surface,
        surface_hi: mix(phos, black, 0.80),
        panel: mix(phos, black, 0.92),
        inset: mix(phos, black, 0.965),
        foreground,
        muted: mix(phos, glass, 0.42),
        border: mix(phos, black, 0.72),
        border_hi: mix(phos, black, 0.55),
        focus: accent,
        accent,
        selected: mix(surface, phos, 0.24),
        success: phos,
        warning: Color32::from_rgb(235, 205, 110),
        error: Color32::from_rgb(240, 120, 110),
        disabled: mix(phos, glass, 0.68),
        led: mix(phos, Color32::WHITE, 0.18),
        led_dim: mix(phos, black, 0.86),
        vis: phos,
        vis_peak: accent,
        shadow: black,
        light: false,
        skin: Skin::Phosphor,
    }
}

fn hifi_palette(theme: &Palette) -> Palette {
    // Graphite panels below a brushed-metal deck; a light theme gets a
    // lighter graphite but stays a dark hi-fi rack.
    let graphite = Color32::from_rgb(28, 29, 31);
    let tint = if theme.light {
        mix(graphite, theme.background, 0.10)
    } else {
        mix(graphite, theme.background, 0.35)
    };
    let fg = if theme.light {
        Color32::from_rgb(232, 230, 224)
    } else {
        mix(theme.foreground, Color32::WHITE, 0.15)
    };
    let accent = if luma(theme.accent) < 0.35 {
        mix(theme.accent, Color32::WHITE, 0.35)
    } else {
        theme.accent
    };
    let accent = if saturation(accent) < 0.2 {
        // A grey accent gets the deck's warm amber LED.
        Color32::from_rgb(242, 170, 70)
    } else {
        accent
    };
    let surface = mix(tint, Color32::WHITE, 0.06);
    Palette {
        background: tint,
        surface,
        surface_hi: mix(tint, Color32::WHITE, 0.13),
        panel: mix(tint, Color32::BLACK, 0.18),
        inset: mix(tint, Color32::BLACK, 0.45),
        foreground: fg,
        muted: mix(fg, tint, 0.38),
        border: mix(tint, Color32::WHITE, 0.10),
        border_hi: mix(tint, Color32::WHITE, 0.28),
        focus: accent,
        accent,
        selected: mix(surface, accent, 0.24),
        success: Color32::from_rgb(143, 191, 127),
        warning: Color32::from_rgb(215, 179, 106),
        error: Color32::from_rgb(215, 122, 122),
        disabled: mix(fg, tint, 0.62),
        led: vfd(theme),
        led_dim: mix(vfd(theme), Color32::BLACK, 0.86),
        vis: vfd(theme),
        vis_peak: mix(vfd(theme), Color32::WHITE, 0.45),
        shadow: Color32::from_rgb(6, 6, 7),
        light: false,
        skin: Skin::HiFi,
    }
}

/// Brushed aluminium for the Hi-Fi faceplate, kept neutral with a whisper of
/// the theme's background so it sits with the rest of the desktop.
pub fn metal(theme_palette: &Palette) -> Color32 {
    mix(
        Color32::from_rgb(178, 180, 182),
        theme_palette.background,
        0.12,
    )
}

// ---------------------------------------------------------------- textures

/// Generated textures, cached by kind, size and colours (a handful at most).
#[derive(Default)]
pub struct SkinArt {
    entries: Vec<(u64, TextureHandle)>,
}

impl SkinArt {
    fn get(
        &mut self,
        ctx: &Context,
        key: impl Hash,
        make: impl FnOnce() -> ColorImage,
    ) -> TextureHandle {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut h);
        let key = h.finish();
        if let Some(i) = self.entries.iter().position(|(k, _)| *k == key) {
            let entry = self.entries.remove(i);
            let tex = entry.1.clone();
            self.entries.push(entry);
            return tex;
        }
        let tex = ctx.load_texture("skin", make(), TextureOptions);
        self.entries.push((key, tex.clone()));
        if self.entries.len() > 8 {
            self.entries.remove(0);
        }
        tex
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Brushed metal at `w` x `h` (drawn 1:1, the cheapest blit):
    /// fine horizontal streaks, a soft sheen and grain.
    pub fn brushed(&mut self, ctx: &Context, base: Color32, w: u32, h: u32) -> TextureHandle {
        let (w, h) = (w.clamp(1, 4096), h.clamp(1, 1024));
        self.get(ctx, ("brushed", base.0, w, h), || {
            brushed_image(base, w as usize, h as usize)
        })
    }

    /// CRT glass for a `w` x `h` screen: scanlines, vignette, a top sheen.
    pub fn crt(&mut self, ctx: &Context, w: u32, h: u32) -> TextureHandle {
        self.get(ctx, ("crt", w, h), || crt_image(w.max(1), h.max(1)))
    }
}

fn hash2(x: u32, y: u32) -> f32 {
    let mut n = x.wrapping_mul(374_761_393) ^ y.wrapping_mul(668_265_263);
    n = (n ^ (n >> 13)).wrapping_mul(1_274_126_177);
    n ^= n >> 16;
    (n & 0xffff) as f32 / 65535.0
}

fn brushed_image(base: Color32, w: usize, h: usize) -> ColorImage {
    let mut rgba = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        let row = hash2(7, y as u32) - 0.5;
        let fy = y as f32 / h as f32;
        for x in 0..w {
            // Long streaks: value noise along x, a new pattern every row.
            let span = 23.0 + 40.0 * hash2(y as u32, 3);
            let fx = x as f32 / span;
            let i = fx.floor() as u32;
            let t = fx.fract();
            let t = t * t * (3.0 - 2.0 * t);
            let streak = hash2(i, y as u32) * (1.0 - t) + hash2(i + 1, y as u32) * t - 0.5;
            let grain = hash2(x as u32, y as u32 + 977) - 0.5;
            let u = x as f32 / w as f32;
            let sheen = (-((u - 0.32) * (u - 0.32)) / 0.03).exp() * 0.9
                + (-((u - 0.78) * (u - 0.78)) / 0.05).exp() * 0.35;
            let d = row * 9.0 + streak * 7.0 + grain * 4.0 + sheen * 16.0 + (0.5 - fy) * 14.0;
            let f = |c: u8| (c as f32 + d).round().clamp(0.0, 255.0) as u8;
            rgba.extend_from_slice(&[f(base.r()), f(base.g()), f(base.b()), 255]);
        }
    }
    ColorImage::from_rgba_unmultiplied([w, h], &rgba)
}

fn crt_image(w: u32, h: u32) -> ColorImage {
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        let v = y as f32 / h as f32;
        let scan = y % 3 == 2;
        for x in 0..w {
            let u = x as f32 / w as f32;
            // Vignette toward the corners of the tube.
            let dx = (u - 0.5) * 2.0;
            let dy = (v - 0.5) * 2.0;
            let r = (dx * dx * 0.55 + dy * dy * 0.85).min(1.6);
            let mut dark = (r * r * 0.42).min(0.7);
            if scan {
                dark = (dark + 0.34).min(0.85);
            }
            // A faint reflection across the upper glass.
            let sheen = if !scan && v < 0.32 {
                (0.32 - v) / 0.32 * 0.07 * (1.0 - (u - 0.4).abs())
            } else {
                0.0
            };
            if sheen > dark {
                rgba.extend_from_slice(&[255, 255, 255, (sheen * 255.0) as u8]);
            } else {
                rgba.extend_from_slice(&[0, 0, 0, (dark * 255.0) as u8]);
            }
        }
    }
    ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba)
}

// ----------------------------------------------------------------- drawing

/// A vertical gradient as whole-pixel bands: as smooth as a stretched
/// texture for these subtle ramps, and far cheaper to repaint.
pub fn gradient_bands(painter: &Painter, rect: Rect, top: Color32, bottom: Color32) {
    let y0 = rect.top().round();
    let y1 = rect.bottom().round();
    let x0 = rect.left().round();
    let x1 = rect.right().round();
    let h = (y1 - y0).max(1.0);
    let bands = (h / 3.0).ceil().clamp(1.0, 48.0) as usize;
    for k in 0..bands {
        let a = (y0 + h * k as f32 / bands as f32).round();
        let b = (y0 + h * (k + 1) as f32 / bands as f32).round();
        if b > a {
            let t = (k as f32 + 0.5) / bands as f32;
            painter.rect_filled(
                Rect::from_min_max(pos2(x0, a), pos2(x1, b)),
                Rounding::ZERO,
                mix(top, bottom, t),
            );
        }
    }
}

/// Text with a soft phosphor halo.
pub fn glow_text(
    painter: &Painter,
    pos: Pos2,
    anchor: Align2,
    text: &str,
    font: FontId,
    color: Color32,
) -> Rect {
    let halo = alpha(color, 0.22);
    for d in [
        vec2(-1.0, 0.0),
        vec2(1.0, 0.0),
        vec2(0.0, -1.0),
        vec2(0.0, 1.0),
    ] {
        painter.text(pos + d, anchor, text, font, halo);
    }
    painter.text(pos, anchor, text, font, color)
}

/// Seven-segment digits with a glow around the lit segments.
pub fn glow_segments(painter: &Painter, rect: Rect, text: &str, on: Color32, off: Color32) {
    crate::views::common::seven_segment(
        painter,
        rect.expand(1.4),
        text,
        alpha(on, 0.22),
        Color32::TRANSPARENT,
    );
    crate::views::common::seven_segment(painter, rect, text, on, off);
}

/// An analogue VU meter: backlit face, scale, red zone and a needle at
/// `level` (0..1, the spectrum's display scale; 0.8 is 0 VU).
pub fn vu_meter(painter: &Painter, rect: Rect, level: f32, label: &str, p: &Palette) {
    let face_top = Color32::from_rgb(250, 236, 196);
    let face_bottom = Color32::from_rgb(226, 196, 132);
    let face_top = mix(face_top, p.accent, 0.10);
    let face_bottom = mix(face_bottom, p.accent, 0.12);
    // Bezel.
    painter.rect_filled(rect, Rounding::same(3.0), Color32::from_rgb(20, 20, 22));
    let face = rect.shrink(2.5);
    gradient_bands(painter, face, face_top, face_bottom);
    let ink = Color32::from_rgb(40, 34, 28);
    let red = Color32::from_rgb(196, 52, 40);
    let radius = (face.height() * 0.92).min(face.width() * 0.66);
    let pivot = pos2(face.center().x, face.bottom() + radius * 0.33);
    let angle = |v: f32| (-46.0 + 92.0 * v.clamp(0.0, 1.0)).to_radians();
    let at = |v: f32, r: f32| {
        let a = angle(v);
        pos2(pivot.x + r * a.sin(), pivot.y - r * a.cos())
    };
    // dB relative to 0 VU on the display scale: 36 dB span, 0 VU at 0.8.
    let db = |d: f32| 0.8 + d / 36.0;
    let arc_r = radius * 0.74;
    let steps = 24;
    let mut arc = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        arc.push(at(
            db(-20.0) + (db(0.0) - db(-20.0)) * i as f32 / steps as f32,
            arc_r,
        ));
    }
    painter.add(Shape::line(arc, Stroke::new(1.0, ink)));
    let mut red_arc = Vec::new();
    for i in 0..=8 {
        red_arc.push(at(db(0.0) + (db(3.0) - db(0.0)) * i as f32 / 8.0, arc_r));
    }
    painter.add(Shape::line(red_arc, Stroke::new(2.2, red)));
    for (d, label_text) in [
        (-20.0, Some("20")),
        (-10.0, Some("10")),
        (-7.0, None),
        (-5.0, Some("5")),
        (-3.0, None),
        (-1.0, None),
        (0.0, Some("0")),
        (1.5, None),
        (3.0, Some("3")),
    ] {
        let v = db(d);
        let color = if d > 0.0 { red } else { ink };
        let long = label_text.is_some();
        painter.line_segment(
            [at(v, arc_r), at(v, arc_r + if long { 4.5 } else { 3.0 })],
            Stroke::new(1.0, color),
        );
        if let Some(t) = label_text {
            painter.text(
                at(v, arc_r + 9.5),
                Align2::CENTER_CENTER,
                t,
                FontId::proportional(6.5),
                color,
            );
        }
    }
    painter.text(
        pos2(face.center().x, face.bottom() - face.height() * 0.36),
        Align2::CENTER_CENTER,
        "VU",
        FontId::proportional(9.0),
        ink,
    );
    painter.text(
        pos2(face.center().x, face.bottom() - face.height() * 0.20),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(6.5),
        alpha(ink, 0.75),
    );
    // Needle, clipped to the face, with a small shadow.
    let clipped = painter.with_clip_rect(face);
    let tip = at(level, radius * 0.93);
    clipped.line_segment(
        [pivot + vec2(1.2, 0.0), tip + vec2(1.2, 1.0)],
        Stroke::new(1.2, alpha(Color32::BLACK, 0.18)),
    );
    clipped.line_segment(
        [pivot, tip],
        Stroke::new(1.2, Color32::from_rgb(25, 22, 20)),
    );
    // The pivot cover at the bottom edge.
    painter.rect_filled(
        Rect::from_min_max(pos2(face.left(), face.bottom() - 5.0), face.right_bottom()),
        Rounding::ZERO,
        Color32::from_rgb(34, 32, 30),
    );
    // A faint glass reflection.
    painter.rect_filled(
        Rect::from_min_max(
            face.left_top(),
            pos2(face.right(), face.top() + face.height() * 0.22),
        ),
        Rounding::same(2.0),
        alpha(Color32::WHITE, 0.10),
    );
}

/// Points of a reel hub tooth ring rotated by `angle`.
fn hub(painter: &Painter, center: Pos2, r: f32, angle: f32, color: Color32, hole: Color32) {
    painter.circle_filled(center, r, color);
    painter.circle_filled(center, r * 0.55, hole);
    for k in 0..6 {
        let a = angle + k as f32 * std::f32::consts::TAU / 6.0;
        let (s, c) = a.sin_cos();
        let d = vec2(s, -c);
        let n = vec2(c, s);
        let inner = r * 0.30;
        let outer = r * 0.58;
        let w = r * 0.11;
        painter.add(Shape::convex_polygon(
            vec![
                center + d * inner + n * w,
                center + d * outer + n * w,
                center + d * outer - n * w,
                center + d * inner - n * w,
            ],
            color,
            Stroke::NONE,
        ));
    }
}

pub struct Cassette<'a> {
    pub title: &'a str,
    pub artist: &'a str,
    /// Played fraction 0..1 (moves tape from the left reel to the right).
    pub progress: f32,
    /// Seconds played: turns the hubs.
    pub position: f32,
    pub playing: bool,
}

/// A cassette behind a smoked window: label with the track, reels whose
/// tape packs follow the position.
pub fn cassette(painter: &Painter, rect: Rect, tape: &Cassette, p: &Palette) {
    // Smoked window in the door.
    painter.rect_filled(rect, Rounding::same(4.0), Color32::from_rgb(16, 16, 18));
    painter.rect_stroke(
        rect,
        Rounding::same(4.0),
        Stroke::new(1.0, Color32::from_rgb(60, 60, 64)),
    );
    let shell = rect.shrink2(vec2(6.0, 5.0));
    painter.rect_filled(shell, Rounding::same(3.0), Color32::from_rgb(44, 44, 48));
    // Label.
    let label = Rect::from_min_max(
        shell.left_top() + vec2(5.0, 4.0),
        pos2(shell.right() - 5.0, shell.top() + shell.height() * 0.50),
    );
    let paper = mix(Color32::from_rgb(238, 230, 208), p.accent, 0.06);
    painter.rect_filled(label, Rounding::same(2.0), paper);
    let stripe = Rect::from_min_max(
        pos2(label.left(), label.bottom() - 4.0),
        label.right_bottom(),
    );
    painter.rect_filled(stripe, Rounding::ZERO, p.accent);
    let ink = Color32::from_rgb(38, 34, 46);
    painter.text(
        label.left_top() + vec2(4.0, 2.0),
        Align2::LEFT_TOP,
        "A",
        FontId::proportional(13.0),
        p.accent,
    );
    let text_rect = Rect::from_min_max(
        label.left_top() + vec2(17.0, 2.0),
        pos2(label.right() - 4.0, stripe.top() - 1.0),
    );
    let clip = painter.with_clip_rect(text_rect);
    clip.text(
        text_rect.left_top(),
        Align2::LEFT_TOP,
        tape.title,
        FontId::proportional(11.0),
        ink,
    );
    clip.text(
        text_rect.left_top() + vec2(0.0, 14.0),
        Align2::LEFT_TOP,
        tape.artist,
        FontId::proportional(8.0),
        mix(ink, paper, 0.35),
    );
    // Tape window with the two reels.
    let win = Rect::from_min_max(
        pos2(shell.left() + shell.width() * 0.16, label.bottom() + 4.0),
        pos2(shell.right() - shell.width() * 0.16, shell.bottom() - 5.0),
    );
    painter.rect_filled(
        win,
        Rounding::same(win.height() * 0.5),
        Color32::from_rgb(12, 12, 14),
    );
    let r_max = (win.height() * 0.5 - 1.0).min(win.width() * 0.22);
    let r_hub = r_max * 0.42;
    let lc = pos2(win.left() + win.width() * 0.25, win.center().y);
    let rc = pos2(win.right() - win.width() * 0.25, win.center().y);
    let progress = tape.progress.clamp(0.0, 1.0);
    let pack = |f: f32| (r_hub * r_hub + (r_max * r_max - r_hub * r_hub) * f).sqrt();
    let tape_color = Color32::from_rgb(70, 44, 30);
    painter.circle_filled(lc, pack(1.0 - progress), tape_color);
    painter.circle_filled(rc, pack(progress), tape_color);
    // Hubs turn faster on the emptier reel, as on a real transport.
    let spin = |r: f32| tape.position * 3.0 * r_max / r.max(1.0);
    let hub_color = Color32::from_rgb(232, 230, 224);
    hub(
        painter,
        lc,
        r_hub,
        spin(pack(1.0 - progress)),
        hub_color,
        Color32::from_rgb(12, 12, 14),
    );
    hub(
        painter,
        rc,
        r_hub,
        spin(pack(progress)),
        hub_color,
        Color32::from_rgb(12, 12, 14),
    );
    // Guide holes and the head opening at the bottom edge.
    for x in [shell.left() + 9.0, shell.right() - 9.0] {
        painter.circle_filled(
            pos2(x, shell.bottom() - 6.0),
            2.2,
            Color32::from_rgb(16, 16, 18),
        );
    }
    // Smoked glass over everything.
    painter.rect_filled(
        rect,
        Rounding::same(4.0),
        alpha(Color32::from_rgb(20, 18, 26), 0.16),
    );
    painter.line_segment(
        [
            rect.left_top() + vec2(10.0, 2.0),
            rect.right_top() + vec2(-40.0, 2.0),
        ],
        Stroke::new(1.0, alpha(Color32::WHITE, 0.12)),
    );
    if tape.playing {
        painter.circle_filled(rect.right_top() + vec2(-8.0, 8.0), 2.0, p.accent);
    }
}

/// Engraved label text on metal: a light lower edge under dark ink.
pub fn engraved(
    painter: &Painter,
    pos: Pos2,
    anchor: Align2,
    text: &str,
    font: FontId,
    ink: Color32,
) -> Rect {
    painter.text(
        pos + vec2(0.0, 1.0),
        anchor,
        text,
        font,
        alpha(Color32::WHITE, 0.45),
    );
    painter.text(pos, anchor, text, font, ink)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_round_trip_and_cycle() {
        for skin in Skin::ALL {
            assert_eq!(Skin::from_key(skin.key()), skin);
        }
        assert_eq!(Skin::from_key("nonsense"), Skin::Classic);
        assert_eq!(Skin::from_key("Hi-Fi"), Skin::HiFi);
        assert_eq!(Skin::Classic.next().next().next(), Skin::Classic);
    }

    #[test]
    fn skins_follow_the_theme_accent() {
        let theme = crate::theme::classic_palette();
        let phos = Skin::Phosphor.palette(&theme);
        // Still a green phosphor, with the amber accent for highlights.
        assert!(phos.vis.g() > phos.vis.r() && phos.vis.g() > phos.vis.b());
        assert_eq!(phos.skin, Skin::Phosphor);
        assert!(phos.accent.r() > phos.accent.b());
        let hifi = Skin::HiFi.palette(&theme);
        assert_eq!(hifi.accent, theme.accent);
        assert_eq!(Skin::Classic.palette(&theme).background, theme.background);
        // A grey accent (Niru Noir) leaves the phosphor green almost untouched.
        let mut grey = theme;
        grey.accent = Color32::from_rgb(90, 90, 96);
        assert!(saturation(grey.accent) < 0.1);
        let (a, b) = (phosphor(&grey), brighten_to(PHOSPHOR_GREEN, 250.0));
        assert!(
            a.r().abs_diff(b.r()) < 6 && a.g().abs_diff(b.g()) < 6 && a.b().abs_diff(b.b()) < 6
        );
    }

    #[test]
    fn crt_glass_has_scanlines() {
        let img = crt_image(40, 30);
        let a = |x: usize, y: usize| img.rgba[(y * 40 + x) * 4 + 3];
        assert!(a(20, 14) > a(20, 13) + 40, "every third row is darker");
        assert!(a(0, 0) > a(20, 15), "darker corners");
    }

    #[test]
    fn textures_are_cached() {
        let ctx = Context::new();
        let mut art = SkinArt::default();
        let a = art.brushed(&ctx, Color32::from_rgb(170, 170, 170), 64, 32);
        let b = art.brushed(&ctx, Color32::from_rgb(170, 170, 170), 64, 32);
        assert!(std::rc::Rc::ptr_eq(&a.id().0, &b.id().0));
        for i in 0..20 {
            art.crt(&ctx, 10 + i, 10);
        }
        assert!(art.len() <= 8);
    }
}
