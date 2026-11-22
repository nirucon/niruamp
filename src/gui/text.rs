//! Text for the canvas: the egui default faces (Ubuntu Light, Hack) embedded,
//! an optional NIRUX font, and a lazily found fallback for missing glyphs.
//! Coverage masks are cached per (font, glyph, size), so a frame of text costs
//! blits, not rasterisation.
use ab_glyph::{point, Font, FontArc, GlyphId, PxScale, ScaleFont};
use std::collections::HashMap;

static UBUNTU_LIGHT: &[u8] = include_bytes!("../../assets/fonts/Ubuntu-Light.ttf");
static HACK: &[u8] = include_bytes!("../../assets/fonts/Hack-Regular.ttf");

pub struct Mask {
    pub w: i32,
    pub h: i32,
    pub x: i32,
    pub y: i32,
    pub coverage: Vec<u8>,
}

/// One laid-out line of text.
#[derive(Clone, Debug)]
pub struct Line {
    pub text: String,
    pub width: f32,
}

/// Laid-out text, like egui's `Galley`: lines, total size, font.
#[derive(Clone, Debug)]
pub struct Galley {
    pub lines: Vec<Line>,
    pub size: crate::gui::Vec2,
    pub mono: bool,
    pub font_size: f32,
    pub row_height: f32,
}

impl Galley {
    pub fn size(&self) -> crate::gui::Vec2 {
        self.size
    }
}

pub struct Fonts {
    prop: FontArc,
    mono: FontArc,
    fallback: Option<Option<FontArc>>,
    masks: HashMap<(u8, u16, u32), Option<Mask>>,
    advances: HashMap<(bool, char, u32), (u8, f32)>,
}

impl Default for Fonts {
    fn default() -> Self {
        Self::new()
    }
}

impl Fonts {
    pub fn new() -> Self {
        Self {
            prop: FontArc::try_from_slice(UBUNTU_LIGHT).expect("embedded font"),
            mono: FontArc::try_from_slice(HACK).expect("embedded font"),
            fallback: None,
            masks: HashMap::new(),
            advances: HashMap::new(),
        }
    }

    /// Use the NIRUX font (appearance `font_path`) for both families, as
    /// egui put it first in both.
    pub fn set_custom(&mut self, path: Option<&std::path::Path>) {
        let custom = path
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| FontArc::try_from_vec(b).ok());
        match custom {
            Some(f) => {
                self.prop = f.clone();
                self.mono = f;
            }
            None => {
                self.prop = FontArc::try_from_slice(UBUNTU_LIGHT).expect("embedded font");
                self.mono = FontArc::try_from_slice(HACK).expect("embedded font");
            }
        }
        self.masks.clear();
        self.advances.clear();
    }

    fn face(&self, which: u8) -> Option<&FontArc> {
        match which {
            0 => Some(&self.prop),
            1 => Some(&self.mono),
            _ => self.fallback.as_ref().and_then(|f| f.as_ref()),
        }
    }

    /// Which face draws `c`: the family's own, Ubuntu for mono, or the
    /// system fallback.
    fn pick(&mut self, mono: bool, c: char) -> u8 {
        let own = if mono { 1 } else { 0 };
        if c.is_ascii() || self.face(own).is_some_and(|f| f.glyph_id(c).0 != 0) {
            return own;
        }
        if mono && self.prop.glyph_id(c).0 != 0 {
            return 0;
        }
        let fb = self.fallback.get_or_insert_with(load_fallback);
        match fb {
            Some(f) if f.glyph_id(c).0 != 0 => 2,
            _ => own,
        }
    }

    pub fn row_height(&self, mono: bool, size: f32) -> f32 {
        let f = if mono { &self.mono } else { &self.prop };
        let s = f.as_scaled(PxScale::from(size.max(1.0)));
        s.ascent() - s.descent() + s.line_gap()
    }

    pub fn ascent(&self, mono: bool, size: f32) -> f32 {
        let f = if mono { &self.mono } else { &self.prop };
        f.as_scaled(PxScale::from(size.max(1.0))).ascent()
    }

    fn glyph(&mut self, mono: bool, c: char, size: f32) -> (u8, f32) {
        let key = (mono, c, size.to_bits());
        if let Some(v) = self.advances.get(&key) {
            return *v;
        }
        let which = self.pick(mono, c);
        let f = self.face(which).expect("face");
        let a = f.as_scaled(PxScale::from(size)).h_advance(f.glyph_id(c));
        self.advances.insert(key, (which, a));
        (which, a)
    }

    pub fn width(&mut self, s: &str, mono: bool, size: f32) -> f32 {
        s.chars().map(|c| self.glyph(mono, c, size).1).sum()
    }

    /// One line without wrapping.
    pub fn layout_no_wrap(&mut self, text: &str, mono: bool, size: f32) -> Galley {
        let rh = self.row_height(mono, size);
        let lines: Vec<Line> = text
            .split('\n')
            .map(|l| Line {
                text: l.to_string(),
                width: self.width(l, mono, size),
            })
            .collect();
        let w = lines.iter().map(|l| l.width).fold(0.0, f32::max);
        Galley {
            size: crate::gui::vec2(w, rh * lines.len() as f32),
            lines,
            mono,
            font_size: size,
            row_height: rh,
        }
    }

    /// Wrap at word boundaries (and inside long words) to `max` pixels.
    pub fn layout_wrap(&mut self, text: &str, mono: bool, size: f32, max: f32) -> Galley {
        let rh = self.row_height(mono, size);
        let mut lines = Vec::new();
        for para in text.split('\n') {
            let mut line = String::new();
            let mut lw = 0.0f32;
            for word in split_keep_spaces(para) {
                let ww = self.width(&word, mono, size);
                if lw + ww <= max || line.is_empty() && ww <= max {
                    line.push_str(&word);
                    lw += ww;
                    continue;
                }
                if !line.is_empty() {
                    let t = line.trim_end().to_string();
                    let w = self.width(&t, mono, size);
                    lines.push(Line { text: t, width: w });
                }
                line = String::new();
                lw = 0.0;
                let word = word.trim_start().to_string();
                // A word longer than the line is broken by characters.
                for c in word.chars() {
                    let a = self.glyph(mono, c, size).1;
                    if lw + a > max && !line.is_empty() {
                        lines.push(Line {
                            text: std::mem::take(&mut line),
                            width: lw,
                        });
                        lw = 0.0;
                    }
                    line.push(c);
                    lw += a;
                }
            }
            let t = line.trim_end().to_string();
            let w = self.width(&t, mono, size);
            lines.push(Line { text: t, width: w });
        }
        let w = lines.iter().map(|l| l.width).fold(0.0, f32::max);
        Galley {
            size: crate::gui::vec2(w, rh * lines.len() as f32),
            lines,
            mono,
            font_size: size,
            row_height: rh,
        }
    }

    /// Visit every glyph mask of one line: (x, y, mask) relative to the
    /// line's top-left corner.
    pub fn for_each_glyph(
        &mut self,
        text: &str,
        mono: bool,
        size: f32,
        mut f: impl FnMut(i32, i32, &Mask),
    ) {
        let ascent = self.ascent(mono, size).round() as i32;
        let mut pen = 0.0f32;
        for c in text.chars() {
            let (which, adv) = self.glyph(mono, c, size);
            if !c.is_whitespace() {
                let face = self.face(which).expect("face").clone();
                let gid = face.glyph_id(c);
                let key = (which, gid.0, size.to_bits());
                let m = self
                    .masks
                    .entry(key)
                    .or_insert_with(|| rasterise(&face, gid, size));
                if let Some(m) = m {
                    f(pen.round() as i32 + m.x, ascent + m.y, m);
                }
            }
            pen += adv;
        }
        // Bound the cache: a long session with many sizes stays small.
        if self.masks.len() > 4000 {
            self.masks.clear();
        }
    }
}

