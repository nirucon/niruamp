//! A small immediate-mode GUI on a software canvas.
//!
//! NIRUAMP was written against egui. This module keeps the subset of that
//! API the views use (rects, painter, responses, layouts, a few widgets,
//! popups and scroll areas) so the views port nearly line for line, but it
//! draws with tiny-skia into a CPU framebuffer that `x.rs` presents with
//! MIT-SHM: no GL context, no toolkit threads, nothing when nothing changes.
pub mod raster;
pub mod text;
mod ui;

pub use ui::*;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};
use std::rc::Rc;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------- geometry

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pos2 {
    pub x: f32,
    pub y: f32,
}

pub const fn vec2(x: f32, y: f32) -> Vec2 {
    Vec2 { x, y }
}

pub const fn pos2(x: f32, y: f32) -> Pos2 {
    Pos2 { x, y }
}

impl Vec2 {
    pub const ZERO: Vec2 = vec2(0.0, 0.0);
    pub const fn new(x: f32, y: f32) -> Self {
        vec2(x, y)
    }
    pub fn splat(v: f32) -> Self {
        vec2(v, v)
    }
    pub fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
    pub fn normalized(self) -> Self {
        let l = self.length();
        if l > 0.0 {
            vec2(self.x / l, self.y / l)
        } else {
            self
        }
    }
}

impl From<[f32; 2]> for Vec2 {
    fn from(v: [f32; 2]) -> Self {
        vec2(v[0], v[1])
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, o: Vec2) -> Vec2 {
        vec2(self.x + o.x, self.y + o.y)
    }
}
impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, o: Vec2) -> Vec2 {
        vec2(self.x - o.x, self.y - o.y)
    }
}
impl Mul<f32> for Vec2 {
    type Output = Vec2;
    fn mul(self, f: f32) -> Vec2 {
        vec2(self.x * f, self.y * f)
    }
}
impl Div<f32> for Vec2 {
    type Output = Vec2;
    fn div(self, f: f32) -> Vec2 {
        vec2(self.x / f, self.y / f)
    }
}
impl Neg for Vec2 {
    type Output = Vec2;
    fn neg(self) -> Vec2 {
        vec2(-self.x, -self.y)
    }
}
impl Add<Vec2> for Pos2 {
    type Output = Pos2;
    fn add(self, o: Vec2) -> Pos2 {
        pos2(self.x + o.x, self.y + o.y)
    }
}
impl AddAssign<Vec2> for Pos2 {
    fn add_assign(&mut self, o: Vec2) {
        self.x += o.x;
        self.y += o.y;
    }
}
impl SubAssign<Vec2> for Pos2 {
    fn sub_assign(&mut self, o: Vec2) {
        self.x -= o.x;
        self.y -= o.y;
    }
}
impl Sub<Vec2> for Pos2 {
    type Output = Pos2;
    fn sub(self, o: Vec2) -> Pos2 {
        pos2(self.x - o.x, self.y - o.y)
    }
}
impl Sub for Pos2 {
    type Output = Vec2;
    fn sub(self, o: Pos2) -> Vec2 {
        vec2(self.x - o.x, self.y - o.y)
    }
}

