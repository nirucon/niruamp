//! Layout (`Ui`) and the widgets NIRUAMP uses: labels, buttons, selectable
//! labels, check boxes, a slider, single-line text fields, combo boxes,
//! context menus, frames, areas and virtualised scroll areas.
use super::*;
use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Dir {
    Vertical,
    Horizontal { wrap: bool, centered: bool },
    Centered,
}

#[derive(Clone, Copy, Debug)]
pub struct Spacing {
    pub item_spacing: Vec2,
}

pub struct Layout {
    dir: Dir,
}

impl Layout {
    pub fn top_down(_align: Align) -> Self {
        Layout { dir: Dir::Vertical }
    }
    pub fn left_to_right(_align: Align) -> Self {
        Layout {
            dir: Dir::Horizontal {
                wrap: false,
                centered: false,
            },
        }
    }
}

pub struct InnerResponse<R> {
    pub inner: R,
    pub response: Response,
}

pub struct Ui {
    ctx: Context,
    id: Id,
    layer: usize,
    max_rect: Rect,
    clip: Rect,
    cursor: Pos2,
    dir: Dir,
    row_h: f32,
    min_rect: Rect,
    spacing: Spacing,
    enabled: bool,
    counter: u64,
    menu: bool,
    scroll: Option<(Id, f32)>,
}

impl Ui {
    pub(crate) fn root(ctx: &Context, id: Id, layer: usize, rect: Rect) -> Ui {
        let style = ctx.style();
        Ui {
            ctx: ctx.clone(),
            id,
            layer,
            max_rect: rect,
            clip: rect,
            cursor: rect.min,
            dir: Dir::Vertical,
            row_h: 0.0,
            min_rect: Rect::NOTHING,
            spacing: Spacing {
                item_spacing: style.item_spacing,
            },
            enabled: true,
            counter: 0,
            menu: false,
            scroll: None,
        }
    }

    fn next_id(&mut self) -> Id {
        self.counter += 1;
        self.id.with(("auto", self.counter))
    }

    fn child(&mut self, max_rect: Rect, dir: Dir) -> Ui {
        self.counter += 1;
        Ui {
            ctx: self.ctx.clone(),
            id: self.id.with(("child", self.counter)),
            layer: self.layer,
            max_rect,
            clip: self.clip,
            cursor: max_rect.min,
            dir,
            row_h: 0.0,
            min_rect: Rect::NOTHING,
            spacing: self.spacing,
            enabled: self.enabled,
            counter: 0,
            menu: self.menu,
            scroll: self.scroll,
        }
    }

    pub fn ctx(&self) -> &Context {
        &self.ctx
    }
    pub fn id(&self) -> Id {
        self.id
    }
    pub fn input<R>(&self, f: impl FnOnce(&InputState) -> R) -> R {
        self.ctx.input(f)
    }
    pub fn painter(&self) -> Painter {
        Painter {
            ctx: self.ctx.clone(),
            layer: self.layer,
            clip: self.clip,
        }
    }
    pub fn style(&self) -> Rc<Style> {
        self.ctx.style()
    }
    pub fn visuals(&self) -> Visuals {
        self.ctx.style().visuals
    }
    pub fn spacing_mut(&mut self) -> &mut Spacing {
        &mut self.spacing
    }
    pub fn max_rect(&self) -> Rect {
        self.max_rect
    }
    pub fn clip_rect(&self) -> Rect {
        self.clip
    }
    pub fn set_clip_rect(&mut self, r: Rect) {
        self.clip = r;
    }
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    pub fn set_enabled(&mut self, e: bool) {
        self.enabled = e;
    }
    pub fn cursor(&self) -> Rect {
        Rect::from_min_max(self.cursor, self.max_rect.max)
    }
    pub fn min_rect(&self) -> Rect {
        if self.min_rect.is_positive()
            || self.min_rect.width() >= 0.0 && self.min_rect.height() >= 0.0
        {
            self.min_rect
        } else {
            Rect::from_min_size(self.max_rect.min, Vec2::ZERO)
        }
    }
    pub fn available_width(&self) -> f32 {
        (self.max_rect.right() - self.cursor.x).max(0.0)
    }
    pub fn available_size(&self) -> Vec2 {
        vec2(
            self.available_width(),
            (self.max_rect.bottom() - self.cursor.y).max(0.0),
        )
    }
    pub fn set_min_width(&mut self, w: f32) {
        let r = Rect::from_min_size(self.max_rect.min, vec2(w, 0.0));
        self.extend(r);
    }
    pub fn set_max_width(&mut self, w: f32) {
        self.max_rect.max.x = self.max_rect.max.x.min(self.max_rect.min.x + w);
    }
    pub fn close_menu(&mut self) {
        self.ctx.memory_mut(|m| m.close_popup());
        self.ctx.request_repaint();
    }

    fn extend(&mut self, r: Rect) {
        self.min_rect = if self.min_rect.width() < 0.0 {
            r
        } else {
            self.min_rect.union(r)
        };
    }

    /// Place a widget of `size` and advance the cursor.
    pub fn allocate_space(&mut self, size: Vec2) -> Rect {
        let sp = self.spacing.item_spacing;
        let rect = match self.dir {
            Dir::Vertical => {
                let r = Rect::from_min_size(pos2(self.max_rect.left(), self.cursor.y), size);
                self.cursor.y = r.bottom() + sp.y;
                r
            }
            Dir::Centered => {
                let r = Rect::from_center_size(self.max_rect.center(), size);
                self.cursor.y = r.bottom() + sp.y;
                r
            }
            Dir::Horizontal { wrap, centered } => {
                if wrap
                    && self.cursor.x > self.max_rect.left()
                    && self.cursor.x + size.x > self.max_rect.right()
                {
                    self.cursor.x = self.max_rect.left();
                    self.cursor.y += self.row_h + sp.y;
                    self.row_h = 0.0;
                }
                let band = if centered {
                    self.max_rect.height()
                } else {
                    self.ctx.style().interact_height.max(self.row_h)
                };
                let y = if centered {
                    self.max_rect.top() + (band - size.y) / 2.0
                } else {
                    self.cursor.y + ((band - size.y) / 2.0).max(0.0)
                };
                let r = Rect::from_min_size(pos2(self.cursor.x, y), size);
                self.cursor.x = r.right() + sp.x;
                self.row_h = self.row_h.max(size.y.max(if centered {
                    0.0
                } else {
                    self.ctx.style().interact_height
                }));
                r
            }
        };
        self.extend(rect);
        rect
    }

