//! Original Winamp-inspired drawing primitives.
use crate::theme::Palette;
use egui::{
    pos2, vec2, Color32, FontId, Id, Key, Painter, Rect, Response, Rounding, Sense, Shape, Stroke,
    Ui, Vec2,
};

pub trait ResponseExt {
    /// Mouse click or Enter on a keyboard-focused control. Space stays bound to
    /// transport globally, so Enter is the explicit activation key.
    fn activated(&self, ui: &Ui) -> bool;
    fn draw_focus(&self, painter: &Painter, rect: Rect, palette: &Palette);
}

impl ResponseExt for Response {
    fn activated(&self, ui: &Ui) -> bool {
        self.clicked() || (self.has_focus() && ui.input(|i| i.key_pressed(Key::Enter)))
    }
    fn draw_focus(&self, painter: &Painter, rect: Rect, palette: &Palette) {
        if self.has_focus() {
            painter.rect_stroke(
                rect.expand(1.0),
                Rounding::same(3.0),
                Stroke::new(1.5_f32, palette.accent),
            );
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Play,
    Pause,
    Stop,
    Prev,
    Next,
    Eject,
    Shuffle,
    Repeat,
    Equalizer,
    Playlist,
    Search,
    Settings,
    Browser,
    Volume,
    Mute,
    Pin,
    Minimize,
    Close,
    Add,
    Remove,
    Up,
    Down,
    Heart,
    Back,
    Clock,
    Queue,
    Refresh,
}

pub fn fmt_time(seconds: f64) -> String {
    let s = seconds.max(0.0).round() as i64;
    let h = s / 3600;
    let m = (s % 3600) / 60;
    let sec = s % 60;
    if h > 0 {
        format!("{h}:{m:02}:{sec:02}")
    } else {
        format!("{m}:{sec:02}")
    }
}

pub fn fmt_duration(seconds: f64) -> String {
    let s = seconds.max(0.0).round() as i64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

pub fn bevel_rect(painter: &Painter, rect: Rect, fill: Color32, palette: &Palette) {
    let r = Rect::from_min_max(rect.min + vec2(1.0, 1.0), rect.max - vec2(1.0, 1.0));
    painter.rect_filled(r, Rounding::same(1.0), fill);
    painter.line_segment(
        [rect.left_top(), rect.right_top()],
        Stroke::new(1.0_f32, palette.border_hi),
    );
    painter.line_segment(
        [rect.left_top(), rect.left_bottom()],
        Stroke::new(1.0_f32, palette.border_hi),
    );
    painter.line_segment(
        [rect.right_top(), rect.right_bottom()],
        Stroke::new(1.0_f32, palette.shadow),
    );
    painter.line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(1.0_f32, palette.shadow),
    );
}

pub fn inset_rect(painter: &Painter, rect: Rect, fill: Color32, palette: &Palette) {
    painter.rect_filled(rect, Rounding::same(2.0), fill);
    painter.line_segment(
        [rect.left_top(), rect.right_top()],
        Stroke::new(1.0_f32, palette.shadow),
    );
    painter.line_segment(
        [rect.left_top(), rect.left_bottom()],
        Stroke::new(1.0_f32, palette.shadow),
    );
    painter.line_segment(
        [rect.right_top(), rect.right_bottom()],
        Stroke::new(1.0_f32, palette.border_hi),
    );
    painter.line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(1.0_f32, palette.border_hi),
    );
}

pub fn icon_button(
    ui: &mut Ui,
    palette: &Palette,
    icon: Icon,
    active: bool,
    enabled: bool,
    size: Vec2,
    tooltip: &str,
) -> Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    icon_button_rect(
        ui,
        response.id,
        rect,
        palette,
        icon,
        active,
        enabled,
        tooltip,
    )
}

