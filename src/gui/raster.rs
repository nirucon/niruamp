//! Paint a frame's shapes into a tiny-skia pixmap.
//!
//! Clip rectangles are axis aligned: rectangles and text are clipped
//! exactly by arithmetic; paths only get a mask when they straddle their
//! clip (rows at the edge of a scroll area), and that mask is reused for
//! every shape with the same clip.
use super::text::Fonts;
use super::*;
use tiny_skia::{FillRule, Mask, Paint, PathBuilder, Pixmap, PixmapPaint, Shader, Transform};

fn paint(c: Color32) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(c.r(), c.g(), c.b(), c.a());
    p.anti_alias = true;
    p
}

pub struct Raster {
    masks: Vec<(Rect, Mask)>,
}

impl Default for Raster {
    fn default() -> Self {
        Self::new()
    }
}

impl Raster {
    pub fn new() -> Self {
        Raster { masks: Vec::new() }
    }

    pub fn paint(&mut self, pm: &mut Pixmap, fonts: &mut Fonts, shapes: &[ClippedShape]) {
        let full = Rect::from_min_size(pos2(0.0, 0.0), vec2(pm.width() as f32, pm.height() as f32));
        self.paint_region(pm, fonts, shapes, full);
    }

    /// Paint only inside `region` (whole pixels); the rest keeps the last frame.
    pub fn paint_region(
        &mut self,
        pm: &mut Pixmap,
        fonts: &mut Fonts,
        shapes: &[ClippedShape],
        region: Rect,
    ) {
        self.masks.clear();
        let full = Rect::from_min_size(pos2(0.0, 0.0), vec2(pm.width() as f32, pm.height() as f32))
            .intersect(region);
        for s in shapes {
            let clip = s.clip.intersect(full);
            if !clip.is_positive() || !s.shape.bounds().intersects(clip) {
                continue;
            }
            match &s.shape {
                Shape::Noop => {}
                Shape::Rect {
                    rect,
                    rounding,
                    fill,
                    stroke,
                } => {
                    if !fill.is_transparent() {
                        self.fill_rect(pm, *rect, *rounding, *fill, clip);
                    }
                    if !stroke.is_empty() {
                        self.stroke_rect(pm, *rect, *rounding, *stroke, clip);
                    }
                }
                Shape::Line { points, stroke } => {
                    if stroke.is_empty() || points.len() < 2 {
                        continue;
                    }
                    // Axis-aligned hairlines are drawn crisp on pixel centres.
                    if points.len() == 2 && stroke.width <= 1.01 {
                        let (a, b) = (points[0], points[1]);
                        if (a.y - b.y).abs() < 0.01 {
                            let y = a.y.floor();
                            let r = Rect::from_min_max(
                                pos2(a.x.min(b.x), y),
                                pos2(a.x.max(b.x), y + 1.0),
                            );
                            self.fill_rect(pm, r, 0.0, stroke.color, clip);
                            continue;
                        }
                        if (a.x - b.x).abs() < 0.01 {
                            let x = a.x.floor();
                            let r = Rect::from_min_max(
                                pos2(x, a.y.min(b.y)),
                                pos2(x + 1.0, a.y.max(b.y)),
                            );
                            self.fill_rect(pm, r, 0.0, stroke.color, clip);
                            continue;
                        }
                    }
                    let mut pb = PathBuilder::new();
                    pb.move_to(points[0].x, points[0].y);
                    for p in &points[1..] {
                        pb.line_to(p.x, p.y);
                    }
                    if let Some(path) = pb.finish() {
                        let st = tiny_skia::Stroke {
                            width: stroke.width,
                            line_cap: tiny_skia::LineCap::Butt,
                            line_join: tiny_skia::LineJoin::Miter,
                            ..Default::default()
                        };
                        let b = bounds(points).expand(stroke.width);
                        if let Some(mask) = self.mask_for(pm, b, clip) {
                            pm.stroke_path(
                                &path,
                                &paint(stroke.color),
                                &st,
                                Transform::identity(),
                                mask,
                            );
                        }
                    }
                }
                Shape::Polygon {
                    points,
                    fill,
                    stroke,
                } => {
                    if points.len() < 3 {
                        continue;
                    }
                    let mut pb = PathBuilder::new();
                    pb.move_to(points[0].x, points[0].y);
                    for p in &points[1..] {
                        pb.line_to(p.x, p.y);
                    }
                    pb.close();
                    if let Some(path) = pb.finish() {
                        let b = bounds(points).expand(stroke.width + 1.0);
                        if let Some(mask) = self.mask_for(pm, b, clip) {
                            if !fill.is_transparent() {
                                pm.fill_path(
                                    &path,
                                    &paint(*fill),
                                    FillRule::Winding,
                                    Transform::identity(),
                                    mask,
                                );
                            }
                            if !stroke.is_empty() {
                                let st = tiny_skia::Stroke {
                                    width: stroke.width,
                                    ..Default::default()
                                };
                                pm.stroke_path(
                                    &path,
                                    &paint(stroke.color),
                                    &st,
                                    Transform::identity(),
                                    mask,
                                );
                            }
                        }
                    }
                }
                Shape::Circle {
                    center,
                    radius,
                    fill,
                    stroke,
                } => {
                    if let Some(path) =
                        PathBuilder::from_circle(center.x, center.y, radius.max(0.1))
                    {
                        let b = Rect::from_center_size(*center, vec2(*radius * 2.0, *radius * 2.0))
                            .expand(stroke.width + 1.0);
                        if let Some(mask) = self.mask_for(pm, b, clip) {
                            if !fill.is_transparent() {
                                pm.fill_path(
                                    &path,
                                    &paint(*fill),
                                    FillRule::Winding,
                                    Transform::identity(),
                                    mask,
                                );
                            }
                            if !stroke.is_empty() {
                                let st = tiny_skia::Stroke {
                                    width: stroke.width,
                                    ..Default::default()
                                };
                                pm.stroke_path(
                                    &path,
                                    &paint(stroke.color),
                                    &st,
                                    Transform::identity(),
                                    mask,
                                );
                            }
                        }
                    }
                }
                Shape::Text { pos, galley, color } => {
                    if color.is_transparent() {
                        continue;
                    }
                    let mut y = pos.y.round() as i32;
                    let x0 = pos.x.round() as i32;
                    let rh = galley.row_height;
                    for (i, line) in galley.lines.iter().enumerate() {
                        let ly = (pos.y + rh * i as f32).round() as i32;
                        y = ly;
                        if (ly as f32) > clip.bottom() || (ly as f32 + rh) < clip.top() {
                            continue;
                        }
                        fonts.for_each_glyph(
                            &line.text,
                            galley.mono,
                            galley.font_size,
                            |gx, gy, m| {
                                blit_mask(pm, x0 + gx, y + gy, m, *color, clip);
                            },
                        );
                    }
                    let _ = y;
                }
                Shape::Image { texture, rect } => {
                    let tw = texture.0.pixmap.width() as f32;
                    let th = texture.0.pixmap.height() as f32;
                    if tw <= 0.0 || th <= 0.0 || !rect.is_positive() {
                        continue;
                    }
                    let sx = rect.width() / tw;
                    let sy = rect.height() / th;
                    let pattern = tiny_skia::Pattern::new(
                        texture.0.pixmap.as_ref(),
                        tiny_skia::SpreadMode::Pad,
                        tiny_skia::FilterQuality::Bicubic,
                        1.0,
                        Transform::from_row(sx, 0.0, 0.0, sy, rect.left(), rect.top()),
                    );
                    let r = rect.intersect(clip);
                    if let Some(tr) =
                        tiny_skia::Rect::from_ltrb(r.left(), r.top(), r.right(), r.bottom())
                    {
                        let mut p = Paint {
                            shader: pattern,
                            ..Default::default()
                        };
                        p.anti_alias = false;
                        pm.fill_rect(tr, &p, Transform::identity(), None);
                    }
                    let _ = PixmapPaint::default();
                    let _ = Shader::SolidColor(tiny_skia::Color::BLACK);
                }
            }
        }
    }