    pub fn add_space(&mut self, amount: f32) {
        match self.dir {
            Dir::Vertical | Dir::Centered => {
                let r = Rect::from_min_size(
                    pos2(self.max_rect.left(), self.cursor.y),
                    vec2(0.0, amount),
                );
                self.cursor.y += amount;
                self.extend(r);
            }
            Dir::Horizontal { .. } => {
                let r = Rect::from_min_size(self.cursor, vec2(amount, 0.0));
                self.cursor.x += amount;
                self.extend(r);
            }
        }
    }

    pub fn interact(&self, rect: Rect, id: Id, sense: Sense) -> Response {
        let clipped = rect.intersect(self.clip);
        let mut r = self
            .ctx
            .interact(clipped, id, sense, self.layer, self.enabled, false);
        r.rect = rect;
        // Hovering another item of the parent menu closes an open submenu;
        // items inside the submenu itself keep it.
        if self.menu && r.hovered() && sense.interactive() {
            let inside_sub = self
                .ctx
                .memory(|m| m.submenu_rect.is_some_and(|sr| sr.contains(rect.center())));
            if !inside_sub {
                self.ctx.memory_mut(|m| {
                    if m.submenu.is_some_and(|(sid, _)| sid != id) {
                        m.submenu = None;
                        m.submenu_rect = None;
                    }
                });
            }
        }
        r
    }

    pub fn allocate_exact_size(&mut self, size: Vec2, sense: Sense) -> (Rect, Response) {
        let rect = self.allocate_space(size);
        let id = self.next_id();
        let resp = self.interact(rect, id, sense);
        (rect, resp)
    }

    pub fn allocate_rect(&mut self, rect: Rect, sense: Sense) -> Response {
        self.extend(rect);
        match self.dir {
            Dir::Vertical | Dir::Centered => {
                self.cursor.y = self
                    .cursor
                    .y
                    .max(rect.bottom() + self.spacing.item_spacing.y)
            }
            Dir::Horizontal { .. } => {
                self.cursor.x = self
                    .cursor
                    .x
                    .max(rect.right() + self.spacing.item_spacing.x);
                self.row_h = self.row_h.max(rect.height());
            }
        }
        let id = self.next_id();
        self.interact(rect, id, sense)
    }

    pub fn child_ui(&mut self, rect: Rect, layout: Layout, _ui_stack: Option<()>) -> Ui {
        self.child(rect, layout.dir)
    }

    pub fn allocate_ui_at_rect<R>(
        &mut self,
        rect: Rect,
        add: impl FnOnce(&mut Ui) -> R,
    ) -> InnerResponse<R> {
        let mut child = self.child(rect, Dir::Vertical);
        let inner = add(&mut child);
        let r = child.min_rect();
        let response = self.allocate_rect(r, Sense::hover());
        InnerResponse { inner, response }
    }

    fn with_dir<R>(&mut self, dir: Dir, add: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
        let max = Rect::from_min_max(self.cursor, self.max_rect.max);
        let max = if matches!(self.dir, Dir::Vertical | Dir::Centered) {
            Rect::from_min_max(pos2(self.max_rect.left(), self.cursor.y), self.max_rect.max)
        } else {
            max
        };
        let mut child = self.child(max, dir);
        let inner = add(&mut child);
        let mut r = child.min_rect();
        if matches!(
            dir,
            Dir::Horizontal {
                centered: false,
                ..
            }
        ) && r.height() < self.ctx.style().interact_height
        {
            r.set_height(self.ctx.style().interact_height);
        }
        let r = Rect::from_min_max(
            max.min,
            pos2(r.max.x.max(max.min.x), r.max.y.max(max.min.y)),
        );
        let response = self.allocate_rect(r, Sense::hover());
        InnerResponse { inner, response }
    }

    pub fn horizontal<R>(&mut self, add: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
        self.with_dir(
            Dir::Horizontal {
                wrap: false,
                centered: false,
            },
            add,
        )
    }
    pub fn horizontal_wrapped<R>(&mut self, add: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
        self.with_dir(
            Dir::Horizontal {
                wrap: true,
                centered: false,
            },
            add,
        )
    }
    pub fn horizontal_centered<R>(&mut self, add: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
        self.with_dir(
            Dir::Horizontal {
                wrap: false,
                centered: true,
            },
            add,
        )
    }
    pub fn vertical<R>(&mut self, add: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
        self.with_dir(Dir::Vertical, add)
    }

    pub fn scroll_to_rect(&self, rect: Rect, _align: Option<Align>) {
        if let Some((id, origin)) = self.scroll {
            self.ctx.memory_mut(|m| {
                m.scroll_request
                    .insert(id, (rect.top() - origin, rect.bottom() - origin));
            });
            self.ctx.request_repaint();
        }
    }

    // ------------------------------------------------------------ widgets

    pub fn add(&mut self, w: impl Widget) -> Response {
        w.ui(self)
    }

    pub fn add_enabled(&mut self, enabled: bool, w: impl Widget) -> Response {
        let before = self.enabled;
        self.enabled = before && enabled;
        let r = w.ui(self);
        self.enabled = before;
        r
    }

    pub fn add_sized(&mut self, size: impl Into<Vec2>, w: impl Widget) -> Response {
        let rect = self.allocate_space(size.into());
        let mut child = self.child(rect, Dir::Centered);
        let mut r = w.ui(&mut child);
        r.rect = rect;
        r
    }

    pub fn label(&mut self, text: impl Into<RichText>) -> Response {
        Label::new(text).ui(self)
    }

    pub fn button(&mut self, text: impl Into<RichText>) -> Response {
        Button::new(text).ui(self)
    }

    pub fn small_button(&mut self, text: impl Into<RichText>) -> Response {
        Button::new(text).small().ui(self)
    }

    pub fn selectable_label(&mut self, selected: bool, text: impl Into<RichText>) -> Response {
        let text: RichText = text.into();
        let style = self.ctx.style();
        let (font, color) = text.resolve(&style, style.visuals.text);
        let galley = self
            .ctx
            .fonts_mut(|f| f.layout_no_wrap(&text.text, font.mono, font.size));
        let pad = style.button_padding;
        let mut size = galley.size + pad * 2.0;
        size.y = size.y.max(style.interact_height);
        if self.menu {
            size.x = size.x.max(self.available_width());
        }
        let (rect, resp) = self.allocate_exact_size(size, Sense::click());
        let v = style.visuals;
        let p = self.painter();
        if selected {
            p.rect_filled(rect, 2.0, v.selection_bg);
        } else if resp.hovered() {
            p.rect_filled(rect, 2.0, v.hovered_weak_bg);
        }
        if resp.has_focus() {
            p.rect_stroke(rect, 2.0, Stroke::new(1.0, v.hovered_stroke));
        }
        let c = if self.enabled {
            color
        } else {
            color.gamma_multiply(0.5)
        };
        let pos = pos2(rect.left() + pad.x, rect.center().y - galley.size.y / 2.0);
        p.galley(pos, Rc::new(galley), c);
        if resp.clicked() && self.menu {
            self.ctx.memory_mut(|m| {
                if m.popup.is_some_and(|p| p.kind == PopupKind::Combo) {
                    m.close_popup();
                }
            });
        }
        resp
    }