pub fn icon_button_rect(
    ui: &mut Ui,
    id: Id,
    rect: Rect,
    palette: &Palette,
    icon: Icon,
    active: bool,
    enabled: bool,
    tooltip: &str,
) -> Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let response = ui.interact(rect, id, sense);
    let hovered = enabled && response.hovered();
    let pressed = enabled && response.is_pointer_button_down_on();
    let fill = if !enabled {
        palette.surface
    } else if pressed {
        palette.inset
    } else if active {
        palette.selected
    } else if hovered {
        palette.surface_hi
    } else {
        palette.surface
    };
    if pressed {
        painter_bevel_inset(ui.painter(), rect, fill, palette);
    } else {
        bevel_rect(ui.painter(), rect, fill, palette);
    }
    let color = if !enabled {
        palette.disabled
    } else if active {
        palette.accent
    } else {
        palette.foreground
    };
    draw_icon(ui.painter(), icon, rect.shrink(rect.height() * 0.22), color);
    if response.has_focus() && enabled {
        ui.painter().rect_stroke(
            rect.expand(1.0),
            Rounding::same(2.0),
            Stroke::new(1.5_f32, palette.accent),
        );
    }
    if enabled && !tooltip.is_empty() {
        response.on_hover_text(tooltip)
    } else {
        response
    }
}

fn painter_bevel_inset(painter: &Painter, rect: Rect, fill: Color32, palette: &Palette) {
    painter.rect_filled(rect, Rounding::same(2.0), fill);
    painter.line_segment(
        [rect.left_top(), rect.right_top()],
        Stroke::new(1.0_f32, palette.shadow),
    );
    painter.line_segment(
        [rect.left_top(), rect.left_bottom()],
        Stroke::new(1.0_f32, palette.shadow),
    );
    painter.line_segment(
        [rect.right_top(), rect.right_bottom()],
        Stroke::new(1.0_f32, palette.border_hi),
    );
    painter.line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(1.0_f32, palette.border_hi),
    );
}