fn split_keep_spaces(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        if c == ' ' && !cur.is_empty() && !cur.ends_with(' ') {
            out.push(std::mem::take(&mut cur));
        }
        cur.push(c);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn rasterise(f: &FontArc, gid: GlyphId, px: f32) -> Option<Mask> {
    let glyph = gid.with_scale_and_position(PxScale::from(px), point(0.0, 0.0));
    let outline = f.outline_glyph(glyph)?;
    let b = outline.px_bounds();
    let (w, h) = (b.width().ceil() as i32, b.height().ceil() as i32);
    if w <= 0 || h <= 0 {
        return None;
    }
    let mut coverage = vec![0u8; (w * h) as usize];
    outline.draw(|gx, gy, v| {
        let i = (gy as i32 * w + gx as i32) as usize;
        if i < coverage.len() {
            coverage[i] = (v * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    });
    Some(Mask {
        w,
        h,
        x: b.min.x.floor() as i32,
        y: b.min.y.floor() as i32,
        coverage,
    })
}

/// A system font for symbols the embedded faces lack (arrows, check marks).
fn load_fallback() -> Option<FontArc> {
    let mut paths = Vec::new();
    if let Ok(out) = std::process::Command::new("fc-match")
        .args(["-f", "%{file}", "DejaVu Sans,Noto Sans Symbols2,Noto Sans"])
        .output()
    {
        let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if out.status.success() && !p.is_empty() {
            paths.push(p);
        }
    }
    for p in [
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "/usr/share/fonts/dejavu/DejaVuSans.ttf",
    ] {
        paths.push(p.to_string());
    }
    paths.into_iter().find_map(|p| {
        std::fs::read(p)
            .ok()
            .and_then(|b| FontArc::try_from_vec(b).ok())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_words_within_the_width() {
        let mut f = Fonts::new();
        let g = f.layout_wrap("alpha beta gamma delta epsilon", false, 12.0, 60.0);
        assert!(g.lines.len() > 1);
        assert!(g
            .lines
            .iter()
            .all(|l| l.width <= 60.5 || !l.text.contains(' ')));
        let one = f.layout_no_wrap("alpha beta", false, 12.0);
        assert_eq!(one.lines.len(), 1);
        assert!(one.size.x > 30.0);
    }

    #[test]
    fn row_height_follows_the_size() {
        let f = Fonts::new();
        assert!(f.row_height(false, 20.0) > f.row_height(false, 10.0));
        assert!(f.row_height(true, 12.0) > 10.0);
    }
}