    pub fn selectable_value<T: PartialEq>(
        &mut self,
        current: &mut T,
        value: T,
        text: impl Into<RichText>,
    ) -> Response {
        let mut r = self.selectable_label(*current == value, text);
        if r.clicked() && *current != value {
            *current = value;
            r.changed = true;
        }
        r
    }

    pub fn checkbox(&mut self, checked: &mut bool, text: impl Into<RichText>) -> Response {
        Checkbox::new(checked, text).ui(self)
    }

    pub fn separator(&mut self) -> Response {
        let w = self.available_width();
        let (rect, resp) = self.allocate_exact_size(vec2(w, 6.0), Sense::hover());
        let v = self.ctx.style().visuals;
        self.painter().line_segment(
            [rect.left_center(), rect.right_center()],
            Stroke::new(1.0, v.window_stroke),
        );
        resp
    }

    pub fn spinner(&mut self) -> Response {
        let s = self.ctx.style().interact_height;
        let (rect, resp) = self.allocate_exact_size(vec2(s, s), Sense::hover());
        let t = self.ctx.input(|i| i.time) as f32;
        let c = rect.center();
        let r = rect.height() * 0.38;
        let start = t * 4.0;
        let pts: Vec<Pos2> = (0..=16)
            .map(|i| {
                let a = start + i as f32 / 16.0 * std::f32::consts::PI * 1.4;
                c + vec2(a.cos(), a.sin()) * r
            })
            .collect();
        let color = self.ctx.style().visuals.text;
        self.painter()
            .add(Shape::line(pts, Stroke::new(2.0, color)));
        self.ctx
            .request_repaint_after(std::time::Duration::from_millis(100));
        resp
    }

    /// A submenu inside a context menu.
    pub fn menu_button<R>(
        &mut self,
        text: impl Into<RichText>,
        add: impl FnOnce(&mut Ui) -> R,
    ) -> Response {
        let text: RichText = text.into();
        let style = self.ctx.style();
        let (font, color) = text.resolve(&style, style.visuals.text);
        let galley = self
            .ctx
            .fonts_mut(|f| f.layout_no_wrap(&text.text, font.mono, font.size));
        let pad = style.button_padding;
        let size = vec2(
            (galley.size.x + pad.x * 2.0 + 16.0).max(self.available_width()),
            galley.size.y.max(style.interact_height),
        );
        let (rect, resp) = self.allocate_exact_size(size, Sense::click());
        let open = self
            .ctx
            .memory(|m| m.submenu.is_some_and(|(id, _)| id == resp.id));
        let v = style.visuals;
        let p = self.painter();
        if resp.hovered() || open {
            p.rect_filled(rect, 2.0, v.hovered_weak_bg);
        }
        p.galley(
            pos2(rect.left() + pad.x, rect.center().y - galley.size.y / 2.0),
            Rc::new(galley),
            color,
        );
        let tri = Rect::from_center_size(pos2(rect.right() - 8.0, rect.center().y), vec2(5.0, 8.0));
        p.add(Shape::convex_polygon(
            vec![tri.left_top(), tri.left_bottom(), tri.right_center()],
            color,
            Stroke::NONE,
        ));
        if resp.hovered() || resp.clicked() {
            let id = resp.id;
            self.ctx.memory_mut(|m| m.submenu = Some((id, rect)));
        }
        if open || resp.hovered() || resp.clicked() {
            let sub_id = resp.id.with("sub");
            let pos = rect.right_top() + vec2(2.0, 0.0);
            let shown = show_popup_frame(&self.ctx, sub_id, pos, 0.0, true, |ui| {
                add(ui);
                ui.min_rect()
            });
            self.ctx
                .memory_mut(|m| m.submenu_rect = Some(shown.expand(6.0)));
        }
        resp
    }
}

/// Paint a popup frame (menu, combo list) at `pos` on the foreground layer,
/// sized from the previous frame and kept on screen.
fn show_popup_frame<R>(
    ctx: &Context,
    id: Id,
    pos: Pos2,
    min_width: f32,
    menu: bool,
    add: impl FnOnce(&mut Ui) -> R,
) -> R {
    let style = ctx.style();
    let screen = ctx.screen_rect();
    let last = ctx
        .memory(|m| m.area_sizes.get(&id).copied())
        .unwrap_or(vec2(min_width.max(120.0), 40.0));
    let mut pos = pos;
    if pos.x + last.x > screen.right() - 2.0 {
        pos.x = (screen.right() - 2.0 - last.x).max(screen.left());
    }
    if pos.y + last.y > screen.bottom() - 2.0 {
        pos.y = (screen.bottom() - 2.0 - last.y).max(screen.top());
    }
    let margin = 4.0;
    let bg = ctx.add_shape(1, Rect::EVERYTHING, Shape::Noop);
    let inner_rect = Rect::from_min_size(
        pos + vec2(margin, margin),
        vec2(
            (last.x - 2.0 * margin)
                .max(min_width - 2.0 * margin)
                .max(60.0),
            screen.height(),
        ),
    );
    let mut ui = Ui::root(ctx, id, 1, inner_rect);
    ui.clip = screen;
    ui.menu = menu;
    ui.spacing.item_spacing.y = 1.0;
    let r = add(&mut ui);
    let content = ui.min_rect();
    let w = (content.width() + 2.0 * margin).max(min_width);
    let rect = Rect::from_min_size(pos, vec2(w, content.height().max(0.0) + 2.0 * margin));
    ctx.set_shape(
        1,
        bg,
        Shape::Rect {
            rect,
            rounding: 2.0,
            fill: style.visuals.window_fill,
            stroke: Stroke::new(1.0, style.visuals.window_stroke),
        },
    );
    ctx.register_popup(rect);
    ctx.memory_mut(|m| {
        let old = m.area_sizes.insert(id, rect.size());
        if old.is_none_or(|o| (o - rect.size()).length() > 0.5) {
            ctx.request_repaint();
        }
    });
    r
}