pub fn draw_icon(painter: &Painter, icon: Icon, rect: Rect, color: Color32) {
    let w = rect.width();
    let h = rect.height();
    let thick = (w.min(h) * 0.13).clamp(1.3, 3.0);
    let stroke = Stroke::new(thick, color);
    let c = rect.center();
    let poly = |pts: Vec<egui::Pos2>| painter.add(Shape::convex_polygon(pts, color, Stroke::NONE));
    match icon {
        Icon::Play => {
            let r = rect.shrink2(vec2(w * 0.18, h * 0.08));
            poly(vec![
                pos2(r.left(), r.top()),
                pos2(r.left(), r.bottom()),
                pos2(r.right(), c.y),
            ]);
        }
        Icon::Pause => {
            let bar = (w * 0.22).max(2.0);
            painter.rect_filled(
                Rect::from_min_max(
                    pos2(rect.left() + w * 0.18, rect.top() + h * 0.12),
                    pos2(rect.left() + w * 0.18 + bar, rect.bottom() - h * 0.12),
                ),
                Rounding::same(1.0),
                color,
            );
            painter.rect_filled(
                Rect::from_min_max(
                    pos2(rect.right() - w * 0.18 - bar, rect.top() + h * 0.12),
                    pos2(rect.right() - w * 0.18, rect.bottom() - h * 0.12),
                ),
                Rounding::same(1.0),
                color,
            );
        }
        Icon::Stop => {
            painter.rect_filled(
                rect.shrink2(vec2(w * 0.20, h * 0.20)),
                Rounding::same(1.0),
                color,
            );
        }
        Icon::Prev | Icon::Next => {
            let flip = icon == Icon::Prev;
            let x = if flip {
                rect.left() + w * 0.18
            } else {
                rect.right() - w * 0.18
            };
            let bx = if flip { x + w * 0.18 } else { x - w * 0.18 };
            painter.rect_filled(
                Rect::from_min_max(
                    pos2(bx.min(x) - w * 0.06, rect.top() + h * 0.14),
                    pos2(bx.max(x) + w * 0.06, rect.bottom() - h * 0.14),
                ),
                Rounding::same(1.0),
                color,
            );
            if flip {
                poly(vec![
                    pos2(rect.right() - w * 0.10, rect.top() + h * 0.10),
                    pos2(rect.right() - w * 0.10, rect.bottom() - h * 0.10),
                    pos2(rect.left() + w * 0.26, c.y),
                ]);
            } else {
                poly(vec![
                    pos2(rect.left() + w * 0.10, rect.top() + h * 0.10),
                    pos2(rect.left() + w * 0.10, rect.bottom() - h * 0.10),
                    pos2(rect.right() - w * 0.26, c.y),
                ]);
            }
        }
        Icon::Eject => {
            poly(vec![
                pos2(rect.left() + w * 0.12, rect.top() + h * 0.62),
                pos2(rect.right() - w * 0.12, rect.top() + h * 0.62),
                pos2(c.x, rect.top() + h * 0.14),
            ]);
            painter.rect_filled(
                Rect::from_min_max(
                    pos2(rect.left() + w * 0.16, rect.bottom() - h * 0.20),
                    pos2(rect.right() - w * 0.16, rect.bottom() - h * 0.08),
                ),
                Rounding::same(1.0),
                color,
            );
        }
        Icon::Shuffle => {
            let p1 = pos2(rect.left() + w * 0.12, rect.bottom() - h * 0.20);
            let p2 = pos2(rect.right() - w * 0.20, rect.top() + h * 0.22);
            painter.line_segment([p1, p2], stroke);
            painter.line_segment(
                [
                    pos2(rect.left() + w * 0.12, rect.top() + h * 0.22),
                    pos2(c.x, c.y),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    pos2(c.x, c.y),
                    pos2(rect.right() - w * 0.20, rect.bottom() - h * 0.20),
                ],
                stroke,
            );
            arrow_head(painter, p2, vec2(1.0, -1.0), color, thick);
            arrow_head(
                painter,
                pos2(rect.right() - w * 0.20, rect.bottom() - h * 0.20),
                vec2(1.0, 1.0),
                color,
                thick,
            );
        }
        Icon::Repeat => {
            let r = rect.shrink2(vec2(w * 0.10, h * 0.12));
            painter.add(Shape::line(
                vec![
                    pos2(r.left(), r.bottom()),
                    pos2(r.left(), r.top() + h * 0.18),
                    pos2(r.right() - h * 0.12, r.top() + h * 0.18),
                    pos2(r.right() - h * 0.12, r.bottom()),
                ],
                stroke,
            ));
            arrow_head(
                painter,
                pos2(r.right() - h * 0.12, r.bottom()),
                vec2(0.0, 1.0),
                color,
                thick,
            );
            arrow_head(
                painter,
                pos2(r.left(), r.top() + h * 0.18),
                vec2(0.0, -1.0),
                color,
                thick,
            );
        }
        Icon::Equalizer => {
            for (i, frac) in [0.30, 0.68, 0.45].iter().enumerate() {
                let x = rect.left() + w * (0.22 + i as f32 * 0.28);
                painter.line_segment(
                    [
                        pos2(x, rect.top() + h * 0.14),
                        pos2(x, rect.bottom() - h * 0.14),
                    ],
                    Stroke::new(thick * 0.8, color),
                );
                let y = rect.top() + h * frac;
                painter.rect_filled(
                    Rect::from_center_size(pos2(x, y), vec2(w * 0.18, h * 0.12)),
                    Rounding::same(1.0),
                    color,
                );
            }
        }
        Icon::Playlist | Icon::Queue => {
            for (i, frac) in [0.22, 0.50, 0.78].iter().enumerate() {
                let y = rect.top() + h * frac;
                painter.line_segment(
                    [
                        pos2(rect.left() + w * 0.10, y),
                        pos2(rect.right() - w * (if i == 0 { 0.30 } else { 0.10 }), y),
                    ],
                    Stroke::new(thick * 0.8, color),
                );
            }
            if icon == Icon::Queue {
                painter.circle_filled(
                    pos2(rect.left() + w * 0.06, rect.bottom() - h * 0.08),
                    thick * 0.8,
                    color,
                );
            }
        }
        Icon::Search => {
            let r = w.min(h) * 0.26;
            painter.circle_stroke(pos2(c.x - w * 0.06, c.y - h * 0.06), r, stroke);
            painter.line_segment(
                [
                    pos2(c.x + r * 0.55, c.y + r * 0.55),
                    pos2(rect.right() - w * 0.12, rect.bottom() - h * 0.12),
                ],
                stroke,
            );
        }
        Icon::Settings => {
            painter.circle_stroke(c, w.min(h) * 0.22, stroke);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                let p1 = c + vec2(a.cos(), a.sin()) * (w.min(h) * 0.30);
                let p2 = c + vec2(a.cos(), a.sin()) * (w.min(h) * 0.44);
                painter.line_segment([p1, p2], stroke);
            }
        }
        Icon::Browser => {
            let r = rect.shrink2(vec2(w * 0.10, h * 0.14));
            painter.rect_stroke(r, Rounding::same(1.0), stroke);
            painter.line_segment(
                [
                    pos2(r.left(), r.top() + r.height() * 0.28),
                    pos2(r.right(), r.top() + r.height() * 0.28),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    pos2(r.left() + r.width() * 0.34, r.top() + r.height() * 0.28),
                    pos2(r.left() + r.width() * 0.34, r.bottom()),
                ],
                stroke,
            );
        }
        Icon::Volume | Icon::Mute => {
            poly(vec![
                pos2(rect.left() + w * 0.10, c.y - h * 0.14),
                pos2(rect.left() + w * 0.32, c.y - h * 0.14),
                pos2(rect.left() + w * 0.52, c.y - h * 0.30),
                pos2(rect.left() + w * 0.52, c.y + h * 0.30),
                pos2(rect.left() + w * 0.32, c.y + h * 0.14),
                pos2(rect.left() + w * 0.10, c.y + h * 0.14),
            ]);
            if icon == Icon::Volume {
                painter.add(Shape::line(
                    vec![
                        pos2(rect.left() + w * 0.64, c.y - h * 0.16),
                        pos2(rect.right() - w * 0.10, c.y - h * 0.30),
                    ],
                    Stroke::new(thick * 0.7, color),
                ));
                painter.add(Shape::line(
                    vec![
                        pos2(rect.left() + w * 0.64, c.y + h * 0.16),
                        pos2(rect.right() - w * 0.10, c.y + h * 0.30),
                    ],
                    Stroke::new(thick * 0.7, color),
                ));
            } else {
                painter.line_segment(
                    [
                        pos2(rect.left() + w * 0.62, c.y - h * 0.22),
                        pos2(rect.right() - w * 0.10, c.y + h * 0.22),
                    ],
                    Stroke::new(thick, color),
                );
                painter.line_segment(
                    [
                        pos2(rect.left() + w * 0.62, c.y + h * 0.22),
                        pos2(rect.right() - w * 0.10, c.y - h * 0.22),
                    ],
                    Stroke::new(thick, color),
                );
            }
        }
        Icon::Pin => {
            painter.circle_filled(pos2(c.x, rect.top() + h * 0.32), w * 0.13, color);
            painter.line_segment(
                [
                    pos2(c.x, rect.top() + h * 0.40),
                    pos2(c.x, rect.bottom() - h * 0.12),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    pos2(c.x - w * 0.16, rect.bottom() - h * 0.12),
                    pos2(c.x + w * 0.16, rect.bottom() - h * 0.12),
                ],
                stroke,
            );
        }
        Icon::Minimize => {
            painter.line_segment(
                [
                    pos2(rect.left() + w * 0.18, rect.bottom() - h * 0.24),
                    pos2(rect.right() - w * 0.18, rect.bottom() - h * 0.24),
                ],
                stroke,
            );
        }
        Icon::Close => {
            painter.line_segment(
                [
                    pos2(rect.left() + w * 0.20, rect.top() + h * 0.20),
                    pos2(rect.right() - w * 0.20, rect.bottom() - h * 0.20),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    pos2(rect.left() + w * 0.20, rect.bottom() - h * 0.20),
                    pos2(rect.right() - w * 0.20, rect.top() + h * 0.20),
                ],
                stroke,
            );
        }
        Icon::Add => {
            painter.line_segment(
                [
                    pos2(rect.left() + w * 0.18, c.y),
                    pos2(rect.right() - w * 0.18, c.y),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    pos2(c.x, rect.top() + h * 0.18),
                    pos2(c.x, rect.bottom() - h * 0.18),
                ],
                stroke,
            );
        }
        Icon::Remove => {
            painter.line_segment(
                [
                    pos2(rect.left() + w * 0.20, c.y),
                    pos2(rect.right() - w * 0.20, c.y),
                ],
                stroke,
            );
        }
        Icon::Up => {
            poly(vec![
                pos2(rect.left() + w * 0.16, rect.bottom() - h * 0.22),
                pos2(rect.right() - w * 0.16, rect.bottom() - h * 0.22),
                pos2(c.x, rect.top() + h * 0.18),
            ]);
        }
        Icon::Down => {
            poly(vec![
                pos2(rect.left() + w * 0.16, rect.top() + h * 0.22),
                pos2(rect.right() - w * 0.16, rect.top() + h * 0.22),
                pos2(c.x, rect.bottom() - h * 0.18),
            ]);
        }
        Icon::Heart => {
            let r = w.min(h) * 0.28;
            painter.circle_filled(pos2(c.x - r * 0.55, c.y - r * 0.35), r * 0.7, color);
            painter.circle_filled(pos2(c.x + r * 0.55, c.y - r * 0.35), r * 0.7, color);
            poly(vec![
                pos2(c.x - r * 1.15, c.y - r * 0.2),
                pos2(c.x + r * 1.15, c.y - r * 0.2),
                pos2(c.x, c.y + r * 1.25),
            ]);
        }
        Icon::Back => {
            painter.line_segment(
                [
                    pos2(rect.left() + w * 0.22, c.y),
                    pos2(rect.right() - w * 0.18, c.y),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    pos2(rect.left() + w * 0.22, c.y),
                    pos2(rect.left() + w * 0.50, c.y - h * 0.22),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    pos2(rect.left() + w * 0.22, c.y),
                    pos2(rect.left() + w * 0.50, c.y + h * 0.22),
                ],
                stroke,
            );
        }
        Icon::Clock => {
            painter.circle_stroke(c, w.min(h) * 0.38, stroke);
            painter.line_segment([c, pos2(c.x, c.y - h * 0.20)], stroke);
            painter.line_segment([c, pos2(c.x + w * 0.15, c.y + h * 0.06)], stroke);
        }
        Icon::Refresh => {
            let r = w.min(h) * 0.34;
            painter.add(Shape::line(
                vec![pos2(c.x + r * 0.4, c.y - r), pos2(c.x + r, c.y - r * 0.4)],
                stroke,
            ));
            painter.circle_stroke(c, r, stroke);
            arrow_head(
                painter,
                pos2(c.x - r * 0.4, c.y - r),
                vec2(-1.0, 0.0),
                color,
                thick,
            );
            arrow_head(
                painter,
                pos2(c.x + r, c.y + r * 0.5),
                vec2(0.5, 1.0),
                color,
                thick,
            );
        }
    }
}