impl Pos2 {
    pub fn to_vec2(self) -> Vec2 {
        vec2(self.x, self.y)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub min: Pos2,
    pub max: Pos2,
}

impl Rect {
    pub const NOTHING: Rect = Rect {
        min: pos2(f32::INFINITY, f32::INFINITY),
        max: pos2(f32::NEG_INFINITY, f32::NEG_INFINITY),
    };
    pub const EVERYTHING: Rect = Rect {
        min: pos2(-1e9, -1e9),
        max: pos2(1e9, 1e9),
    };
    pub fn from_min_max(min: Pos2, max: Pos2) -> Self {
        Rect { min, max }
    }
    pub fn from_min_size(min: Pos2, size: Vec2) -> Self {
        Rect {
            min,
            max: min + size,
        }
    }
    pub fn from_center_size(c: Pos2, size: Vec2) -> Self {
        Rect {
            min: c - size / 2.0,
            max: c + size / 2.0,
        }
    }
    pub fn width(&self) -> f32 {
        self.max.x - self.min.x
    }
    pub fn height(&self) -> f32 {
        self.max.y - self.min.y
    }
    pub fn size(&self) -> Vec2 {
        vec2(self.width(), self.height())
    }
    pub fn area(&self) -> f32 {
        self.width().max(0.0) * self.height().max(0.0)
    }
    pub fn center(&self) -> Pos2 {
        pos2(
            (self.min.x + self.max.x) / 2.0,
            (self.min.y + self.max.y) / 2.0,
        )
    }
    pub fn left(&self) -> f32 {
        self.min.x
    }
    pub fn right(&self) -> f32 {
        self.max.x
    }
    pub fn top(&self) -> f32 {
        self.min.y
    }
    pub fn bottom(&self) -> f32 {
        self.max.y
    }
    pub fn left_top(&self) -> Pos2 {
        self.min
    }
    pub fn right_top(&self) -> Pos2 {
        pos2(self.max.x, self.min.y)
    }
    pub fn left_bottom(&self) -> Pos2 {
        pos2(self.min.x, self.max.y)
    }
    pub fn right_bottom(&self) -> Pos2 {
        self.max
    }
    pub fn left_center(&self) -> Pos2 {
        pos2(self.min.x, self.center().y)
    }
    pub fn right_center(&self) -> Pos2 {
        pos2(self.max.x, self.center().y)
    }
    pub fn center_top(&self) -> Pos2 {
        pos2(self.center().x, self.min.y)
    }
    pub fn center_bottom(&self) -> Pos2 {
        pos2(self.center().x, self.max.y)
    }
    pub fn shrink(&self, d: f32) -> Rect {
        self.shrink2(vec2(d, d))
    }
    pub fn shrink2(&self, d: Vec2) -> Rect {
        Rect::from_min_max(self.min + d, self.max - d)
    }
    pub fn expand(&self, d: f32) -> Rect {
        self.shrink(-d)
    }
    pub fn expand2(&self, d: Vec2) -> Rect {
        self.shrink2(-d)
    }
    pub fn translate(&self, d: Vec2) -> Rect {
        Rect::from_min_max(self.min + d, self.max + d)
    }
    pub fn contains(&self, p: Pos2) -> bool {
        p.x >= self.min.x && p.y >= self.min.y && p.x < self.max.x && p.y < self.max.y
    }
    pub fn contains_rect(&self, o: Rect) -> bool {
        o.min.x >= self.min.x
            && o.min.y >= self.min.y
            && o.max.x <= self.max.x
            && o.max.y <= self.max.y
    }
    pub fn intersect(&self, o: Rect) -> Rect {
        Rect::from_min_max(
            pos2(self.min.x.max(o.min.x), self.min.y.max(o.min.y)),
            pos2(self.max.x.min(o.max.x), self.max.y.min(o.max.y)),
        )
    }
    pub fn intersects(&self, o: Rect) -> bool {
        self.min.x < o.max.x && o.min.x < self.max.x && self.min.y < o.max.y && o.min.y < self.max.y
    }
    pub fn union(&self, o: Rect) -> Rect {
        Rect::from_min_max(
            pos2(self.min.x.min(o.min.x), self.min.y.min(o.min.y)),
            pos2(self.max.x.max(o.max.x), self.max.y.max(o.max.y)),
        )
    }
    pub fn is_positive(&self) -> bool {
        self.width() > 0.0 && self.height() > 0.0
    }
    pub fn set_width(&mut self, w: f32) {
        self.max.x = self.min.x + w;
    }
    pub fn set_height(&mut self, h: f32) {
        self.max.y = self.min.y + h;
    }
}

// ------------------------------------------------------------------ colour

/// RGBA with straight (unmultiplied) alpha.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Color32(pub [u8; 4]);

impl Color32 {
    pub const TRANSPARENT: Color32 = Color32([0, 0, 0, 0]);
    pub const BLACK: Color32 = Color32([0, 0, 0, 255]);
    pub const WHITE: Color32 = Color32([255, 255, 255, 255]);
    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Color32([r, g, b, 255])
    }
    pub const fn from_rgba_unmultiplied(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color32([r, g, b, a])
    }
    pub fn r(&self) -> u8 {
        self.0[0]
    }
    pub fn g(&self) -> u8 {
        self.0[1]
    }
    pub fn b(&self) -> u8 {
        self.0[2]
    }
    pub fn a(&self) -> u8 {
        self.0[3]
    }
    /// Fade: multiplies the opacity.
    pub fn gamma_multiply(self, f: f32) -> Self {
        let mut c = self;
        c.0[3] = (self.0[3] as f32 * f.clamp(0.0, 1.0)).round() as u8;
        c
    }
    pub fn linear_multiply(self, f: f32) -> Self {
        self.gamma_multiply(f)
    }
    pub fn is_transparent(&self) -> bool {
        self.0[3] == 0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stroke {
    pub width: f32,
    pub color: Color32,
}

impl Stroke {
    pub const NONE: Stroke = Stroke {
        width: 0.0,
        color: Color32::TRANSPARENT,
    };
    pub fn new(width: f32, color: Color32) -> Self {
        Stroke { width, color }
    }
    pub fn is_empty(&self) -> bool {
        self.width <= 0.0 || self.color.is_transparent()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rounding(pub f32);

impl Rounding {
    pub const ZERO: Rounding = Rounding(0.0);
    pub const fn same(r: f32) -> Self {
        Rounding(r)
    }
}

impl From<f32> for Rounding {
    fn from(r: f32) -> Self {
        Rounding(r)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Min,
    Center,
    Max,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Align2(pub [Align; 2]);

impl Align2 {
    pub const LEFT_TOP: Align2 = Align2([Align::Min, Align::Min]);
    pub const LEFT_CENTER: Align2 = Align2([Align::Min, Align::Center]);
    pub const LEFT_BOTTOM: Align2 = Align2([Align::Min, Align::Max]);
    pub const CENTER_TOP: Align2 = Align2([Align::Center, Align::Min]);
    pub const CENTER_CENTER: Align2 = Align2([Align::Center, Align::Center]);
    pub const CENTER_BOTTOM: Align2 = Align2([Align::Center, Align::Max]);
    pub const RIGHT_TOP: Align2 = Align2([Align::Max, Align::Min]);
    pub const RIGHT_CENTER: Align2 = Align2([Align::Max, Align::Center]);
    pub const RIGHT_BOTTOM: Align2 = Align2([Align::Max, Align::Max]);

    /// The rect of `size` anchored at `pos` with this alignment.
    pub fn anchor_size(self, pos: Pos2, size: Vec2) -> Rect {
        let x = match self.0[0] {
            Align::Min => pos.x,
            Align::Center => pos.x - size.x / 2.0,
            Align::Max => pos.x - size.x,
        };
        let y = match self.0[1] {
            Align::Min => pos.y,
            Align::Center => pos.y - size.y / 2.0,
            Align::Max => pos.y - size.y,
        };
        Rect::from_min_size(pos2(x, y), size)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FontId {
    pub size: f32,
    pub mono: bool,
}

impl FontId {
    pub fn proportional(size: f32) -> Self {
        FontId { size, mono: false }
    }
    pub fn monospace(size: f32) -> Self {
        FontId { size, mono: true }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Margin {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

impl Margin {
    pub fn same(v: f32) -> Self {
        Margin {
            left: v,
            right: v,
            top: v,
            bottom: v,
        }
    }
    pub fn symmetric(x: f32, y: f32) -> Self {
        Margin {
            left: x,
            right: x,
            top: y,
            bottom: y,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Id(pub u64);

impl Id {
    pub const NULL: Id = Id(0);
    pub fn new(source: impl Hash) -> Self {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        source.hash(&mut h);
        Id(h.finish() | 1)
    }
    pub fn with(self, child: impl Hash) -> Self {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.0.hash(&mut h);
        child.hash(&mut h);
        Id(h.finish() | 1)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    Background,
    Middle,
    Foreground,
    Tooltip,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayerId {
    pub order: Order,
    pub id: Id,
}

impl LayerId {
    pub fn new(order: Order, id: Id) -> Self {
        LayerId { order, id }
    }
    fn index(&self) -> usize {
        match self.order {
            Order::Background | Order::Middle => 0,
            Order::Foreground => 1,
            Order::Tooltip => 2,
        }
    }
}

// ------------------------------------------------------------------ input

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Space,
    Enter,
    Escape,
    Tab,
    Backspace,
    Delete,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Home,
    End,
    PageUp,
    PageDown,
    F11,
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    Num0,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,
    Slash,
    Plus,
    Equals,
    Minus,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub command: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Key {
        key: Key,
        pressed: bool,
        modifiers: Modifiers,
    },
    Text(String),
    Paste(String),
}

#[derive(Clone, Debug, Default)]
pub struct DroppedFile {
    pub path: Option<std::path::PathBuf>,
    pub name: String,
    pub bytes: Option<std::sync::Arc<[u8]>>,
}

#[derive(Clone, Debug, Default)]
pub struct RawInput {
    pub dropped_files: Vec<DroppedFile>,
}

#[derive(Clone, Debug, Default)]
pub struct ViewportInfo {
    pub inner_rect: Option<Rect>,
    pub outer_rect: Option<Rect>,
    pub monitor_size: Option<Vec2>,
    pub minimized: Option<bool>,
    pub fullscreen: Option<bool>,
}

#[derive(Clone, Debug, Default)]
pub struct PointerState {
    pub pos: Option<Pos2>,
    pub delta: Vec2,
    pub primary_pressed: bool,
    pub primary_released: bool,
    pub secondary_pressed: bool,
    pub secondary_released: bool,
    pub primary_down: bool,
    pub secondary_down: bool,
}

impl PointerState {
    pub fn interact_pos(&self) -> Option<Pos2> {
        self.pos
    }
    pub fn hover_pos(&self) -> Option<Pos2> {
        self.pos
    }
    pub fn delta(&self) -> Vec2 {
        self.delta
    }
    pub fn any_released(&self) -> bool {
        self.primary_released || self.secondary_released
    }
    pub fn any_pressed(&self) -> bool {
        self.primary_pressed || self.secondary_pressed
    }
    pub fn primary_down(&self) -> bool {
        self.primary_down
    }
}

#[derive(Clone, Debug, Default)]
pub struct InputState {
    pub pointer: PointerState,
    pub events: Vec<Event>,
    pub modifiers: Modifiers,
    pub scroll_delta: Vec2,
    pub time: f64,
    pub predicted_dt: f32,
    pub raw: RawInput,
    pub viewport: ViewportInfo,
    pub screen_rect: Rect,
}

impl InputState {
    pub fn key_pressed(&self, key: Key) -> bool {
        self.events
            .iter()
            .any(|e| matches!(e, Event::Key { key: k, pressed: true, .. } if *k == key))
    }
    pub fn viewport(&self) -> &ViewportInfo {
        &self.viewport
    }
    pub fn screen_rect(&self) -> Rect {
        self.screen_rect
    }
}

// ----------------------------------------------------------------- viewport

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowLevel {
    Normal,
    AlwaysOnTop,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ViewportCommand {
    Close,
    InnerSize(Vec2),
    WindowLevel(WindowLevel),
    Visible(bool),
    Minimized(bool),
    Focus,
    StartDrag,
    Fullscreen(bool),
    /// Save the next painted frame as PNG.
    Screenshot(std::path::PathBuf),
    /// Ask the CLIPBOARD owner for text; it arrives as `Event::Paste`.
    Paste,
    /// Own the CLIPBOARD with this text.
    Copy(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CursorIcon {
    #[default]
    Default,
    ResizeSouthEast,
    Grabbing,
    Text,
}

// ------------------------------------------------------------------ shapes

pub struct ColorImage {
    pub size: [usize; 2],
    /// Premultiplied RGBA, row-major.
    pub rgba: Vec<u8>,
}

impl ColorImage {
    pub fn from_rgba_unmultiplied(size: [usize; 2], rgba: &[u8]) -> Self {
        let mut out = Vec::with_capacity(rgba.len());
        for px in rgba.chunks_exact(4) {
            let a = px[3] as u16;
            out.push(((px[0] as u16 * a + 127) / 255) as u8);
            out.push(((px[1] as u16 * a + 127) / 255) as u8);
            out.push(((px[2] as u16 * a + 127) / 255) as u8);
            out.push(px[3]);
        }
        ColorImage { size, rgba: out }
    }
    pub fn pixel_count(&self) -> usize {
        self.size[0] * self.size[1]
    }
}

pub struct Texture {
    pub pixmap: tiny_skia::Pixmap,
}

#[derive(Clone)]
pub struct TextureId(pub Rc<Texture>);

#[derive(Clone)]
pub struct TextureHandle(Rc<Texture>);

impl TextureHandle {
    pub fn id(&self) -> TextureId {
        TextureId(self.0.clone())
    }
    pub fn size(&self) -> [usize; 2] {
        [
            self.0.pixmap.width() as usize,
            self.0.pixmap.height() as usize,
        ]
    }
}

#[derive(Clone)]
pub enum Shape {
    Rect {
        rect: Rect,
        rounding: f32,
        fill: Color32,
        stroke: Stroke,
    },
    Line {
        points: Vec<Pos2>,
        stroke: Stroke,
    },
    Polygon {
        points: Vec<Pos2>,
        fill: Color32,
        stroke: Stroke,
    },
    Circle {
        center: Pos2,
        radius: f32,
        fill: Color32,
        stroke: Stroke,
    },
    Text {
        pos: Pos2,
        galley: Rc<text::Galley>,
        color: Color32,
    },
    Image {
        texture: TextureId,
        rect: Rect,
    },
    Noop,
}

impl Shape {
    pub fn convex_polygon(points: Vec<Pos2>, fill: Color32, stroke: Stroke) -> Self {
        Shape::Polygon {
            points,
            fill,
            stroke,
        }
    }
    pub fn line(points: Vec<Pos2>, stroke: Stroke) -> Self {
        Shape::Line { points, stroke }
    }
}

pub struct ClippedShape {
    pub clip: Rect,
    pub shape: Shape,
}

impl Shape {
    /// Where the shape can paint (generous: strokes and anti-aliasing).
    pub fn bounds(&self) -> Rect {
        let pts = |v: &[Pos2], w: f32| {
            v.iter()
                .fold(Rect::NOTHING, |r, p| r.union(Rect::from_min_max(*p, *p)))
                .expand(w + 1.5)
        };
        match self {
            Shape::Rect { rect, stroke, .. } => rect.expand(stroke.width + 1.0),
            Shape::Line { points, stroke } => pts(points, stroke.width),
            Shape::Polygon { points, stroke, .. } => pts(points, stroke.width),
            Shape::Circle {
                center,
                radius,
                stroke,
                ..
            } => Rect::from_center_size(*center, vec2(radius * 2.0, radius * 2.0))
                .expand(stroke.width + 1.5),
            Shape::Text { pos, galley, .. } => Rect::from_min_size(*pos, galley.size).expand2(
                vec2(galley.font_size * 0.5 + 2.0, galley.font_size * 0.4 + 2.0),
            ),
            Shape::Image { rect, .. } => rect.expand(1.0),
            Shape::Noop => Rect::NOTHING,
        }
    }

    /// Same drawing as `other` (text by content, images by identity).
    pub fn same(&self, other: &Shape) -> bool {
        match (self, other) {
            (
                Shape::Rect {
                    rect: a,
                    rounding: b,
                    fill: c,
                    stroke: d,
                },
                Shape::Rect {
                    rect: e,
                    rounding: f,
                    fill: g,
                    stroke: h,
                },
            ) => a == e && b == f && c == g && d == h,
            (
                Shape::Line {
                    points: a,
                    stroke: b,
                },
                Shape::Line {
                    points: c,
                    stroke: d,
                },
            ) => a == c && b == d,
            (
                Shape::Polygon {
                    points: a,
                    fill: b,
                    stroke: c,
                },
                Shape::Polygon {
                    points: d,
                    fill: e,
                    stroke: f,
                },
            ) => a == d && b == e && c == f,
            (
                Shape::Circle {
                    center: a,
                    radius: b,
                    fill: c,
                    stroke: d,
                },
                Shape::Circle {
                    center: e,
                    radius: f,
                    fill: g,
                    stroke: h,
                },
            ) => a == e && b == f && c == g && d == h,
            (
                Shape::Text {
                    pos: a,
                    galley: b,
                    color: c,
                },
                Shape::Text {
                    pos: d,
                    galley: e,
                    color: f,
                },
            ) => {
                a == d
                    && c == f
                    && (Rc::ptr_eq(b, e)
                        || (b.font_size == e.font_size
                            && b.mono == e.mono
                            && b.lines.len() == e.lines.len()
                            && b.lines
                                .iter()
                                .zip(e.lines.iter())
                                .all(|(x, y)| x.text == y.text)))
            }
            (
                Shape::Image {
                    texture: a,
                    rect: b,
                },
                Shape::Image {
                    texture: c,
                    rect: d,
                },
            ) => Rc::ptr_eq(&a.0, &c.0) && b == d,
            (Shape::Noop, Shape::Noop) => true,
            _ => false,
        }
    }
}

/// The area that differs between two frames, or `None` when nothing does.
/// Equal leading and trailing shapes are skipped; a middle of equal length
/// is compared pairwise, otherwise all of it counts (a spectrum whose number
/// of highlight lines changed still only damages the spectrum).
/// The areas that differ between two frames, as a few whole-pixel rects.
/// Changes far apart (a cassette's reels, an analyser and two VU needles)
/// stay separate instead of repainting everything between them; rects that
/// touch or nearly touch are merged, and many scattered changes fall back
/// to one bounding rect.
pub fn damage_rects(prev: &[ClippedShape], cur: &[ClippedShape], full: Rect) -> Vec<Rect> {
    const MAX_RECTS: usize = 6;
    const NEAR: f32 = 6.0;
    let same = |a: &ClippedShape, b: &ClippedShape| a.clip == b.clip && a.shape.same(&b.shape);
    let n = prev.len().min(cur.len());
    let mut lead = 0;
    while lead < n && same(&prev[lead], &cur[lead]) {
        lead += 1;
    }
    let mut tail = 0;
    while tail < n - lead && same(&prev[prev.len() - 1 - tail], &cur[cur.len() - 1 - tail]) {
        tail += 1;
    }
    let pm = &prev[lead..prev.len() - tail];
    let cm = &cur[lead..cur.len() - tail];
    let area = |c: &ClippedShape| c.shape.bounds().intersect(c.clip).intersect(full);
    let mut parts: Vec<Rect> = Vec::new();
    let mut add = |r: Rect| {
        if r.width() > 0.0 && r.height() > 0.0 {
            parts.push(r);
        }
    };
    if pm.len() == cm.len() {
        for (a, b) in pm.iter().zip(cm.iter()) {
            if !same(a, b) {
                add(area(a));
                add(area(b));
            }
        }
    } else {
        for c in pm.iter().chain(cm.iter()) {
            add(area(c));
        }
    }
    // Merge rects that overlap or nearly touch until none do.
    let mut merged: Vec<Rect> = Vec::new();
    for r in parts {
        let mut r = r;
        loop {
            let hit = merged.iter().position(|m| m.expand(NEAR).intersects(r));
            match hit {
                Some(i) => r = r.union(merged.swap_remove(i)),
                None => break,
            }
        }
        merged.push(r);
    }
    if merged.len() > MAX_RECTS {
        let all = merged.iter().fold(Rect::NOTHING, |a, r| a.union(*r));
        merged = vec![all];
    }
    merged
        .into_iter()
        .map(|d| {
            Rect::from_min_max(
                pos2(d.min.x.floor().max(0.0), d.min.y.floor().max(0.0)),
                pos2(
                    d.max.x.ceil().min(full.max.x),
                    d.max.y.ceil().min(full.max.y),
                ),
            )
        })
        .filter(|d| d.width() > 0.0 && d.height() > 0.0)
        .collect()
}

// ----------------------------------------------------------------- memory

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sense {
    pub click: bool,
    pub drag: bool,
    pub focusable: bool,
}

impl Sense {
    pub fn hover() -> Self {
        Sense::default()
    }
    pub fn click() -> Self {
        Sense {
            click: true,
            drag: false,
            focusable: true,
        }
    }
    pub fn drag() -> Self {
        Sense {
            click: false,
            drag: true,
            focusable: true,
        }
    }
    pub fn click_and_drag() -> Self {
        Sense {
            click: true,
            drag: true,
            focusable: true,
        }
    }
    pub fn interactive(&self) -> bool {
        self.click || self.drag
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct WidgetInfo {
    pub id: Id,
    pub rect: Rect,
    pub layer: usize,
    pub sense: Sense,
    pub text_edit: bool,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct TextEditState {
    pub cursor: usize,
    pub select_all: bool,
    pub scroll_x: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PopupKind {
    Context,
    Combo,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Popup {
    pub id: Id,
    pub kind: PopupKind,
    pub pos: Pos2,
    pub min_width: f32,
    pub opened_frame: u64,
}

#[derive(Default)]
pub struct Memory {
    pub(crate) focus: Option<Id>,
    pub(crate) focus_at_start: Option<Id>,
    pub(crate) text_edit_focused: bool,
    pub(crate) press_id: Option<Id>,
    pub(crate) press_sense: Sense,
    pub(crate) press_pos: Pos2,
    pub(crate) dragging: bool,
    pub(crate) drag_started: bool,
    pub(crate) drag_stopped_id: Option<Id>,
    pub(crate) click_id: Option<Id>,
    pub(crate) double_click: bool,
    pub(crate) last_click: Option<(Id, f64)>,
    pub(crate) secondary_id: Option<Id>,
    pub(crate) secondary_click: Option<Id>,
    pub(crate) hover_target: Option<Id>,
    pub(crate) top_layer: usize,
    pub(crate) prev_widgets: Vec<WidgetInfo>,
    pub(crate) cur_widgets: Vec<WidgetInfo>,
    pub(crate) prev_popups: Vec<Rect>,
    pub(crate) cur_popups: Vec<Rect>,
    pub(crate) scroll: HashMap<Id, f32>,
    pub(crate) scroll_request: HashMap<Id, (f32, f32)>,
    pub(crate) popup: Option<Popup>,
    pub(crate) submenu: Option<(Id, Rect)>,
    pub(crate) submenu_rect: Option<Rect>,
    pub(crate) popup_seen: bool,
    pub(crate) area_sizes: HashMap<Id, Vec2>,
    pub(crate) text: HashMap<Id, TextEditState>,
    pub(crate) scrollbar_drag: Option<(Id, f32, f32)>,
    pub(crate) tab: Option<bool>,
}

impl Memory {
    pub fn focused(&self) -> Option<Id> {
        self.focus
    }
    pub fn surrender_focus(&mut self, _id: Id) {
        self.focus = None;
    }
    pub fn request_focus(&mut self, id: Id) {
        self.focus = Some(id);
    }
    pub fn has_focus(&self, id: Id) -> bool {
        self.focus == Some(id)
    }
    pub fn close_popup(&mut self) {
        self.popup = None;
        self.submenu = None;
        self.submenu_rect = None;
    }
}

#[derive(Clone, Debug)]
pub struct Style {
    pub body_size: f32,
    pub item_spacing: Vec2,
    pub button_padding: Vec2,
    pub interact_height: f32,
    pub visuals: Visuals,
}

#[derive(Clone, Copy, Debug)]
pub struct Visuals {
    pub text: Color32,
    pub strong_text: Color32,
    pub weak_text: Color32,
    pub panel_fill: Color32,
    pub window_fill: Color32,
    pub window_stroke: Color32,
    pub extreme_bg: Color32,
    pub selection_bg: Color32,
    pub inactive_bg: Color32,
    pub inactive_weak_bg: Color32,
    pub inactive_stroke: Color32,
    pub hovered_bg: Color32,
    pub hovered_weak_bg: Color32,
    pub hovered_stroke: Color32,
    pub active_bg: Color32,
    pub disabled_text: Color32,
}

impl Default for Visuals {
    fn default() -> Self {
        let g = |v: u8| Color32::from_rgb(v, v, v);
        Visuals {
            text: g(200),
            strong_text: g(255),
            weak_text: g(140),
            panel_fill: g(27),
            window_fill: g(27),
            window_stroke: g(60),
            extreme_bg: g(10),
            selection_bg: Color32::from_rgb(0, 92, 128),
            inactive_bg: g(60),
            inactive_weak_bg: g(45),
            inactive_stroke: g(60),
            hovered_bg: g(70),
            hovered_weak_bg: g(70),
            hovered_stroke: g(150),
            active_bg: g(55),
            disabled_text: g(100),
        }
    }
}

impl Default for Style {
    fn default() -> Self {
        Style {
            body_size: 14.0,
            item_spacing: vec2(8.0, 3.0),
            button_padding: vec2(4.0, 1.0),
            interact_height: 18.0,
            visuals: Visuals::default(),
        }
    }
}

// ----------------------------------------------------------------- context

pub(crate) struct Inner {
    pub input: RefCell<InputState>,
    pub mem: RefCell<Memory>,
    pub layers: RefCell<[Vec<ClippedShape>; 3]>,
    pub commands: RefCell<Vec<ViewportCommand>>,
    pub cursor: Cell<CursorIcon>,
    pub repaint: Cell<bool>,
    pub repaint_at: Cell<Option<Instant>>,
    pub tooltip: RefCell<Option<(String, Pos2)>>,
    pub style: RefCell<Rc<Style>>,
    pub fonts: RefCell<text::Fonts>,
    pub frame: Cell<u64>,
    pub start: Instant,
    pub last_motion: Cell<Instant>,
}

/// Shared frame state; cheap to clone.
#[derive(Clone)]
pub struct Context(pub(crate) Rc<Inner>);

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

impl Context {
    pub fn new() -> Self {
        Context(Rc::new(Inner {
            input: RefCell::new(InputState::default()),
            mem: RefCell::new(Memory::default()),
            layers: RefCell::new([Vec::new(), Vec::new(), Vec::new()]),
            commands: RefCell::new(Vec::new()),
            cursor: Cell::new(CursorIcon::Default),
            repaint: Cell::new(false),
            repaint_at: Cell::new(None),
            tooltip: RefCell::new(None),
            style: RefCell::new(Rc::new(Style::default())),
            fonts: RefCell::new(text::Fonts::new()),
            frame: Cell::new(0),
            start: Instant::now(),
            last_motion: Cell::new(Instant::now()),
        }))
    }

    pub fn input<R>(&self, f: impl FnOnce(&InputState) -> R) -> R {
        f(&self.0.input.borrow())
    }
    pub fn input_mut<R>(&self, f: impl FnOnce(&mut InputState) -> R) -> R {
        f(&mut self.0.input.borrow_mut())
    }
    pub fn memory<R>(&self, f: impl FnOnce(&Memory) -> R) -> R {
        f(&self.0.mem.borrow())
    }
    pub fn memory_mut<R>(&self, f: impl FnOnce(&mut Memory) -> R) -> R {
        f(&mut self.0.mem.borrow_mut())
    }
    pub fn send_viewport_cmd(&self, cmd: ViewportCommand) {
        self.0.commands.borrow_mut().push(cmd);
        self.request_repaint();
    }
    pub fn take_commands(&self) -> Vec<ViewportCommand> {
        std::mem::take(&mut self.0.commands.borrow_mut())
    }
    pub fn request_repaint(&self) {
        self.0.repaint.set(true);
    }
    pub fn request_repaint_after(&self, d: Duration) {
        let at = Instant::now() + d;
        let cur = self.0.repaint_at.get();
        if cur.is_none_or(|c| at < c) {
            self.0.repaint_at.set(Some(at));
        }
    }
    pub fn set_cursor_icon(&self, icon: CursorIcon) {
        self.0.cursor.set(icon);
    }
    pub fn cursor_icon(&self) -> CursorIcon {
        self.0.cursor.get()
    }
    pub fn screen_rect(&self) -> Rect {
        self.0.input.borrow().screen_rect
    }
    pub fn style(&self) -> Rc<Style> {
        self.0.style.borrow().clone()
    }
    pub fn set_style(&self, style: Style) {
        *self.0.style.borrow_mut() = Rc::new(style);
    }
    pub fn fonts_mut<R>(&self, f: impl FnOnce(&mut text::Fonts) -> R) -> R {
        f(&mut self.0.fonts.borrow_mut())
    }
    pub fn layer_painter(&self, layer: LayerId) -> Painter {
        Painter {
            ctx: self.clone(),
            layer: layer.index(),
            clip: Rect::EVERYTHING,
        }
    }
    pub fn frame_nr(&self) -> u64 {
        self.0.frame.get()
    }
    pub fn load_texture(
        &self,
        _name: impl Into<String>,
        image: ColorImage,
        _options: TextureOptions,
    ) -> TextureHandle {
        let (w, h) = (image.size[0].max(1) as u32, image.size[1].max(1) as u32);
        let pixmap =
            tiny_skia::Pixmap::from_vec(image.rgba, tiny_skia::IntSize::from_wh(w, h).unwrap())
                .unwrap_or_else(|| tiny_skia::Pixmap::new(1, 1).unwrap());
        TextureHandle(Rc::new(Texture { pixmap }))
    }

    /// Start a frame with this input. Resolves which widget the pointer is
    /// on (from the previous frame's widgets) and the press/drag/click state.
    pub fn begin_frame(&self, input: InputState) {
        self.0.frame.set(self.0.frame.get() + 1);
        self.0.repaint.set(false);
        self.0.repaint_at.set(None);
        self.0.cursor.set(CursorIcon::Default);
        *self.0.tooltip.borrow_mut() = None;
        for l in self.0.layers.borrow_mut().iter_mut() {
            l.clear();
        }
        if input.pointer.delta != Vec2::ZERO {
            self.0.last_motion.set(Instant::now());
        }
        let mut m = self.0.mem.borrow_mut();
        m.prev_widgets = std::mem::take(&mut m.cur_widgets);
        m.prev_popups = std::mem::take(&mut m.cur_popups);
        m.focus_at_start = m.focus;
        m.drag_started = false;
        m.drag_stopped_id = None;
        m.click_id = None;
        m.double_click = false;
        m.secondary_click = None;
        m.popup_seen = false;
        let p = &input.pointer;
        m.top_layer = match p.pos {
            Some(pos) if m.prev_popups.iter().any(|r| r.contains(pos)) => 1,
            _ => 0,
        };
        m.hover_target = p.pos.and_then(|pos| {
            m.prev_widgets
                .iter()
                .filter(|w| w.layer == m.top_layer && w.sense.interactive() && w.rect.contains(pos))
                .fold(None::<WidgetInfo>, |best, w| match best {
                    Some(b) if b.rect.area() < w.rect.area() => Some(b),
                    _ => Some(*w),
                })
                .map(|w| w.id)
        });
        if p.primary_pressed {
            // A press outside every popup closes it.
            if m.top_layer == 0 && m.popup.is_some() {
                m.popup = None;
                m.submenu = None;
            }
            m.press_id = m.hover_target;
            m.press_pos = p.pos.unwrap_or_default();
            m.press_sense = m
                .press_id
                .and_then(|id| m.prev_widgets.iter().find(|w| w.id == id))
                .map(|w| w.sense)
                .unwrap_or_default();
            m.dragging = false;
            if m.press_sense.drag && !m.press_sense.click {
                m.dragging = true;
                m.drag_started = true;
            }
            // Focus follows a press on a text field; any other press leaves
            // text fields.
            let target_is_text = m
                .press_id
                .and_then(|id| m.prev_widgets.iter().find(|w| w.id == id))
                .is_some_and(|w| w.text_edit);
            if target_is_text {
                m.focus = m.press_id;
            } else if m.focus != m.press_id {
                m.focus = None;
            }
        }
        if p.secondary_pressed {
            if m.top_layer == 0 && m.popup.is_some() {
                m.popup = None;
                m.submenu = None;
            }
            m.secondary_id = m.hover_target;
        }
        if p.primary_down && m.press_id.is_some() && !m.dragging && m.press_sense.drag {
            if let Some(pos) = p.pos {
                if (pos - m.press_pos).length() >= 4.0 {
                    m.dragging = true;
                    m.drag_started = true;
                }
            }
        }
        if p.primary_released {
            if let Some(id) = m.press_id {
                let on_target = m.hover_target == Some(id)
                    || p.pos.is_some_and(|pos| {
                        m.prev_widgets
                            .iter()
                            .any(|w| w.id == id && w.rect.contains(pos))
                    });
                if m.dragging {
                    m.drag_stopped_id = Some(id);
                }
                let moved = p.pos.map(|pos| (pos - m.press_pos).length()).unwrap_or(0.0);
                if m.press_sense.click && on_target && (!m.dragging || moved < 4.0) {
                    m.click_id = Some(id);
                    let now = input.time;
                    m.double_click = m
                        .last_click
                        .is_some_and(|(lid, t)| lid == id && now - t < 0.35);
                    m.last_click = if m.double_click {
                        None
                    } else {
                        Some((id, now))
                    };
                }
            }
            m.press_id = None;
            m.dragging = false;
        }
        if p.secondary_released {
            if m.secondary_id.is_some() && m.secondary_id == m.hover_target {
                m.secondary_click = m.secondary_id;
            }
            m.secondary_id = None;
        }
        m.tab = None;
        for e in &input.events {
            if let Event::Key {
                key: Key::Tab,
                pressed: true,
                modifiers,
            } = e
            {
                m.tab = Some(modifiers.shift);
            }
        }
        if let Some(back) = m.tab {
            let order: Vec<Id> = m
                .prev_widgets
                .iter()
                .filter(|w| {
                    w.sense.focusable && w.layer == if m.prev_popups.is_empty() { 0 } else { 1 }
                })
                .map(|w| w.id)
                .collect();
            if !order.is_empty() {
                let pos = m.focus.and_then(|f| order.iter().position(|i| *i == f));
                let next = match (pos, back) {
                    (None, false) => 0,
                    (None, true) => order.len() - 1,
                    (Some(i), false) => (i + 1) % order.len(),
                    (Some(i), true) => (i + order.len() - 1) % order.len(),
                };
                m.focus = Some(order[next]);
            }
        }
        drop(m);
        *self.0.input.borrow_mut() = input;
    }

    /// End the frame: tooltip, text-field focus bookkeeping. Returns the
    /// shapes of all layers in paint order.
    pub fn end_frame(&self) -> Vec<ClippedShape> {
        if let Some((text, pos)) = self.0.tooltip.borrow_mut().take() {
            let style = self.style();
            let p = self.layer_painter(LayerId::new(Order::Tooltip, Id::new("tooltip")));
            let size = style.body_size * 0.85;
            let screen = self.screen_rect();
            let max_w = (screen.width() - 16.0).clamp(60.0, 320.0);
            let galley = self.fonts_mut(|f| f.layout_wrap(&text, false, size, max_w));
            let mut r = Rect::from_min_size(pos + vec2(12.0, 16.0), galley.size + vec2(12.0, 8.0));
            if r.right() > screen.right() - 2.0 {
                r = r.translate(vec2(screen.right() - 2.0 - r.right(), 0.0));
            }
            if r.bottom() > screen.bottom() - 2.0 {
                r = r.translate(vec2(0.0, pos.y - 4.0 - r.bottom()));
            }
            p.rect_filled(r, 2.0, style.visuals.window_fill);
            p.rect_stroke(r, 2.0, Stroke::new(1.0, style.visuals.window_stroke));
            p.galley(r.min + vec2(6.0, 4.0), Rc::new(galley), style.visuals.text);
        }
        let mut m = self.0.mem.borrow_mut();
        let focus = m.focus;
        m.text_edit_focused =
            focus.is_some_and(|f| m.cur_widgets.iter().any(|w| w.id == f && w.text_edit));
        // A focused widget that vanished loses focus.
        if let Some(f) = focus {
            if !m.cur_widgets.iter().any(|w| w.id == f) {
                m.focus = None;
            }
        }
        if m.popup
            .is_some_and(|p| p.opened_frame == self.0.frame.get())
        {
            self.0.repaint.set(true);
        }
        // A popup whose owner was not drawn (scrolled away, view changed)
        // closes.
        if m.popup.is_some() && !m.popup_seen {
            m.popup = None;
            m.submenu = None;
            m.submenu_rect = None;
            self.0.repaint.set(true);
        }
        drop(m);
        let mut layers = self.0.layers.borrow_mut();
        let mut out = Vec::new();
        for l in layers.iter_mut() {
            out.append(l);
        }
        out
    }

    /// Whether a text field had keyboard focus at the end of the last frame.
    pub fn text_edit_focused(&self) -> bool {
        self.0.mem.borrow().text_edit_focused
    }

    pub fn wants_repaint(&self) -> (bool, Option<Instant>) {
        (self.0.repaint.get(), self.0.repaint_at.get())
    }

    pub(crate) fn add_shape(&self, layer: usize, clip: Rect, shape: Shape) -> usize {
        let mut layers = self.0.layers.borrow_mut();
        layers[layer].push(ClippedShape { clip, shape });
        layers[layer].len() - 1
    }

    pub(crate) fn set_shape(&self, layer: usize, idx: usize, shape: Shape) {
        if let Some(s) = self.0.layers.borrow_mut()[layer].get_mut(idx) {
            s.shape = shape;
        }
    }

    pub(crate) fn register(&self, info: WidgetInfo) {
        self.0.mem.borrow_mut().cur_widgets.push(info);
    }

    pub(crate) fn register_popup(&self, rect: Rect) {
        self.0.mem.borrow_mut().cur_popups.push(rect);
    }

    pub(crate) fn show_tooltip(&self, text: &str) {
        let rested = self.0.last_motion.get().elapsed();
        let delay = Duration::from_millis(500);
        if rested >= delay {
            let pos = self.input(|i| i.pointer.pos).unwrap_or_default();
            *self.0.tooltip.borrow_mut() = Some((text.to_string(), pos));
        } else {
            self.request_repaint_after(delay - rested + Duration::from_millis(5));
        }
    }

    /// Interaction for `rect` (already clipped) on `layer`.
    pub(crate) fn interact(
        &self,
        rect: Rect,
        id: Id,
        sense: Sense,
        layer: usize,
        enabled: bool,
        text_edit: bool,
    ) -> Response {
        if sense.interactive() || sense.focusable {
            self.register(WidgetInfo {
                id,
                rect,
                layer,
                sense: if enabled { sense } else { Sense::hover() },
                text_edit,
            });
        }
        let input = self.0.input.borrow();
        let m = self.0.mem.borrow();
        let pos = input.pointer.pos;
        let inside = pos.is_some_and(|p| rect.contains(p));
        let on_top = layer >= m.top_layer;
        // While a button is held on another widget nothing else lights up,
        // except during a real drag, where the drop target is hovered.
        let pointer_busy = m.press_id.is_some()
            && m.press_id != Some(id)
            && input.pointer.primary_down
            && !m.dragging;
        let hovered = enabled
            && inside
            && on_top
            && !pointer_busy
            && (!sense.interactive() || m.hover_target.is_none() || m.hover_target == Some(id));
        let active = enabled && sense.interactive();
        let pressed_here = active && m.press_id == Some(id);
        Response {
            ctx: self.clone(),
            id,
            rect,
            sense,
            enabled,
            hovered,
            clicked: active && m.click_id == Some(id),
            double_clicked: active && m.click_id == Some(id) && m.double_click,
            secondary_clicked: active && m.secondary_click == Some(id),
            drag_started: pressed_here && sense.drag && m.drag_started,
            dragged: pressed_here && sense.drag && m.dragging && input.pointer.primary_down,
            drag_stopped: active && sense.drag && m.drag_stopped_id == Some(id),
            pointer_down: pressed_here && input.pointer.primary_down,
            changed: false,
            lost_focus: m.focus_at_start == Some(id) && m.focus != Some(id),
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TextureOptions;

impl TextureOptions {
    pub const LINEAR: TextureOptions = TextureOptions;
}

// ----------------------------------------------------------------- painter

#[derive(Clone)]
pub struct Painter {
    pub(crate) ctx: Context,
    pub(crate) layer: usize,
    pub(crate) clip: Rect,
}

impl Painter {
    pub fn with_clip_rect(&self, rect: Rect) -> Painter {
        Painter {
            ctx: self.ctx.clone(),
            layer: self.layer,
            clip: self.clip.intersect(rect),
        }
    }
    pub fn clip_rect(&self) -> Rect {
        self.clip
    }
    pub fn ctx(&self) -> &Context {
        &self.ctx
    }
    pub fn add(&self, shape: Shape) -> usize {
        self.ctx.add_shape(self.layer, self.clip, shape)
    }
    pub(crate) fn set(&self, idx: usize, shape: Shape) {
        self.ctx.set_shape(self.layer, idx, shape);
    }
    pub fn rect_filled(&self, rect: Rect, rounding: impl Into<Rounding>, fill: Color32) {
        self.add(Shape::Rect {
            rect,
            rounding: rounding.into().0,
            fill,
            stroke: Stroke::NONE,
        });
    }
    pub fn rect_stroke(&self, rect: Rect, rounding: impl Into<Rounding>, stroke: Stroke) {
        self.add(Shape::Rect {
            rect,
            rounding: rounding.into().0,
            fill: Color32::TRANSPARENT,
            stroke,
        });
    }
    pub fn rect(&self, rect: Rect, rounding: impl Into<Rounding>, fill: Color32, stroke: Stroke) {
        self.add(Shape::Rect {
            rect,
            rounding: rounding.into().0,
            fill,
            stroke,
        });
    }
    pub fn line_segment(&self, pts: [Pos2; 2], stroke: Stroke) {
        self.add(Shape::Line {
            points: pts.to_vec(),
            stroke,
        });
    }
    pub fn circle_filled(&self, center: Pos2, radius: f32, fill: Color32) {
        self.add(Shape::Circle {
            center,
            radius,
            fill,
            stroke: Stroke::NONE,
        });
    }
    pub fn circle_stroke(&self, center: Pos2, radius: f32, stroke: Stroke) {
        self.add(Shape::Circle {
            center,
            radius,
            fill: Color32::TRANSPARENT,
            stroke,
        });
    }
    pub fn layout_no_wrap(&self, text: String, font: FontId, _color: Color32) -> Rc<text::Galley> {
        Rc::new(
            self.ctx
                .fonts_mut(|f| f.layout_no_wrap(&text, font.mono, font.size)),
        )
    }
    pub fn layout(
        &self,
        text: String,
        font: FontId,
        _color: Color32,
        wrap: f32,
    ) -> Rc<text::Galley> {
        Rc::new(
            self.ctx
                .fonts_mut(|f| f.layout_wrap(&text, font.mono, font.size, wrap)),
        )
    }
    pub fn galley(&self, pos: Pos2, galley: Rc<text::Galley>, color: Color32) {
        self.add(Shape::Text { pos, galley, color });
    }
    /// Draw text anchored at `pos`; returns its rect.
    pub fn text(
        &self,
        pos: Pos2,
        anchor: Align2,
        text: impl ToString,
        font: FontId,
        color: Color32,
    ) -> Rect {
        let galley = self.layout_no_wrap(text.to_string(), font, color);
        let rect = anchor.anchor_size(pos, galley.size);
        self.galley(rect.min, galley, color);
        rect
    }
    pub fn image(&self, texture: TextureId, rect: Rect, _uv: Rect, _tint: Color32) {
        self.add(Shape::Image { texture, rect });
    }
}

// ---------------------------------------------------------------- response

#[derive(Clone)]
pub struct Response {
    pub ctx: Context,
    pub id: Id,
    pub rect: Rect,
    pub sense: Sense,
    pub enabled: bool,
    hovered: bool,
    clicked: bool,
    double_clicked: bool,
    secondary_clicked: bool,
    drag_started: bool,
    dragged: bool,
    drag_stopped: bool,
    pointer_down: bool,
    pub(crate) changed: bool,
    lost_focus: bool,
}

impl Response {
    pub fn hovered(&self) -> bool {
        self.hovered
    }
    pub fn clicked(&self) -> bool {
        self.clicked
    }
    pub fn double_clicked(&self) -> bool {
        self.double_clicked
    }
    pub fn secondary_clicked(&self) -> bool {
        self.secondary_clicked
    }
    pub fn drag_started(&self) -> bool {
        self.drag_started
    }
    pub fn dragged(&self) -> bool {
        self.dragged
    }
    pub fn drag_stopped(&self) -> bool {
        self.drag_stopped
    }
    pub fn is_pointer_button_down_on(&self) -> bool {
        self.pointer_down
    }
    pub fn changed(&self) -> bool {
        self.changed
    }
    pub fn mark_changed(&mut self) {
        self.changed = true;
    }
    pub fn has_focus(&self) -> bool {
        self.ctx.memory(|m| m.focus == Some(self.id))
    }
    pub fn lost_focus(&self) -> bool {
        self.lost_focus
            || (self
                .ctx
                .memory(|m| m.focus_at_start == Some(self.id) && m.focus != Some(self.id)))
    }
    pub fn request_focus(&self) {
        self.ctx.memory_mut(|m| m.focus = Some(self.id));
        self.ctx.request_repaint();
    }
    pub fn surrender_focus(&self) {
        self.ctx.memory_mut(|m| {
            if m.focus == Some(self.id) {
                m.focus = None
            }
        });
    }
    pub fn on_hover_text(self, text: impl ToString) -> Self {
        if self.hovered && self.enabled && !self.pointer_down {
            self.ctx.show_tooltip(&text.to_string());
        }
        self
    }
    pub fn on_disabled_hover_text(self, text: impl ToString) -> Self {
        if !self.enabled {
            let inside = self
                .ctx
                .input(|i| i.pointer.pos)
                .is_some_and(|p| self.rect.contains(p));
            if inside {
                self.ctx.show_tooltip(&text.to_string());
            }
        }
        self
    }
    pub fn union(&self, other: Response) -> Response {
        let mut r = self.clone();
        r.rect = r.rect.union(other.rect);
        r.hovered |= other.hovered;
        r.clicked |= other.clicked;
        r.changed |= other.changed;
        r
    }
}

#[cfg(test)]
mod damage_tests {
    use super::*;

    fn rect(x: f32, y: f32, w: f32) -> ClippedShape {
        ClippedShape {
            clip: Rect::EVERYTHING,
            shape: Shape::Rect {
                rect: Rect::from_min_size(pos2(x, y), vec2(w, 10.0)),
                rounding: 0.0,
                fill: Color32::WHITE,
                stroke: Stroke::NONE,
            },
        }
    }

    #[test]
    fn far_apart_changes_stay_separate_and_near_ones_merge() {
        let full = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
        let frame = |xs: &[f32]| xs.iter().map(|x| rect(*x, 10.0, 20.0)).collect::<Vec<_>>();
        let prev = frame(&[10.0, 300.0, 600.0]);
        assert!(damage_rects(&prev, &frame(&[10.0, 300.0, 600.0]), full).is_empty());
        let d = damage_rects(&prev, &frame(&[12.0, 300.0, 602.0]), full);
        assert_eq!(d.len(), 2, "{d:?}");
        assert!(d.iter().all(|r| r.width() < 40.0));
        // Neighbours merge into one rect.
        let prev2 = frame(&[290.0, 300.0, 600.0]);
        let near = frame(&[291.0, 301.0, 600.0]);
        assert_eq!(damage_rects(&prev2, &near, full).len(), 1);
    }
}