impl Response {
    /// A right-click menu for this widget.
    pub fn context_menu(&self, add: impl FnOnce(&mut Ui)) {
        if self.secondary_clicked() {
            let pos = self
                .ctx
                .input(|i| i.pointer.pos)
                .unwrap_or(self.rect.center());
            let frame = self.ctx.frame_nr();
            let id = self.id;
            self.ctx.memory_mut(|m| {
                m.popup = Some(Popup {
                    id,
                    kind: PopupKind::Context,
                    pos,
                    min_width: 0.0,
                    opened_frame: frame,
                });
                m.submenu = None;
            });
        }
        let open = self.ctx.memory(|m| {
            m.popup
                .filter(|p| p.id == self.id && p.kind == PopupKind::Context)
        });
        if let Some(p) = open {
            self.ctx.memory_mut(|m| m.popup_seen = true);
            show_popup_frame(
                &self.ctx,
                self.id.with("ctx"),
                p.pos,
                p.min_width,
                true,
                add,
            );
        }
    }
}

// --------------------------------------------------------------- RichText

#[derive(Clone, Debug, Default)]
pub struct RichText {
    pub text: String,
    size: Option<f32>,
    color: Option<Color32>,
    strong: bool,
    mono: bool,
    small: bool,
}

impl RichText {
    pub fn new(text: impl Into<String>) -> Self {
        RichText {
            text: text.into(),
            ..Default::default()
        }
    }
    pub fn size(mut self, s: f32) -> Self {
        self.size = Some(s);
        self
    }
    pub fn color(mut self, c: Color32) -> Self {
        self.color = Some(c);
        self
    }
    pub fn strong(mut self) -> Self {
        self.strong = true;
        self
    }
    pub fn monospace(mut self) -> Self {
        self.mono = true;
        self
    }
    pub fn small(mut self) -> Self {
        self.small = true;
        self
    }
    fn resolve(&self, style: &Style, default: Color32) -> (FontId, Color32) {
        let size = self.size.unwrap_or(if self.small {
            style.body_size * 0.8
        } else {
            style.body_size
        });
        let color = self.color.unwrap_or(if self.strong {
            style.visuals.strong_text
        } else {
            default
        });
        (
            FontId {
                size,
                mono: self.mono,
            },
            color,
        )
    }
}

impl From<&str> for RichText {
    fn from(s: &str) -> Self {
        RichText::new(s)
    }
}
impl From<String> for RichText {
    fn from(s: String) -> Self {
        RichText::new(s)
    }
}
impl From<&String> for RichText {
    fn from(s: &String) -> Self {
        RichText::new(s.clone())
    }
}

// ----------------------------------------------------------------- widgets

pub trait Widget {
    fn ui(self, ui: &mut Ui) -> Response;
}

pub struct Label {
    text: RichText,
    wrap: Option<bool>,
}

impl Label {
    pub fn new(text: impl Into<RichText>) -> Self {
        Label {
            text: text.into(),
            wrap: None,
        }
    }
    pub fn wrap(mut self) -> Self {
        self.wrap = Some(true);
        self
    }
}

impl Widget for Label {
    fn ui(self, ui: &mut Ui) -> Response {
        let style = ui.ctx.style();
        let (font, color) = self.text.resolve(&style, style.visuals.text);
        let wrap = self
            .wrap
            .unwrap_or(matches!(ui.dir, Dir::Vertical) && !ui.menu);
        let galley = if wrap {
            let w = ui.available_width().max(10.0);
            ui.ctx
                .fonts_mut(|f| f.layout_wrap(&self.text.text, font.mono, font.size, w))
        } else {
            ui.ctx
                .fonts_mut(|f| f.layout_no_wrap(&self.text.text, font.mono, font.size))
        };
        let (rect, resp) = ui.allocate_exact_size(galley.size, Sense::hover());
        let c = if ui.enabled {
            color
        } else {
            color.gamma_multiply(0.5)
        };
        ui.painter().galley(rect.min, Rc::new(galley), c);
        resp
    }
}

pub struct Button {
    text: RichText,
    small: bool,
}

impl Button {
    pub fn new(text: impl Into<RichText>) -> Self {
        Button {
            text: text.into(),
            small: false,
        }
    }
    pub fn small(mut self) -> Self {
        self.small = true;
        self
    }
}

impl Widget for Button {
    fn ui(self, ui: &mut Ui) -> Response {
        let style = ui.ctx.style();
        let mut text = self.text;
        if self.small {
            text.small = true;
        }
        let (font, color) = text.resolve(&style, style.visuals.text);
        let galley = ui
            .ctx
            .fonts_mut(|f| f.layout_no_wrap(&text.text, font.mono, font.size));
        let pad = if self.small {
            vec2(style.button_padding.x, 0.0)
        } else {
            style.button_padding
        };
        let mut size = galley.size + pad * 2.0;
        if !self.small {
            size.y = size.y.max(style.interact_height);
        }
        if ui.menu {
            size.x = size.x.max(ui.available_width());
        }
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
        let v = style.visuals;
        let p = ui.painter();
        if ui.menu {
            if resp.hovered() {
                p.rect_filled(rect, 2.0, v.hovered_weak_bg);
            }
        } else {
            let (fill, stroke) = if !ui.enabled {
                (v.inactive_weak_bg, v.inactive_stroke)
            } else if resp.is_pointer_button_down_on() {
                (v.active_bg, v.hovered_stroke)
            } else if resp.hovered() || resp.has_focus() {
                (v.hovered_weak_bg, v.hovered_stroke)
            } else {
                (v.inactive_weak_bg, v.inactive_stroke)
            };
            p.rect(rect, 2.0, fill, Stroke::new(1.0, stroke));
        }
        let c = if ui.enabled {
            color
        } else {
            color.gamma_multiply(0.45)
        };
        let x = if ui.menu {
            rect.left() + pad.x
        } else {
            rect.center().x - galley.size.x / 2.0
        };
        p.galley(
            pos2(x, rect.center().y - galley.size.y / 2.0),
            Rc::new(galley),
            c,
        );
        resp
    }
}

pub struct Checkbox<'a> {
    checked: &'a mut bool,
    text: RichText,
}

impl<'a> Checkbox<'a> {
    pub fn new(checked: &'a mut bool, text: impl Into<RichText>) -> Self {
        Checkbox {
            checked,
            text: text.into(),
        }
    }
}