fn arrow_head(painter: &Painter, tip: egui::Pos2, dir: Vec2, color: Color32, thick: f32) {
    let d = dir.normalized();
    let n = vec2(-d.y, d.x);
    let s = thick * 2.4;
    painter.add(Shape::convex_polygon(
        vec![tip, tip - d * s + n * s * 0.55, tip - d * s - n * s * 0.55],
        color,
        Stroke::NONE,
    ));
}

pub fn seven_segment(painter: &Painter, rect: Rect, text: &str, on: Color32, off: Color32) {
    let chars: Vec<char> = text.chars().collect();
    let digit_w = rect.height() * 0.56;
    let gap = digit_w * 0.16;
    let colon_w = digit_w * 0.30;
    let total: f32 = chars
        .iter()
        .map(|c| {
            if *c == ':' {
                colon_w
            } else if *c == '-' {
                digit_w * 0.6
            } else {
                digit_w
            }
        })
        .sum::<f32>()
        + gap * (chars.len().saturating_sub(1) as f32);
    let mut x = rect.center().x - total / 2.0;
    let y = rect.center().y - rect.height() / 2.0;
    for ch in chars {
        if ch == ':' {
            let r = (digit_w * 0.075).max(1.2);
            painter.circle_filled(pos2(x + colon_w / 2.0, y + rect.height() * 0.32), r, on);
            painter.circle_filled(pos2(x + colon_w / 2.0, y + rect.height() * 0.68), r, on);
            x += colon_w + gap;
        } else if ch == '-' {
            let seg = segment_rect(x, y, digit_w * 0.6, rect.height(), 0);
            painter.rect_filled(seg, Rounding::same(1.0), on);
            x += digit_w * 0.6 + gap;
        } else if let Some(d) = ch.to_digit(10) {
            draw_digit(
                painter,
                Rect::from_min_size(pos2(x, y), vec2(digit_w, rect.height())),
                d as u8,
                on,
                off,
            );
            x += digit_w + gap;
        } else {
            x += digit_w + gap;
        }
    }
}