    /// `None` when the shape is fully outside its clip; `Some(None)` when it
    /// is fully inside; `Some(Some(mask))` when it straddles.
    fn mask_for(&mut self, pm: &Pixmap, b: Rect, clip: Rect) -> Option<Option<&Mask>> {
        if !b.intersects(clip) {
            return None;
        }
        if clip.contains_rect(b) {
            return Some(None);
        }
        let idx = match self.masks.iter().position(|(r, _)| *r == clip) {
            Some(i) => i,
            None => {
                let mut mask = Mask::new(pm.width(), pm.height())?;
                let r = tiny_skia::Rect::from_ltrb(
                    clip.left(),
                    clip.top(),
                    clip.right(),
                    clip.bottom(),
                )?;
                let path = PathBuilder::from_rect(r);
                mask.fill_path(&path, FillRule::Winding, false, Transform::identity());
                if self.masks.len() > 8 {
                    self.masks.remove(0);
                }
                self.masks.push((clip, mask));
                self.masks.len() - 1
            }
        };
        Some(Some(&self.masks[idx].1))
    }

    fn fill_rect(&mut self, pm: &mut Pixmap, rect: Rect, rounding: f32, c: Color32, clip: Rect) {
        let r = rect.intersect(clip);
        if !r.is_positive() {
            return;
        }
        if rounding <= 0.3 || r != rect {
            if let Some(tr) = tiny_skia::Rect::from_ltrb(r.left(), r.top(), r.right(), r.bottom()) {
                let mut p = paint(c);
                // Whole-pixel rects (most panels) need no anti-aliasing.
                p.anti_alias = !(is_int(r.left())
                    && is_int(r.top())
                    && is_int(r.right())
                    && is_int(r.bottom()));
                if p.anti_alias && cfg!(debug_assertions) {
                    // tiny-skia's anti-aliased rect fill has a debug-only
                    // assertion (hairline_aa fill_dot8) that fractional
                    // rects can trip; a path fill draws the same rect.
                    let path = tiny_skia::PathBuilder::from_rect(tr);
                    pm.fill_path(&path, &p, FillRule::Winding, Transform::identity(), None);
                } else {
                    pm.fill_rect(tr, &p, Transform::identity(), None);
                }
            }
            return;
        }
        if let Some(path) = rounded(rect, rounding) {
            pm.fill_path(
                &path,
                &paint(c),
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }

    fn stroke_rect(&mut self, pm: &mut Pixmap, rect: Rect, rounding: f32, s: Stroke, clip: Rect) {
        // One-pixel frames land on pixel centres, inside the rect.
        let r = if (s.width - 1.0).abs() < 0.01 {
            Rect::from_min_max(
                pos2(rect.left().floor() + 0.5, rect.top().floor() + 0.5),
                pos2(
                    (rect.right() - 1.0).ceil() + 0.5,
                    (rect.bottom() - 1.0).ceil() + 0.5,
                ),
            )
        } else {
            rect
        };
        let path = if rounding > 0.3 {
            rounded(r, rounding)
        } else {
            tiny_skia::Rect::from_ltrb(
                r.left(),
                r.top(),
                r.right().max(r.left() + 0.1),
                r.bottom().max(r.top() + 0.1),
            )
            .map(PathBuilder::from_rect)
        };
        if let Some(path) = path {
            let st = tiny_skia::Stroke {
                width: s.width,
                ..Default::default()
            };
            let b = rect.expand(s.width + 1.0);
            if let Some(mask) = self.mask_for(pm, b, clip) {
                pm.stroke_path(&path, &paint(s.color), &st, Transform::identity(), mask);
            }
        }
    }
}

fn is_int(v: f32) -> bool {
    (v - v.round()).abs() < 0.01
}

fn bounds(points: &[Pos2]) -> Rect {
    points
        .iter()
        .fold(Rect::NOTHING, |r, p| r.union(Rect::from_min_max(*p, *p)))
}

fn rounded(r: Rect, radius: f32) -> Option<tiny_skia::Path> {
    let rad = radius.min(r.width() / 2.0).min(r.height() / 2.0).max(0.0);
    let (l, t, rr, b) = (r.left(), r.top(), r.right(), r.bottom());
    let k = rad * 0.4477;
    let mut pb = PathBuilder::new();
    pb.move_to(l + rad, t);
    pb.line_to(rr - rad, t);
    pb.cubic_to(rr - k, t, rr, t + k, rr, t + rad);
    pb.line_to(rr, b - rad);
    pb.cubic_to(rr, b - k, rr - k, b, rr - rad, b);
    pb.line_to(l + rad, b);
    pb.cubic_to(l + k, b, l, b - k, l, b - rad);
    pb.line_to(l, t + rad);
    pb.cubic_to(l, t + k, l + k, t, l + rad, t);
    pb.close();
    pb.finish()
}

/// Blend a glyph coverage mask in `c` into the premultiplied pixmap.
fn blit_mask(pm: &mut Pixmap, x: i32, y: i32, m: &super::text::Mask, c: Color32, clip: Rect) {
    let (w, h) = (pm.width() as i32, pm.height() as i32);
    let x0 = x.max(clip.left().floor() as i32).max(0);
    let y0 = y.max(clip.top().floor() as i32).max(0);
    let x1 = (x + m.w).min(clip.right().ceil() as i32).min(w);
    let y1 = (y + m.h).min(clip.bottom().ceil() as i32).min(h);
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    let ca = c.a() as u32;
    let (cr, cg, cb) = (c.r() as u32, c.g() as u32, c.b() as u32);
    let data = pm.data_mut();
    for py in y0..y1 {
        let srow = ((py - y) * m.w) as usize;
        let drow = (py * w) as usize * 4;
        for px in x0..x1 {
            let cov = m.coverage[srow + (px - x) as usize] as u32;
            if cov == 0 {
                continue;
            }
            let a = cov * ca / 255;
            if a == 0 {
                continue;
            }
            let i = drow + px as usize * 4;
            let inv = 255 - a;
            data[i] = ((cr * a + data[i] as u32 * inv + 127) / 255) as u8;
            data[i + 1] = ((cg * a + data[i + 1] as u32 * inv + 127) / 255) as u8;
            data[i + 2] = ((cb * a + data[i + 2] as u32 * inv + 127) / 255) as u8;
            data[i + 3] = (a + data[i + 3] as u32 * inv / 255).min(255) as u8;
        }
    }
}

/// Convert `region` of the pixmap into a full-frame BGRX buffer.
pub fn to_bgrx_region(pm: &Pixmap, out: &mut [u8], region: (u32, u32, u32, u32)) {
    let w = pm.width() as usize;
    let (rx, ry, rw, rh) = (
        region.0 as usize,
        region.1 as usize,
        region.2 as usize,
        region.3 as usize,
    );
    let src = pm.data();
    for y in ry..(ry + rh).min(pm.height() as usize) {
        let o = (y * w + rx) * 4;
        let n = rw.min(w - rx) * 4;
        for (s, d) in src[o..o + n]
            .chunks_exact(4)
            .zip(out[o..o + n].chunks_exact_mut(4))
        {
            d[0] = s[2];
            d[1] = s[1];
            d[2] = s[0];
            d[3] = 0;
        }
    }
}

/// Premultiplied RGBA to the X server's 0x00RRGGBB (little endian BGRX).
pub fn to_bgrx(pm: &Pixmap, out: &mut [u8]) {
    for (src, dst) in pm.data().chunks_exact(4).zip(out.chunks_exact_mut(4)) {
        dst[0] = src[2];
        dst[1] = src[1];
        dst[2] = src[0];
        dst[3] = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rects_text_and_clip_paint_where_expected() {
        let mut pm = Pixmap::new(40, 20).unwrap();
        let mut fonts = Fonts::new();
        let mut r = Raster::new();
        let clip = Rect::from_min_size(pos2(0.0, 0.0), vec2(20.0, 20.0));
        let shapes = vec![
            ClippedShape {
                clip: Rect::EVERYTHING,
                shape: Shape::Rect {
                    rect: Rect::from_min_size(pos2(0.0, 0.0), vec2(40.0, 20.0)),
                    rounding: 0.0,
                    fill: Color32::from_rgb(10, 20, 30),
                    stroke: Stroke::NONE,
                },
            },
            ClippedShape {
                clip,
                shape: Shape::Rect {
                    rect: Rect::from_min_size(pos2(0.0, 0.0), vec2(40.0, 20.0)),
                    rounding: 0.0,
                    fill: Color32::WHITE,
                    stroke: Stroke::NONE,
                },
            },
        ];
        r.paint(&mut pm, &mut fonts, &shapes);
        let px = |x: usize, y: usize| {
            let i = (y * 40 + x) * 4;
            [pm.data()[i], pm.data()[i + 1], pm.data()[i + 2]]
        };
        assert_eq!(px(5, 5), [255, 255, 255]);
        assert_eq!(px(30, 5), [10, 20, 30]);
        let mut out = vec![0u8; 40 * 20 * 4];
        to_bgrx(&pm, &mut out);
        let i = (5 * 40 + 30) * 4;
        assert_eq!(&out[i..i + 3], &[30, 20, 10]);
    }
}