impl Widget for Checkbox<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let style = ui.ctx.style();
        let (font, color) = self.text.resolve(&style, style.visuals.text);
        let galley = ui
            .ctx
            .fonts_mut(|f| f.layout_no_wrap(&self.text.text, font.mono, font.size));
        let icon = 14.0;
        let gap = 4.0;
        let size = vec2(
            icon + gap + galley.size.x,
            galley.size.y.max(style.interact_height),
        );
        let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
        if resp.clicked() || (resp.has_focus() && ui.input(|i| i.key_pressed(Key::Enter))) {
            *self.checked = !*self.checked;
            resp.changed = true;
        }
        let v = style.visuals;
        let p = ui.painter();
        let b = Rect::from_min_size(
            pos2(rect.left(), rect.center().y - icon / 2.0),
            vec2(icon, icon),
        );
        let (fill, stroke) = if resp.hovered() || resp.has_focus() {
            (v.hovered_bg, v.hovered_stroke)
        } else {
            (v.inactive_bg, v.inactive_stroke)
        };
        let alpha = if ui.enabled { 1.0 } else { 0.45 };
        p.rect(
            b,
            2.0,
            fill.gamma_multiply(alpha),
            Stroke::new(1.0, stroke.gamma_multiply(alpha)),
        );
        if *self.checked {
            let s = b.shrink(3.0);
            p.add(Shape::line(
                vec![
                    pos2(s.left(), s.center().y),
                    pos2(s.left() + s.width() * 0.38, s.bottom()),
                    pos2(s.right(), s.top()),
                ],
                Stroke::new(2.0, v.strong_text.gamma_multiply(alpha)),
            ));
        }
        p.galley(
            pos2(b.right() + gap, rect.center().y - galley.size.y / 2.0),
            Rc::new(galley),
            color.gamma_multiply(alpha),
        );
        resp
    }
}

pub struct Slider<'a> {
    value: &'a mut f32,
    range: std::ops::RangeInclusive<f32>,
    step: f32,
    decimals: usize,
}

impl<'a> Slider<'a> {
    pub fn new(value: &'a mut f32, range: std::ops::RangeInclusive<f32>) -> Self {
        Slider {
            value,
            range,
            step: 0.0,
            decimals: 2,
        }
    }
    pub fn step_by(mut self, s: f64) -> Self {
        self.step = s as f32;
        self
    }
    pub fn fixed_decimals(mut self, d: usize) -> Self {
        self.decimals = d;
        self
    }
}

impl Widget for Slider<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let style = ui.ctx.style();
        let v = style.visuals;
        let h = style.interact_height;
        let (rect, mut resp) = ui.allocate_exact_size(vec2(100.0, h), Sense::click_and_drag());
        let (lo, hi) = (*self.range.start(), *self.range.end());
        let rail = Rect::from_center_size(rect.center(), vec2(rect.width() - h, 4.0));
        if resp.dragged() || resp.clicked() {
            if let Some(p) = ui.input(|i| i.pointer.pos) {
                let t = ((p.x - rail.left()) / rail.width().max(1.0)).clamp(0.0, 1.0);
                let mut nv = lo + t * (hi - lo);
                if self.step > 0.0 {
                    nv = (nv / self.step).round() * self.step;
                }
                nv = nv.clamp(lo, hi);
                if (nv - *self.value).abs() > f32::EPSILON {
                    *self.value = nv;
                    resp.changed = true;
                }
            }
        }
        let t = ((*self.value - lo) / (hi - lo).max(1e-6)).clamp(0.0, 1.0);
        let p = ui.painter();
        p.rect_filled(rail, 2.0, v.inactive_bg);
        p.rect_filled(
            Rect::from_min_max(
                rail.min,
                pos2(rail.left() + rail.width() * t, rail.bottom()),
            ),
            2.0,
            v.selection_bg,
        );
        let c = pos2(rail.left() + rail.width() * t, rect.center().y);
        let hover = resp.hovered() || resp.dragged();
        p.add(Shape::Circle {
            center: c,
            radius: h * 0.4,
            fill: if hover { v.hovered_bg } else { v.inactive_bg },
            stroke: Stroke::new(
                1.0,
                if hover {
                    v.hovered_stroke
                } else {
                    v.inactive_stroke
                },
            ),
        });
        let text = format!("{:.*}", self.decimals, *self.value);
        let galley = ui
            .ctx
            .fonts_mut(|f| f.layout_no_wrap(&text, false, style.body_size));
        let (tr, _) = ui.allocate_exact_size(galley.size + vec2(8.0, 2.0), Sense::hover());
        ui.painter()
            .rect(tr, 2.0, v.extreme_bg, Stroke::new(1.0, v.inactive_stroke));
        ui.painter()
            .galley(tr.min + vec2(4.0, 1.0), Rc::new(galley), v.text);
        resp
    }
}

// --------------------------------------------------------------- TextEdit

pub struct TextEdit<'a> {
    text: &'a mut String,
    hint: String,
    frame: bool,
    width: Option<f32>,
    password: bool,
}

impl<'a> TextEdit<'a> {
    pub fn singleline(text: &'a mut String) -> Self {
        TextEdit {
            text,
            hint: String::new(),
            frame: true,
            width: None,
            password: false,
        }
    }
    pub fn hint_text(mut self, h: impl Into<String>) -> Self {
        self.hint = h.into();
        self
    }
    pub fn frame(mut self, f: bool) -> Self {
        self.frame = f;
        self
    }
    pub fn desired_width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }
    pub fn password(mut self, p: bool) -> Self {
        self.password = p;
        self
    }
}