fn segment_rect(x: f32, y: f32, w: f32, h: f32, index: usize) -> Rect {
    let t = (w * 0.16).max(1.2);
    let m = w * 0.10;
    match index {
        0 => Rect::from_min_max(pos2(x + m, y), pos2(x + w - m, y + t)),
        1 => Rect::from_min_max(pos2(x + w - t, y + m), pos2(x + w, y + h * 0.5 - m * 0.6)),
        2 => Rect::from_min_max(
            pos2(x + w - t, y + h * 0.5 + m * 0.6),
            pos2(x + w, y + h - m),
        ),
        3 => Rect::from_min_max(pos2(x + m, y + h - t), pos2(x + w - m, y + h)),
        4 => Rect::from_min_max(pos2(x, y + h * 0.5 + m * 0.6), pos2(x + t, y + h - m)),
        5 => Rect::from_min_max(pos2(x, y + m), pos2(x + t, y + h * 0.5 - m * 0.6)),
        _ => Rect::from_min_max(
            pos2(x + m, y + h * 0.5 - t * 0.5),
            pos2(x + w - m, y + h * 0.5 + t * 0.5),
        ),
    }
}

fn draw_digit(painter: &Painter, rect: Rect, digit: u8, on: Color32, off: Color32) {
    let active: [bool; 7] = match digit {
        0 => [true, true, true, true, true, true, false],
        1 => [false, true, true, false, false, false, false],
        2 => [true, true, false, true, true, false, true],
        3 => [true, true, true, true, false, false, true],
        4 => [false, true, true, false, false, true, true],
        5 => [true, false, true, true, false, true, true],
        6 => [true, false, true, true, true, true, true],
        7 => [true, true, true, false, false, false, false],
        8 => [true, true, true, true, true, true, true],
        9 => [true, true, true, true, false, true, true],
        _ => [false; 7],
    };
    for (i, is_active) in active.iter().enumerate() {
        let color = if *is_active { on } else { off };
        painter.rect_filled(
            segment_rect(rect.left(), rect.top(), rect.width(), rect.height(), i),
            Rounding::same(1.0),
            color,
        );
    }
}

pub fn marquee(
    ui: &Ui,
    rect: Rect,
    text: &str,
    font: FontId,
    color: Color32,
    base: Color32,
    reduced_motion: bool,
    speed: f32,
) {
    let painter = ui.painter().with_clip_rect(rect);
    if text.is_empty() {
        return;
    }
    let galley = painter.layout_no_wrap(text.to_owned(), font.clone(), color);
    let cy = rect.center().y - galley.size().y / 2.0;
    if galley.size().x <= rect.width() || reduced_motion {
        if galley.size().x <= rect.width() {
            painter.galley(pos2(rect.left(), cy), galley, color);
        } else {
            let clipped = painter.layout(text.to_owned(), font, color, rect.width());
            painter.galley(pos2(rect.left(), cy), clipped, color);
        }
        return;
    }
    let delay = 1.4;
    let now = ui.input(|i| i.time);
    let span = galley.size().x + 42.0;
    let cycle = span / speed;
    let t = (now % (cycle + delay) as f64) as f32;
    let offset = if t < delay {
        0.0
    } else {
        -((t - delay) * speed) % span
    };
    painter.galley(pos2(rect.left() + offset, cy), galley.clone(), color);
    painter.galley(pos2(rect.left() + offset + span, cy), galley, color);
    painter.line_segment(
        [rect.right_top(), rect.right_bottom()],
        Stroke::new(1.0_f32, base),
    );
}

pub struct SliderResult {
    pub response: Response,
    pub changed: bool,
    pub value: f64,
}