fn byte_at(s: &str, chars: usize) -> usize {
    s.char_indices()
        .nth(chars)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

impl Widget for TextEdit<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let style = ui.ctx.style();
        let v = style.visuals;
        let size = style.body_size;
        let row = ui.ctx.fonts_mut(|f| f.row_height(false, size));
        let margin = vec2(4.0, 2.0);
        let w = self
            .width
            .unwrap_or(280.0)
            .min(ui.available_width())
            .max(24.0);
        let rect = ui.allocate_space(vec2(w, row + margin.y * 2.0));
        let id = ui.next_id();
        let clipped = rect.intersect(ui.clip);
        let mut resp = ui.ctx.interact(
            clipped,
            id,
            Sense::click_and_drag(),
            ui.layer,
            ui.enabled,
            true,
        );
        resp.rect = rect;
        if resp.hovered() {
            ui.ctx.set_cursor_icon(CursorIcon::Text);
        }
        let focused = resp.has_focus();
        let mut st = ui
            .ctx
            .memory(|m| m.text.get(&id).cloned())
            .unwrap_or_default();
        let len = self.text.chars().count();
        st.cursor = st.cursor.min(len);
        let shown = |t: &str| -> String {
            if self.password {
                "•".repeat(t.chars().count())
            } else {
                t.to_string()
            }
        };
        if resp.clicked() || resp.dragged() {
            if let Some(p) = ui.input(|i| i.pointer.pos) {
                let x = p.x - (rect.left() + margin.x) + st.scroll_x;
                let s = shown(self.text);
                let mut acc = 0.0;
                let mut idx = 0;
                ui.ctx.fonts_mut(|f| {
                    for c in s.chars() {
                        let a = f.width(&c.to_string(), false, size);
                        if acc + a / 2.0 > x {
                            break;
                        }
                        acc += a;
                        idx += 1;
                    }
                });
                st.cursor = idx;
                st.select_all = false;
            }
        }
        if focused {
            let events = ui.input(|i| i.events.clone());
            let mut changed = false;
            for e in events {
                match e {
                    Event::Text(t) | Event::Paste(t) => {
                        let t: String = t.chars().filter(|c| !c.is_control()).collect();
                        if t.is_empty() {
                            continue;
                        }
                        if st.select_all {
                            self.text.clear();
                            st.cursor = 0;
                            st.select_all = false;
                        }
                        let b = byte_at(self.text, st.cursor);
                        self.text.insert_str(b, &t);
                        st.cursor += t.chars().count();
                        changed = true;
                    }
                    Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                    } => match key {
                        Key::Backspace => {
                            if st.select_all {
                                self.text.clear();
                                st.cursor = 0;
                                st.select_all = false;
                                changed = true;
                            } else if st.cursor > 0 {
                                if modifiers.ctrl {
                                    let chars: Vec<char> = self.text.chars().collect();
                                    let mut i = st.cursor;
                                    while i > 0 && chars[i - 1] == ' ' {
                                        i -= 1;
                                    }
                                    while i > 0 && chars[i - 1] != ' ' {
                                        i -= 1;
                                    }
                                    let (a, b) =
                                        (byte_at(self.text, i), byte_at(self.text, st.cursor));
                                    self.text.replace_range(a..b, "");
                                    st.cursor = i;
                                } else {
                                    let (a, b) = (
                                        byte_at(self.text, st.cursor - 1),
                                        byte_at(self.text, st.cursor),
                                    );
                                    self.text.replace_range(a..b, "");
                                    st.cursor -= 1;
                                }
                                changed = true;
                            }
                        }
                        Key::Delete => {
                            if st.select_all {
                                self.text.clear();
                                st.cursor = 0;
                                st.select_all = false;
                                changed = true;
                            } else if st.cursor < self.text.chars().count() {
                                let (a, b) = (
                                    byte_at(self.text, st.cursor),
                                    byte_at(self.text, st.cursor + 1),
                                );
                                self.text.replace_range(a..b, "");
                                changed = true;
                            }
                        }
                        Key::ArrowLeft => {
                            st.select_all = false;
                            st.cursor = st.cursor.saturating_sub(1);
                        }
                        Key::ArrowRight => {
                            st.select_all = false;
                            st.cursor = (st.cursor + 1).min(self.text.chars().count());
                        }
                        Key::Home => {
                            st.select_all = false;
                            st.cursor = 0;
                        }
                        Key::End => {
                            st.select_all = false;
                            st.cursor = self.text.chars().count();
                        }
                        Key::A if modifiers.ctrl => st.select_all = true,
                        Key::U if modifiers.ctrl => {
                            self.text.clear();
                            st.cursor = 0;
                            changed = true;
                        }
                        Key::V if modifiers.ctrl => {
                            ui.ctx.send_viewport_cmd(ViewportCommand::Paste);
                        }
                        Key::C if modifiers.ctrl && !self.password => {
                            ui.ctx
                                .send_viewport_cmd(ViewportCommand::Copy(self.text.clone()));
                        }
                        Key::Enter => {
                            ui.ctx.memory_mut(|m| m.focus = None);
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
            if changed {
                resp.changed = true;
            }
        }
        let p = ui.painter();
        if self.frame {
            let stroke = if focused {
                v.strong_text
            } else if resp.hovered() {
                v.hovered_stroke
            } else {
                v.inactive_stroke
            };
            p.rect(rect, 2.0, v.extreme_bg, Stroke::new(1.0, stroke));
        }
        let inner = rect.shrink2(margin);
        let tp = p.with_clip_rect(inner.expand(1.0));
        let display = shown(self.text);
        let before: String = display.chars().take(st.cursor).collect();
        let caret_x = ui.ctx.fonts_mut(|f| f.width(&before, false, size));
        if caret_x - st.scroll_x > inner.width() - 2.0 {
            st.scroll_x = caret_x - inner.width() + 2.0;
        }
        if caret_x < st.scroll_x {
            st.scroll_x = caret_x;
        }
        let origin = pos2(inner.left() - st.scroll_x, inner.center().y - row / 2.0);
        if self.text.is_empty() {
            if !self.hint.is_empty() {
                let g = ui
                    .ctx
                    .fonts_mut(|f| f.layout_no_wrap(&self.hint, false, size));
                tp.galley(pos2(inner.left(), origin.y), Rc::new(g), v.weak_text);
            }
        } else {
            if focused && st.select_all {
                let w = ui.ctx.fonts_mut(|f| f.width(&display, false, size));
                tp.rect_filled(
                    Rect::from_min_size(origin, vec2(w, row)),
                    0.0,
                    v.selection_bg,
                );
            }
            let g = ui
                .ctx
                .fonts_mut(|f| f.layout_no_wrap(&display, false, size));
            tp.galley(origin, Rc::new(g), v.text);
        }
        if focused {
            let x = (origin.x + caret_x).round() + 0.5;
            tp.line_segment(
                [pos2(x, origin.y + 1.0), pos2(x, origin.y + row - 1.0)],
                Stroke::new(1.5, v.strong_text),
            );
        }
        ui.ctx.memory_mut(|m| {
            m.text.insert(id, st);
        });
        resp
    }
}

// --------------------------------------------------------------- ComboBox

pub struct ComboBox {
    id: Id,
    selected: String,
    width: Option<f32>,
}

impl ComboBox {
    pub fn from_id_salt(salt: impl Hash) -> Self {
        ComboBox {
            id: Id::new(("combo", salt)),
            selected: String::new(),
            width: None,
        }
    }
    pub fn selected_text(mut self, t: impl Into<String>) -> Self {
        self.selected = t.into();
        self
    }
    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }
    pub fn show_ui<R>(
        self,
        ui: &mut Ui,
        add: impl FnOnce(&mut Ui) -> R,
    ) -> InnerResponse<Option<R>> {
        let style = ui.ctx.style();
        let v = style.visuals;
        let galley = ui
            .ctx
            .fonts_mut(|f| f.layout_no_wrap(&self.selected, false, style.body_size));
        let pad = style.button_padding;
        let w = self
            .width
            .unwrap_or((galley.size.x + pad.x * 2.0 + 16.0).max(100.0));
        let size = vec2(w, galley.size.y.max(style.interact_height));
        let rect = ui.allocate_space(size);
        let resp = ui.interact(rect, self.id, Sense::click());
        let open = ui.ctx.memory(|m| m.popup.is_some_and(|p| p.id == self.id));
        if resp.clicked() || (resp.has_focus() && ui.input(|i| i.key_pressed(Key::Enter))) {
            let frame = ui.ctx.frame_nr();
            let id = self.id;
            ui.ctx.memory_mut(|m| {
                if open {
                    m.close_popup();
                } else {
                    m.popup = Some(Popup {
                        id,
                        kind: PopupKind::Combo,
                        pos: rect.left_bottom() + vec2(0.0, 2.0),
                        min_width: rect.width(),
                        opened_frame: frame,
                    });
                }
            });
        }
        let p = ui.painter();
        let (fill, stroke) = if resp.hovered() || open || resp.has_focus() {
            (v.hovered_weak_bg, v.hovered_stroke)
        } else {
            (v.inactive_weak_bg, v.inactive_stroke)
        };
        p.rect(rect, 2.0, fill, Stroke::new(1.0, stroke));
        let tp = p.with_clip_rect(rect.shrink(1.0));
        tp.galley(
            pos2(rect.left() + pad.x, rect.center().y - galley.size.y / 2.0),
            Rc::new(galley),
            v.text,
        );
        let tri = Rect::from_center_size(pos2(rect.right() - 9.0, rect.center().y), vec2(8.0, 5.0));
        p.add(Shape::convex_polygon(
            vec![tri.left_top(), tri.right_top(), tri.center_bottom()],
            v.text,
            Stroke::NONE,
        ));
        let open = ui.ctx.memory(|m| {
            m.popup
                .filter(|p| p.id == self.id && p.kind == PopupKind::Combo)
        });
        if open.is_some() {
            ui.ctx.memory_mut(|m| m.popup_seen = true);
        }
        let inner = open.map(|pp| {
            show_popup_frame(
                &ui.ctx,
                self.id.with("list"),
                pp.pos,
                pp.min_width,
                true,
                add,
            )
        });
        InnerResponse {
            inner,
            response: resp,
        }
    }
}

// ------------------------------------------------------------------ Frame

#[derive(Clone, Copy, Debug, Default)]
pub struct Frame {
    fill: Color32,
    stroke: Stroke,
    margin: Margin,
    rounding: f32,
}

impl Frame {
    pub fn none() -> Self {
        Frame::default()
    }
    pub fn popup(style: &Style) -> Self {
        Frame {
            fill: style.visuals.window_fill,
            stroke: Stroke::new(1.0, style.visuals.window_stroke),
            margin: Margin::same(6.0),
            rounding: 2.0,
        }
    }
    pub fn fill(mut self, c: Color32) -> Self {
        self.fill = c;
        self
    }
    pub fn stroke(mut self, s: Stroke) -> Self {
        self.stroke = s;
        self
    }
    pub fn inner_margin(mut self, m: Margin) -> Self {
        self.margin = m;
        self
    }
    pub fn rounding(mut self, r: f32) -> Self {
        self.rounding = r;
        self
    }

    pub fn show<R>(self, ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
        let bg = ui.painter().add(Shape::Noop);
        let m = self.margin;
        let start = if matches!(ui.dir, Dir::Vertical | Dir::Centered) {
            pos2(ui.max_rect.left(), ui.cursor.y)
        } else {
            ui.cursor
        };
        let inner_max = Rect::from_min_max(
            start + vec2(m.left, m.top),
            pos2(
                (ui.max_rect.right() - m.right).max(start.x + m.left),
                (ui.max_rect.bottom() - m.bottom).max(start.y + m.top),
            ),
        );
        let mut child = ui.child(inner_max, Dir::Vertical);
        let inner = add(&mut child);
        let c = child.min_rect();
        let c = if c.width() < 0.0 {
            Rect::from_min_size(inner_max.min, Vec2::ZERO)
        } else {
            c
        };
        let outer = Rect::from_min_max(
            pos2(
                c.min.x.min(inner_max.min.x) - m.left,
                c.min.y.min(inner_max.min.y) - m.top,
            ),
            c.max + vec2(m.right, m.bottom),
        );
        ui.painter().set(
            bg,
            Shape::Rect {
                rect: outer,
                rounding: self.rounding,
                fill: self.fill,
                stroke: self.stroke,
            },
        );
        let response = ui.allocate_rect(outer, Sense::hover());
        InnerResponse { inner, response }
    }
}

pub struct CentralPanel {
    frame: Frame,
}

impl Default for CentralPanel {
    fn default() -> Self {
        CentralPanel {
            frame: Frame::none(),
        }
    }
}

impl CentralPanel {
    pub fn frame(mut self, f: Frame) -> Self {
        self.frame = f;
        self
    }
    pub fn show<R>(self, ctx: &Context, add: impl FnOnce(&mut Ui) -> R) -> R {
        let screen = ctx.screen_rect();
        let painter = ctx.layer_painter(LayerId::new(Order::Background, Id::new("central")));
        painter.rect_filled(screen, 0.0, self.frame.fill);
        let m = self.frame.margin;
        let rect = Rect::from_min_max(
            screen.min + vec2(m.left, m.top),
            screen.max - vec2(m.right, m.bottom),
        );
        let mut ui = Ui::root(ctx, Id::new("central"), 0, rect);
        ui.clip = screen;
        add(&mut ui)
    }
}

/// A floating area on the foreground layer (dialogs).
pub struct Area {
    id: Id,
    anchor: Option<(Align2, Vec2)>,
    pos: Option<Pos2>,
}

impl Area {
    pub fn new(id: Id) -> Self {
        Area {
            id,
            anchor: None,
            pos: None,
        }
    }
    pub fn order(self, _o: Order) -> Self {
        self
    }
    pub fn anchor(mut self, a: Align2, off: Vec2) -> Self {
        self.anchor = Some((a, off));
        self
    }
    pub fn fixed_pos(mut self, p: Pos2) -> Self {
        self.pos = Some(p);
        self
    }
    pub fn show<R>(self, ctx: &Context, add: impl FnOnce(&mut Ui) -> R) -> R {
        let screen = ctx.screen_rect();
        let size = ctx
            .memory(|m| m.area_sizes.get(&self.id).copied())
            .unwrap_or(Vec2::ZERO);
        let pos = if let Some((a, off)) = self.anchor {
            let anchor_pos = match a.0 {
                [Align::Center, Align::Center] => screen.center(),
                _ => screen.min,
            };
            a.anchor_size(anchor_pos + off, size).min
        } else {
            self.pos.unwrap_or(screen.min)
        };
        let rect = Rect::from_min_max(pos, screen.max.max_with(pos + vec2(1.0, 1.0)));
        let mut ui = Ui::root(ctx, self.id, 1, rect);
        ui.clip = screen;
        let r = add(&mut ui);
        let content = ui.min_rect();
        let used = Rect::from_min_max(pos, content.max.max_with(pos));
        ctx.register_popup(used);
        ctx.memory_mut(|m| {
            let old = m.area_sizes.insert(self.id, used.size());
            if old.is_none_or(|o| (o - used.size()).length() > 0.5) {
                ctx.request_repaint();
            }
        });
        r
    }
}

impl Pos2 {
    fn max_with(self, o: Pos2) -> Pos2 {
        pos2(self.x.max(o.x), self.y.max(o.y))
    }
}

// ------------------------------------------------------------- ScrollArea

pub struct ScrollArea {
    salt: Option<Id>,
}

impl ScrollArea {
    pub fn vertical() -> Self {
        ScrollArea { salt: None }
    }
    pub fn auto_shrink(self, _a: [bool; 2]) -> Self {
        self
    }
    pub fn id_salt(mut self, s: impl Hash) -> Self {
        self.salt = Some(Id::new(("scroll", s)));
        self
    }

    fn begin(&self, ui: &mut Ui) -> (Id, Rect, f32) {
        let id = self.salt.unwrap_or_else(|| {
            ui.counter += 1;
            ui.id.with(("scroll", ui.counter))
        });
        let outer = Rect::from_min_max(
            pos2(
                ui.max_rect.left().max(ui.cursor.x.min(ui.max_rect.left())),
                ui.cursor.y,
            ),
            ui.max_rect.max,
        );
        let mut offset = ui.ctx.memory(|m| m.scroll.get(&id).copied()).unwrap_or(0.0);
        if let Some((top, bottom)) = ui.ctx.memory_mut(|m| m.scroll_request.remove(&id)) {
            let h = outer.height();
            if top < offset || bottom > offset + h {
                offset = ((top + bottom) / 2.0 - h / 2.0).max(0.0);
            }
        }
        let (pos, wheel, top_layer) = ui.ctx.input(|i| (i.pointer.pos, i.scroll_delta.y, 0usize));
        let _ = top_layer;
        let on_top = ui.ctx.memory(|m| m.top_layer) <= ui.layer;
        if wheel != 0.0 && on_top && pos.is_some_and(|p| outer.intersect(ui.clip).contains(p)) {
            offset -= wheel;
        }
        (id, outer, offset.max(0.0))
    }

    fn finish(ui: &mut Ui, id: Id, outer: Rect, mut offset: f32, content_h: f32) {
        let h = outer.height();
        let max = (content_h - h).max(0.0);
        if offset > max {
            offset = max;
        }
        if content_h > h + 0.5 && h > 8.0 {
            let v = ui.ctx.style().visuals;
            let bar = Rect::from_min_max(pos2(outer.right() - 8.0, outer.top()), outer.max);
            let resp = ui.interact(bar, id.with("bar"), Sense::drag());
            let thumb_h = (h * h / content_h).max(14.0).min(h);
            if resp.drag_started() {
                let y = ui.input(|i| i.pointer.pos.map(|p| p.y)).unwrap_or(0.0);
                ui.ctx
                    .memory_mut(|m| m.scrollbar_drag = Some((id, y, offset)));
            }
            if resp.dragged() {
                if let (Some((did, y0, o0)), Some(y)) = (
                    ui.ctx.memory(|m| m.scrollbar_drag),
                    ui.input(|i| i.pointer.pos.map(|p| p.y)),
                ) {
                    if did == id {
                        let travel = (h - thumb_h).max(1.0);
                        offset = (o0 + (y - y0) / travel * max).clamp(0.0, max);
                    }
                }
            }
            let t = if max > 0.0 { offset / max } else { 0.0 };
            let wide = resp.hovered() || resp.dragged();
            let w = if wide { 8.0 } else { 4.0 };
            let thumb = Rect::from_min_size(
                pos2(outer.right() - w - 1.0, outer.top() + (h - thumb_h) * t),
                vec2(w, thumb_h),
            );
            let p = ui.painter();
            if wide {
                p.rect_filled(
                    Rect::from_min_max(pos2(outer.right() - w - 1.0, outer.top()), outer.max),
                    2.0,
                    v.extreme_bg.gamma_multiply(0.6),
                );
            }
            p.rect_filled(
                thumb,
                2.0,
                if wide {
                    v.hovered_stroke
                } else {
                    v.inactive_stroke
                },
            );
        }
        let old = ui.ctx.memory_mut(|m| m.scroll.insert(id, offset));
        if old.is_some_and(|o| (o - offset).abs() > 0.1) {
            ui.ctx.request_repaint();
        }
        ui.allocate_rect(outer, Sense::hover());
    }

    pub fn show<R>(self, ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
        let (id, outer, offset) = self.begin(ui);
        let content_rect = Rect::from_min_max(
            pos2(outer.left(), outer.top() - offset),
            pos2(outer.right() - 2.0, outer.top() - offset + 1e6),
        );
        let mut child = ui.child(content_rect, Dir::Vertical);
        child.clip = outer.intersect(ui.clip);
        child.scroll = Some((id, outer.top() - offset));
        let r = add(&mut child);
        let c = child.min_rect();
        let content_h = if c.height() >= 0.0 {
            c.bottom() - content_rect.top()
        } else {
            0.0
        };
        Self::finish(ui, id, outer, offset, content_h);
        r
    }

    pub fn show_rows<R>(
        self,
        ui: &mut Ui,
        row_h: f32,
        total: usize,
        add: impl FnOnce(&mut Ui, Range<usize>) -> R,
    ) -> R {
        let (id, outer, offset) = self.begin(ui);
        let pitch = row_h + ui.spacing.item_spacing.y;
        let content_h = (total as f32 * pitch - ui.spacing.item_spacing.y).max(0.0);
        let max = (content_h - outer.height()).max(0.0);
        let offset = offset.min(max);
        let first = ((offset / pitch).floor() as usize).min(total);
        let last = (((offset + outer.height()) / pitch).ceil() as usize + 1).min(total);
        let top = outer.top() - offset + first as f32 * pitch;
        let content_rect = Rect::from_min_max(
            pos2(outer.left(), top),
            pos2(outer.right() - 2.0, top + 1e6),
        );
        let mut child = ui.child(content_rect, Dir::Vertical);
        child.clip = outer.intersect(ui.clip);
        child.scroll = Some((id, outer.top() - offset));
        let r = add(&mut child, first..last);
        Self::finish(ui, id, outer, offset, content_h);
        r
    }
}