pub fn hardware_slider(
    ui: &mut Ui,
    palette: &Palette,
    value: f64,
    max: f64,
    enabled: bool,
    size: Vec2,
    color: Color32,
    ticks: bool,
) -> SliderResult {
    let sense = if enabled {
        Sense::click_and_drag()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    hardware_slider_rect(
        ui,
        palette,
        value,
        max,
        enabled,
        rect,
        response.id,
        color,
        ticks,
        response,
    )
}

pub fn hardware_slider_rect(
    ui: &mut Ui,
    palette: &Palette,
    value: f64,
    max: f64,
    enabled: bool,
    rect: Rect,
    id: Id,
    color: Color32,
    ticks: bool,
    response: Response,
) -> SliderResult {
    let sense = if enabled {
        Sense::click_and_drag()
    } else {
        Sense::hover()
    };
    let response = if response.sense == sense {
        response
    } else {
        ui.interact(rect, id, sense)
    };
    let groove = Rect::from_center_size(
        rect.center(),
        vec2(rect.width() - 4.0, (rect.height() * 0.34).clamp(4.0, 9.0)),
    );
    inset_rect(ui.painter(), groove, palette.inset, palette);
    let frac = if max > 0.0 {
        (value / max).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    let fill = Rect::from_min_max(
        groove.left_top(),
        pos2(groove.left() + groove.width() * frac, groove.bottom()),
    );
    ui.painter().rect_filled(fill, Rounding::same(1.0), color);
    if ticks {
        let n = 20;
        for i in 0..=n {
            let x = groove.left() + groove.width() * i as f32 / n as f32;
            ui.painter().line_segment(
                [pos2(x, groove.top() - 2.0), pos2(x, groove.top() - 1.0)],
                Stroke::new(1.0_f32, palette.muted),
            );
        }
    }
    let thumb_x = groove.left() + groove.width() * frac;
    let thumb = Rect::from_center_size(
        pos2(thumb_x, rect.center().y),
        vec2((rect.height() * 0.28).clamp(5.0, 9.0), rect.height() * 0.85),
    );
    bevel_rect(
        ui.painter(),
        thumb,
        if response.hovered() || response.dragged() {
            palette.surface_hi
        } else {
            palette.surface
        },
        palette,
    );
    let mut new_value = value;
    if enabled && (response.dragged() || response.clicked()) {
        if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
            let t = ((pos.x - groove.left()) / groove.width().max(1.0)).clamp(0.0, 1.0) as f64;
            new_value = t * max;
        }
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.expand(1.0),
            Rounding::same(2.0),
            Stroke::new(1.5_f32, palette.accent),
        );
    }
    SliderResult {
        response,
        changed: (new_value - value).abs() > f64::EPSILON,
        value: new_value,
    }
}

pub fn draw_art_placeholder(
    painter: &Painter,
    rect: Rect,
    palette: &Palette,
    seed: u64,
    label: &str,
) {
    inset_rect(painter, rect, palette.inset, palette);
    let inner = rect.shrink(3.0);
    let a = palette.surface;
    let b = palette.inset;
    let steps = 18;
    for i in 0..steps {
        let t = i as f32 / steps as f32;
        let y0 = inner.top() + inner.height() * t;
        let y1 = inner.top() + inner.height() * (t + 1.0 / steps as f32);
        let blend = (t * 0.5 + ((seed % 100) as f32 / 400.0)).clamp(0.0, 1.0);
        let color = Color32::from_rgb(
            (a.r() as f32 * (1.0 - blend) + b.r() as f32 * blend) as u8,
            (a.g() as f32 * (1.0 - blend) + b.g() as f32 * blend) as u8,
            (a.b() as f32 * (1.0 - blend) + b.b() as f32 * blend) as u8,
        );
        painter.rect_filled(
            Rect::from_min_max(pos2(inner.left(), y0), pos2(inner.right(), y1 + 0.5)),
            Rounding::ZERO,
            color,
        );
    }
    let mark = inner.shrink(inner.width() * 0.26);
    painter.circle_stroke(
        mark.center(),
        mark.width() * 0.32,
        Stroke::new(1.4_f32, palette.muted),
    );
    draw_icon(painter, Icon::Play, mark, palette.muted);
    if !label.is_empty() {
        painter.text(
            rect.left_bottom() + vec2(5.0, -4.0),
            egui::Align2::LEFT_BOTTOM,
            label,
            FontId::proportional(8.0),
            palette.muted,
        );
    }
}

pub fn logo(painter: &Painter, rect: Rect, palette: &Palette) {
    let r = rect.shrink(rect.height() * 0.10);
    painter.rect_filled(r, Rounding::same(2.0), palette.inset);
    painter.rect_stroke(
        r,
        Rounding::same(2.0),
        Stroke::new(1.0_f32, palette.border_hi),
    );
    let tri = Rect::from_center_size(r.center(), vec2(r.width() * 0.44, r.height() * 0.52));
    draw_icon(painter, Icon::Play, tri, palette.led);
}

pub fn section_label(ui: &mut Ui, text: &str, palette: &Palette) {
    ui.label(
        egui::RichText::new(text)
            .size(10.0)
            .color(palette.muted)
            .strong(),
    );
}

pub fn divider(ui: &mut Ui, palette: &Palette) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().line_segment(
        [rect.left_center(), rect.right_center()],
        Stroke::new(1.0_f32, palette.border),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_short_and_long_times() {
        assert_eq!(fmt_time(0.0), "0:00");
        assert_eq!(fmt_time(65.4), "1:05");
        assert_eq!(fmt_time(3661.0), "1:01:01");
        assert_eq!(fmt_duration(0.0), "0:00");
        assert_eq!(fmt_duration(75.0), "1:15");
    }
}
