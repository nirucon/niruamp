//! The player deck: digital clock, artwork, spectrum and transport controls.
use crate::app::{NiruampApp, Tab};
use crate::gui::{pos2, vec2, Align2, Color32, Context, FontId, Id, Rect, Sense, Stroke, Ui};
use crate::skin::{self, Skin};
use crate::views::common::*;
use std::hash::{Hash, Hasher};

impl NiruampApp {
    pub(crate) fn deck_ui(&mut self, ui: &mut Ui, ctx: &Context) {
        let p = self.palette();
        let scale = self.prefs.scale;
        let s = move |v: f32| v * scale;
        let margin = s(7.0);
        let gap = s(5.0);
        if self.prefs.window_shade {
            self.shade_ui(ui, ctx);
            return;
        }
        let title_h = s(21.0);
        let display_h = s(102.0);
        let seek_h = s(15.0);
        let transport_h = s(33.0);
        let lower_h = s(27.0);
        let deck_h = margin * 2.0
            + title_h
            + gap
            + display_h
            + gap
            + seek_h
            + gap
            + transport_h
            + gap
            + lower_h;
        let width = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(vec2(width, deck_h), Sense::hover());
        let skin = self.skin();
        self.draw_deck_frame(ui, rect, skin);

        let inner = rect.shrink(margin);
        let title = Rect::from_min_size(inner.min, vec2(inner.width(), title_h));
        let display = Rect::from_min_size(
            pos2(inner.left(), title.bottom() + gap),
            vec2(inner.width(), display_h),
        );
        let seek = Rect::from_min_size(
            pos2(inner.left(), display.bottom() + gap),
            vec2(inner.width(), seek_h),
        );
        let transport = Rect::from_min_size(
            pos2(inner.left(), seek.bottom() + gap),
            vec2(inner.width(), transport_h),
        );
        let lower = Rect::from_min_size(
            pos2(inner.left(), transport.bottom() + gap),
            vec2(inner.width(), lower_h),
        );

        if skin == Skin::HiFi {
            // The controls sit on a dark strip below the brushed faceplate.
            let strip = Rect::from_min_max(
                pos2(rect.left() + s(3.0), seek.top() - gap * 0.5),
                pos2(rect.right() - s(3.0), rect.bottom() - s(3.0)),
            );
            let painter = ui.painter();
            painter.rect_filled(strip, crate::gui::Rounding::same(3.0), p.background);
            painter.line_segment(
                [strip.left_top(), strip.right_top()],
                Stroke::new(1.0_f32, p.shadow),
            );
            painter.line_segment(
                [strip.left_bottom(), strip.right_bottom()],
                Stroke::new(1.0_f32, skin::alpha(Color32::WHITE, 0.35)),
            );
        }
        self.draw_title_bar(ui, ctx, title);
        match skin {
            Skin::HiFi => self.draw_hifi_display(ui, display),
            _ => self.draw_display(ui, display),
        }
        self.draw_seek(ui, seek);
        self.draw_transport(ui, ctx, transport);
        self.draw_lower(ui, ctx, lower);
        self.deck_height = deck_h;
    }

    /// The deck's housing for each skin.
    fn draw_deck_frame(&mut self, ui: &mut Ui, rect: Rect, skin: Skin) {
        let p = self.palette();
        let ctx = ui.ctx().clone();
        let painter = ui.painter();
        let panel = rect.shrink(0.5);
        let r = crate::gui::Rounding::same(3.0);
        match skin {
            Skin::Classic => {
                let top = skin::mix(p.surface, p.surface_hi, 0.55);
                let bottom = skin::mix(p.surface, p.background, 0.35);
                let tex = self.skin_art.gradient(&ctx, top, bottom);
                painter.image(tex.id(), panel, Rect::NOTHING, Color32::WHITE);
            }
            Skin::Phosphor => {
                let top = p.surface_hi;
                let bottom = p.surface;
                let tex = self.skin_art.gradient(&ctx, top, bottom);
                painter.image(tex.id(), panel, Rect::NOTHING, Color32::WHITE);
            }
            Skin::HiFi => {
                let tex = self
                    .skin_art
                    .brushed(&ctx, skin::metal(&self.theme.palette));
                painter.image(tex.id(), panel, Rect::NOTHING, Color32::WHITE);
            }
        }
        painter.rect_stroke(panel, r, Stroke::new(1.0_f32, p.border));
        let (hi, lo) = if skin == Skin::HiFi {
            (
                skin::alpha(Color32::WHITE, 0.55),
                skin::alpha(Color32::BLACK, 0.55),
            )
        } else {
            (p.border_hi, p.shadow)
        };
        painter.line_segment(
            [panel.left_top(), panel.right_top()],
            Stroke::new(1.0_f32, hi),
        );
        painter.line_segment(
            [panel.left_top(), panel.left_bottom()],
            Stroke::new(1.0_f32, hi),
        );
        painter.line_segment(
            [panel.right_top(), panel.right_bottom()],
            Stroke::new(1.0_f32, lo),
        );
        painter.line_segment(
            [panel.left_bottom(), panel.right_bottom()],
            Stroke::new(1.0_f32, lo),
        );
        if skin != Skin::Classic {
            // Panel screws in the corners.
            let d = self.s(4.5);
            for c in [
                panel.left_top() + vec2(d, d),
                panel.right_top() + vec2(-d, d),
            ] {
                let (head, slot) = if skin == Skin::HiFi {
                    (
                        Color32::from_rgb(150, 152, 155),
                        Color32::from_rgb(70, 72, 76),
                    )
                } else {
                    (p.border_hi, p.inset)
                };
                painter.circle_filled(c, self.s(2.2), head);
                painter.line_segment(
                    [c + vec2(-1.4, -1.0), c + vec2(1.4, 1.0)],
                    Stroke::new(1.0_f32, slot),
                );
            }
        }
    }

    fn draw_title_bar(&mut self, ui: &mut Ui, ctx: &Context, rect: Rect) {
        let p = self.palette();
        let skin = self.skin();
        let scale = self.prefs.scale;
        let s = move |v: f32| v * scale;
        let btn = s(17.0);
        let buttons_left =
            rect.right() - if skin == Skin::Classic { 0.0 } else { s(8.0) } - 6.0 * (btn + s(2.0));
        // Labels never run under the title buttons in a narrow window.
        let painter = &ui.painter().with_clip_rect(Rect::from_min_max(
            rect.min,
            pos2(buttons_left - s(4.0), rect.max.y),
        ));
        let left = rect.left() + if skin == Skin::Classic { 2.0 } else { s(9.0) };
        let logo_rect =
            Rect::from_min_size(pos2(left, rect.center().y - s(8.0)), vec2(s(16.0), s(16.0)));
        logo(painter, logo_rect, &p);
        let name_x = logo_rect.right() + s(6.0);
        let cy = rect.center().y - s(1.0);
        let online = self.backend.is_some();
        let status_color = if online {
            if self.playback.is_playing() {
                p.success
            } else {
                p.muted
            }
        } else {
            p.error
        };
        let version = format!("v{}", env!("CARGO_PKG_VERSION"));
        match skin {
            Skin::Classic => {
                let name = painter.text(
                    pos2(name_x, cy),
                    Align2::LEFT_CENTER,
                    "NIRUAMP",
                    FontId::proportional(s(12.0)),
                    p.foreground,
                );
                let v = painter.text(
                    pos2(name.right() + s(6.0), cy + s(1.0)),
                    Align2::LEFT_CENTER,
                    version,
                    FontId::proportional(s(8.0)),
                    p.disabled,
                );
                let dot = pos2(v.right() + s(12.0), rect.center().y);
                painter.circle_filled(dot, s(2.6), status_color);
                painter.text(
                    pos2(dot.x + s(7.0), cy),
                    Align2::LEFT_CENTER,
                    if online { "NPLAY" } else { "OFFLINE" },
                    FontId::proportional(s(8.0)),
                    p.muted,
                );
            }
            Skin::Phosphor => {
                let name = skin::glow_text(
                    painter,
                    pos2(name_x, cy),
                    Align2::LEFT_CENTER,
                    "NIRUAMP",
                    FontId::monospace(s(12.0)),
                    p.led,
                );
                let tag = painter.text(
                    pos2(name.right() + s(8.0), cy + s(1.0)),
                    Align2::LEFT_CENTER,
                    format!("OPERATOR CONSOLE {version}"),
                    FontId::monospace(s(8.0)),
                    p.muted,
                );
                painter.text(
                    pos2(tag.right() + s(10.0), cy + s(1.0)),
                    Align2::LEFT_CENTER,
                    if online { "[LINK OK]" } else { "[NO LINK]" },
                    FontId::monospace(s(8.0)),
                    if online { p.foreground } else { p.error },
                );
            }
            Skin::HiFi => {
                let ink = Color32::from_rgb(34, 34, 38);
                let name = skin::engraved(
                    painter,
                    pos2(name_x, cy),
                    Align2::LEFT_CENTER,
                    "NIRUAMP",
                    FontId::proportional(s(13.0)),
                    ink,
                );
                let tag = skin::engraved(
                    painter,
                    pos2(name.right() + s(10.0), cy + s(1.0)),
                    Align2::LEFT_CENTER,
                    "STEREO CASSETTE DECK",
                    FontId::proportional(s(7.5)),
                    skin::mix(ink, Color32::WHITE, 0.25),
                );
                // Power lamp.
                let lamp = pos2(tag.right() + s(14.0), rect.center().y);
                painter.circle_filled(lamp, s(3.4), Color32::from_rgb(40, 40, 44));
                painter.circle_filled(lamp, s(2.4), if online { p.accent } else { p.error });
                skin::engraved(
                    painter,
                    pos2(lamp.x + s(7.0), cy + s(1.0)),
                    Align2::LEFT_CENTER,
                    "POWER",
                    FontId::proportional(s(7.0)),
                    skin::mix(ink, Color32::WHITE, 0.25),
                );
            }
        }

        let mut x = rect.right() - btn - if skin == Skin::Classic { 0.0 } else { s(8.0) };
        let actions = [
            (Icon::Close, "Close NIRUAMP"),
            (Icon::Minimize, "Minimize"),
            (Icon::Up, "Windowshade (W)"),
            (Icon::Pin, "Always on top (T)"),
            (Icon::Fullscreen, "Fullscreen visual (F11)"),
            (Icon::Settings, "Settings (4)"),
        ];
        for (icon, tip) in actions {
            let r = Rect::from_center_size(pos2(x + btn / 2.0, rect.center().y), vec2(btn, btn));
            let active = matches!(icon, Icon::Pin) && self.prefs.always_on_top;
            let resp = icon_button_rect(
                ui,
                Id::new(("deck-title", icon as u8)),
                r,
                &p,
                icon,
                active,
                true,
                tip,
            );
            if resp.activated(ui) {
                match icon {
                    Icon::Close => ctx.send_viewport_cmd(crate::gui::ViewportCommand::Close),
                    Icon::Minimize => self.hide_to_tray(ctx),
                    Icon::Up => self.toggle_shade(ctx),
                    Icon::Pin => self.toggle_pin(ctx),
                    Icon::Fullscreen => self.toggle_immersive(ctx),
                    Icon::Settings => self.show_tab(Tab::Settings, ctx),
                    _ => {}
                }
            }
            x -= btn + s(2.0);
        }
        let drag_rect = Rect::from_min_max(rect.min, pos2(x + btn - s(2.0), rect.max.y));
        let drag = ui.interact(
            drag_rect,
            Id::new("deck-title-drag"),
            Sense::click_and_drag(),
        );
        if drag.drag_started() {
            self.start_drag(ctx);
        }
        if drag.double_clicked() {
            self.toggle_shade(ctx);
        }
        if skin != Skin::HiFi {
            ui.painter().line_segment(
                [rect.left_bottom(), rect.right_bottom()],
                Stroke::new(1.0_f32, p.border),
            );
        }
    }

    /// What the display shows about the current track.
    fn now_playing(&self) -> NowPlaying {
        let Some(t) = self.playback.current.clone() else {
            return NowPlaying {
                source: "NPLAY".into(),
                status: "STOPPED".into(),
                title: "Nothing playing".into(),
                subtitle: "Browse or search to begin".into(),
                detail: String::new(),
            };
        };
        let radio = &self.playback.radio;
        let get = |k: &str| {
            radio
                .get(k)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };
        let radio_title = get("program");
        let radio_line = [get("song"), get("episode"), get("next_program")]
            .into_iter()
            .find(|x| !x.is_empty())
            .unwrap_or_default();
        let title = if !radio_title.is_empty() {
            radio_title
        } else {
            t.title.clone()
        };
        let subtitle = if t.kind == "radio" {
            [t.title.clone(), radio_line]
                .into_iter()
                .filter(|x| !x.is_empty())
                .collect::<Vec<_>>()
                .join(" · ")
        } else {
            t.subtitle()
        };
        let mut detail = Vec::new();
        if let Some(br) = t.bitrate() {
            detail.push(format!("{br} kbps"));
        }
        if let Some(y) = t.year() {
            detail.push(y.to_string());
        }
        if self.playback.transport == "spotify" {
            detail.push("SPOTIFY CONNECT".into());
        }
        NowPlaying {
            source: t.source_label().to_string(),
            status: self.playback.status_label().to_string(),
            title,
            subtitle,
            detail: detail.join(" · "),
        }
    }

    fn clock_text(&self) -> String {
        let position = self.position_now();
        let duration = self.playback.duration.max(0.0);
        if self.prefs.remaining_time && duration > 0.0 {
            format!("-{}", fmt_time((duration - position).max(0.0)))
        } else {
            fmt_time(position)
        }
    }

    /// The LED clock is a button: elapsed / remaining.
    fn clock_button(&mut self, ui: &mut Ui, rect: Rect) {
        let p = self.palette();
        let resp = ui.interact(rect, Id::new("deck-led"), Sense::click());
        resp.draw_focus(&ui.painter(), rect, &p);
        if resp.activated(ui) {
            self.prefs.remaining_time = !self.prefs.remaining_time;
            self.update_prefs(ui.ctx());
        }
        resp.on_hover_text("Toggle elapsed / remaining time (Enter when focused)");
    }

    fn draw_artwork(&mut self, ui: &mut Ui, art_rect: Rect, p: &crate::theme::Palette) {
        let key = self.playback.current.as_ref().map(|t| t.key.clone());
        if let Some(key) = key {
            if let Some(texture) = self.artwork.cache.get(&key) {
                ui.painter().image(
                    texture.id(),
                    art_rect,
                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
                ui.painter().rect_stroke(
                    art_rect,
                    crate::gui::Rounding::same(1.0),
                    Stroke::new(1.0_f32, p.border_hi),
                );
            } else {
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                key.hash(&mut hasher);
                draw_art_placeholder(&ui.painter(), art_rect, p, hasher.finish(), "");
            }
        } else {
            draw_art_placeholder(&ui.painter(), art_rect, p, 3, "NIRUAMP");
        }
    }

    /// Classic and Phosphor: artwork, track, LED clock and the spectrum.
    fn draw_display(&mut self, ui: &mut Ui, rect: Rect) {
        let p = self.palette();
        let phos = self.skin() == Skin::Phosphor;
        let scale = self.prefs.scale;
        let s = move |v: f32| v * scale;
        if phos {
            // The tube: a dark bezel and rounded glass.
            ui.painter()
                .rect_filled(rect.expand(1.0), crate::gui::Rounding::same(7.0), p.shadow);
            ui.painter()
                .rect_filled(rect, crate::gui::Rounding::same(6.0), p.inset);
        } else {
            inset_rect(&ui.painter(), rect, p.inset, &p);
        }
        let pad = s(6.0);
        let art_size = rect.height() - pad * 2.0;
        let art_rect = Rect::from_min_size(rect.min + vec2(pad, pad), vec2(art_size, art_size));
        self.draw_artwork(ui, art_rect, &p);
        if phos {
            // Monochrome tube: tint the cover toward the glass.
            ui.painter().rect_filled(
                art_rect,
                crate::gui::Rounding::same(1.0),
                skin::alpha(p.inset, 0.55),
            );
            ui.painter().rect_filled(
                art_rect,
                crate::gui::Rounding::same(1.0),
                skin::alpha(p.vis, 0.10),
            );
        }
        let info = Rect::from_min_max(
            pos2(art_rect.right() + pad * 1.4, rect.top() + pad),
            rect.max - vec2(pad, pad),
        );
        let np = self.now_playing();
        let badge_h = s(15.0);
        let small = |v: f32| {
            if phos {
                FontId::monospace(s(v))
            } else {
                FontId::proportional(s(v))
            }
        };
        let badge_color = if self.playback.is_playing() {
            p.accent
        } else if self.playback.is_loading() {
            p.warning
        } else {
            p.muted
        };
        let source = if phos {
            format!("[{}]", np.source)
        } else {
            np.source.clone()
        };
        let badge_w = s(10.0) + source.chars().count() as f32 * s(6.2);
        let badge = Rect::from_min_size(
            info.left_top(),
            vec2(badge_w.min(info.width() * 0.58), badge_h),
        );
        if !phos {
            ui.painter()
                .rect_filled(badge, crate::gui::Rounding::same(1.0), p.surface_hi);
        }
        ui.painter().text(
            if phos {
                badge.left_center()
            } else {
                badge.center()
            },
            if phos {
                Align2::LEFT_CENTER
            } else {
                Align2::CENTER_CENTER
            },
            &source,
            small(8.0),
            badge_color,
        );
        let status_x = if phos {
            info.left() + source.chars().count() as f32 * s(8.0 * 0.62) + s(8.0)
        } else {
            badge.right() + s(6.0)
        };
        ui.painter().text(
            pos2(status_x, badge.center().y),
            Align2::LEFT_CENTER,
            if phos {
                format!("STATUS: {}", np.status)
            } else {
                np.status.clone()
            },
            small(8.0),
            if self.playback.is_loading() {
                p.warning
            } else {
                p.muted
            },
        );
        if !np.detail.is_empty() {
            ui.painter().text(
                info.right_top() + vec2(0.0, badge_h * 0.5),
                Align2::RIGHT_CENTER,
                if phos {
                    np.detail.to_uppercase()
                } else {
                    np.detail.clone()
                },
                small(8.0),
                p.disabled,
            );
        }

        let time_h = s(30.0);
        let time_w = s(96.0);
        let led_rect = Rect::from_min_size(
            pos2(info.left(), info.bottom() - time_h),
            vec2(time_w, time_h),
        );
        let vis_rect = Rect::from_min_max(
            pos2(led_rect.right() + s(8.0), info.bottom() - time_h),
            info.max,
        );
        let title_rect = Rect::from_min_max(
            pos2(info.left(), badge.bottom() + s(3.0)),
            pos2(info.right(), led_rect.top() - s(2.0)),
        );
        let title_line = Rect::from_min_max(
            title_rect.min,
            pos2(
                title_rect.right(),
                title_rect.top() + title_rect.height() * 0.58,
            ),
        );
        let sub_rect =
            Rect::from_min_max(pos2(title_rect.left(), title_line.bottom()), title_rect.max);
        if phos {
            let title = format!("> {}", np.title);
            let halo = skin::alpha(p.led, 0.22);
            for dx in [-1.0, 1.0] {
                marquee(
                    ui,
                    title_line.translate(vec2(dx, 0.0)),
                    &title,
                    FontId::monospace(s(15.0)),
                    halo,
                    Color32::TRANSPARENT,
                    self.reduce_motion,
                    s(22.0),
                );
            }
            marquee(
                ui,
                title_line,
                &title,
                FontId::monospace(s(15.0)),
                p.led,
                p.inset,
                self.reduce_motion,
                s(22.0),
            );
            marquee(
                ui,
                sub_rect,
                &format!("  {}", np.subtitle),
                FontId::monospace(s(10.0)),
                p.foreground,
                p.inset,
                true,
                s(18.0),
            );
        } else {
            marquee(
                ui,
                title_line,
                &np.title,
                FontId::proportional(s(18.0)),
                p.foreground,
                p.inset,
                self.reduce_motion,
                s(22.0),
            );
            marquee(
                ui,
                sub_rect,
                &np.subtitle,
                FontId::proportional(s(11.0)),
                p.muted,
                p.inset,
                true,
                s(18.0),
            );
        }

        let clock = self.clock_text();
        if phos {
            ui.painter()
                .rect_filled(led_rect, crate::gui::Rounding::same(2.0), p.background);
            ui.painter().rect_stroke(
                led_rect,
                crate::gui::Rounding::same(2.0),
                Stroke::new(1.0_f32, p.border),
            );
            skin::glow_segments(
                &ui.painter(),
                led_rect.shrink2(vec2(s(6.0), s(5.0))),
                &clock,
                p.led,
                p.led_dim,
            );
        } else {
            inset_rect(&ui.painter(), led_rect, p.background, &p);
            seven_segment(
                &ui.painter(),
                led_rect.shrink2(vec2(s(5.0), s(4.0))),
                &clock,
                p.led,
                p.led_dim,
            );
        }
        self.clock_button(ui, led_rect);
        self.draw_visualizer(ui, vis_rect);
        if phos {
            // Scanlines, vignette and glass over the whole tube.
            let ctx = ui.ctx().clone();
            let tex = self.skin_art.crt(
                &ctx,
                rect.width().round() as u32,
                rect.height().round() as u32,
            );
            ui.painter()
                .image(tex.id(), rect, Rect::NOTHING, Color32::WHITE);
        }
    }

    /// Why the spectrum is not drawing, if it is not.
    fn vis_message(&self) -> Option<(&'static str, bool)> {
        if !self.prefs.show_visualization {
            Some(("VISUALIZER OFF · V", false))
        } else if !self.capabilities.visualization {
            Some(("CAVA NOT INSTALLED", false))
        } else if !self.vis.failed.is_empty() {
            Some(("VISUALIZER UNAVAILABLE", true))
        } else if self.vis.displayed.is_empty() {
            Some(if self.playback.is_playing() {
                ("ACQUIRING CAVA", false)
            } else {
                ("PLAYBACK IS STOPPED", false)
            })
        } else {
            None
        }
    }

    fn draw_visualizer(&mut self, ui: &mut Ui, rect: Rect) {
        let p = self.palette();
        let phos = self.skin() == Skin::Phosphor;
        if phos {
            ui.painter()
                .rect_filled(rect, crate::gui::Rounding::same(2.0), p.background);
            ui.painter().rect_stroke(
                rect,
                crate::gui::Rounding::same(2.0),
                Stroke::new(1.0_f32, p.border),
            );
        } else {
            inset_rect(&ui.painter(), rect, p.background, &p);
        }
        let inner = rect.shrink2(vec2(3.0, 3.0));
        let painter = &ui.painter();
        for i in 1..4 {
            let y = (inner.top() + inner.height() * i as f32 / 4.0).round() + 0.5;
            painter.line_segment(
                [pos2(inner.left(), y), pos2(inner.right(), y)],
                Stroke::new(1.0_f32, if phos { p.led_dim } else { p.surface }),
            );
        }
        if let Some((label, warn)) = self.vis_message() {
            painter.text(
                inner.center(),
                Align2::CENTER_CENTER,
                label,
                if phos {
                    FontId::monospace(self.s(8.0))
                } else {
                    FontId::proportional(self.s(8.0))
                },
                if warn { p.warning } else { p.disabled },
            );
            return;
        }
        let bars = &self.vis.displayed;
        let n = bars.len().min(48);
        let gap = if phos { 2.0 } else { 1.0 };
        let bw = ((inner.width() - gap * (n.saturating_sub(1) as f32)) / n.max(1) as f32).max(1.0);
        let height = |v: f32| inner.height() * (0.03 + v.clamp(0.0, 1.0) * 0.97);
        // Classic: three colour zones by height, as on a hardware analyser.
        let low_top = inner.bottom() - inner.height() * 0.5;
        let mid_top = inner.bottom() - inner.height() * 0.8;
        for i in 0..n {
            let v = bars.shown[i];
            let x = inner.left() + i as f32 * (bw + gap);
            let top = inner.bottom() - height(v).max(1.0);
            let bar = Rect::from_min_max(pos2(x, top), pos2(x + bw, inner.bottom()));
            if phos {
                painter.rect_filled(bar.expand2(vec2(1.0, 1.0)), 0.0, skin::alpha(p.vis, 0.18));
                painter.rect_filled(bar, 0.0, p.vis);
            } else {
                let zone = |from: f32, to: f32, color: Color32| {
                    let r = Rect::from_min_max(
                        pos2(x, top.max(to)),
                        pos2(x + bw, from.min(inner.bottom())),
                    );
                    if r.height() > 0.0 {
                        painter.rect_filled(r, 0.0, color);
                    }
                };
                zone(inner.bottom(), low_top, skin::mix(p.led_dim, p.vis, 0.45));
                zone(low_top, mid_top, p.vis);
                zone(mid_top, inner.top(), p.vis_peak);
            }
            let peak = bars.peaks[i];
            if peak > v + 0.015 {
                let y = inner.bottom() - height(peak);
                painter.rect_filled(
                    Rect::from_min_max(pos2(x, y - 1.5), pos2(x + bw, y + 0.5)),
                    0.0,
                    if phos { p.accent } else { p.foreground },
                );
            }
        }
    }

    /// Hi-Fi Deck: cassette window, VFD with time and analyser, VU meters.
    fn draw_hifi_display(&mut self, ui: &mut Ui, rect: Rect) {
        let p = self.palette();
        let scale = self.prefs.scale;
        let s = move |v: f32| v * scale;
        let ctx = ui.ctx().clone();
        let gap = s(8.0);
        let w = rect.width();
        let tape_w = (w * 0.37).clamp(s(150.0), s(250.0));
        let two_meters = w >= s(500.0);
        let vu_w = (rect.height() * 1.02).min(w * 0.17);
        let meters_w = if two_meters {
            vu_w * 2.0 + s(5.0)
        } else {
            vu_w
        };
        let tape = Rect::from_min_size(rect.min, vec2(tape_w, rect.height()));
        let meters = Rect::from_min_max(pos2(rect.right() - meters_w, rect.top()), rect.max);
        let vfd = Rect::from_min_max(
            pos2(tape.right() + gap, rect.top()),
            pos2(meters.left() - gap, rect.bottom()),
        );
        let np = self.now_playing();
        let position = self.position_now();
        let duration = self.playback.duration.max(0.0);
        let progress = if duration > 0.0 {
            (position / duration) as f32
        } else {
            0.0
        };
        skin::cassette(
            &ui.painter(),
            tape,
            &skin::Cassette {
                title: &np.title,
                artist: &np.subtitle,
                progress,
                position: position as f32,
                playing: self.playback.is_playing(),
            },
            &p,
        );

        // Vacuum fluorescent display.
        let painter = ui.painter();
        painter.rect_filled(
            vfd,
            crate::gui::Rounding::same(4.0),
            Color32::from_rgb(10, 12, 13),
        );
        painter.rect_stroke(
            vfd,
            crate::gui::Rounding::same(4.0),
            Stroke::new(1.0_f32, Color32::from_rgb(60, 62, 66)),
        );
        let inner = vfd.shrink2(vec2(s(8.0), s(6.0)));
        let on = p.led;
        let ghost = skin::alpha(p.led, 0.07);
        let status = if self.playback.is_playing() {
            "▶ PLAY"
        } else if self.playback.is_loading() {
            "… LOAD"
        } else if self.playback.status == "paused" {
            "‖ PAUSE"
        } else {
            "■ STOP"
        };
        skin::glow_text(
            &painter,
            inner.left_top(),
            Align2::LEFT_TOP,
            status,
            FontId::proportional(s(8.5)),
            on,
        );
        let mut tags = vec![np.source.clone()];
        if self.playback.shuffle != "off" {
            tags.push("SHUF".into());
        }
        match self.playback.repeat.as_str() {
            "track" => tags.push("RPT 1".into()),
            "context" => tags.push("RPT".into()),
            _ => {}
        }
        painter.text(
            inner.right_top(),
            Align2::RIGHT_TOP,
            tags.join("  "),
            FontId::proportional(s(8.0)),
            skin::alpha(on, 0.8),
        );
        let time_rect = Rect::from_min_size(
            pos2(inner.left(), inner.top() + s(13.0)),
            vec2(inner.width(), s(30.0)),
        );
        skin::glow_segments(
            &painter,
            time_rect.shrink2(vec2(0.0, s(2.0))),
            &self.clock_text(),
            on,
            ghost,
        );
        self.clock_button(ui, time_rect);
        let ana = Rect::from_min_max(pos2(inner.left(), time_rect.bottom() + s(5.0)), inner.max);
        let painter = ui.painter();
        // A 16-band segmented analyser with peak hold.
        let bands = 16usize;
        let segs = ((ana.height() / s(3.0)).floor() as usize).clamp(4, 14);
        let seg_h = ana.height() / segs as f32;
        let bw = (ana.width() - (bands - 1) as f32 * s(2.0)) / bands as f32;
        let bars = &self.vis.displayed;
        let level = |i: usize| -> (f32, f32) {
            if bars.is_empty() {
                return (0.0, 0.0);
            }
            let n = bars.len();
            let a = i * n / bands;
            let b = ((i + 1) * n / bands).max(a + 1).min(n);
            let v = bars.shown[a..b].iter().cloned().fold(0.0, f32::max);
            let pk = bars.peaks[a..b].iter().cloned().fold(0.0, f32::max);
            (v, pk)
        };
        let message = self.vis_message();
        for i in 0..bands {
            let x = ana.left() + i as f32 * (bw + s(2.0));
            let col = Rect::from_min_max(pos2(x, ana.top()), pos2(x + bw, ana.bottom()));
            painter.rect_filled(col, 0.0, ghost);
            if message.is_some() {
                continue;
            }
            let (v, pk) = level(i);
            let lit = (v * segs as f32).round() as usize;
            if lit > 0 {
                let top = ana.bottom() - lit as f32 * seg_h;
                painter.rect_filled(
                    Rect::from_min_max(pos2(x, top), pos2(x + bw, ana.bottom())),
                    0.0,
                    on,
                );
            }
            let peak_seg = (pk * segs as f32).round() as usize;
            if peak_seg > lit {
                let y = ana.bottom() - peak_seg as f32 * seg_h;
                painter.rect_filled(
                    Rect::from_min_max(pos2(x, y), pos2(x + bw, y + seg_h)),
                    0.0,
                    p.vis_peak,
                );
            }
        }
        // Segment gaps: glass-coloured lines across the analyser.
        for k in 1..segs {
            let y = (ana.bottom() - k as f32 * seg_h).round();
            painter.line_segment(
                [pos2(ana.left(), y), pos2(ana.right(), y)],
                Stroke::new(1.0_f32, Color32::from_rgb(10, 12, 13)),
            );
        }
        if let Some((label, warn)) = message {
            painter.text(
                ana.center(),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(s(7.5)),
                if warn {
                    p.warning
                } else {
                    skin::alpha(on, 0.7)
                },
            );
        }
        // Blue-green glass sheen.
        painter.rect_filled(
            Rect::from_min_max(
                vfd.left_top(),
                pos2(vfd.right(), vfd.top() + vfd.height() * 0.3),
            ),
            crate::gui::Rounding::same(4.0),
            skin::alpha(Color32::WHITE, 0.035),
        );

        // VU meters: the low and high halves of the spectrum.
        let half = |lo: bool| -> f32 {
            if bars.is_empty() || message.is_some() {
                return 0.0;
            }
            let n = bars.len();
            let slice = if lo {
                &bars.shown[..n / 2]
            } else {
                &bars.shown[n / 2..]
            };
            let sq: f32 = slice.iter().map(|v| v * v).sum::<f32>() / slice.len().max(1) as f32;
            sq.sqrt()
        };
        if two_meters {
            let left = Rect::from_min_size(meters.min, vec2(vu_w, meters.height()));
            let right = Rect::from_min_size(
                pos2(meters.right() - vu_w, meters.top()),
                vec2(vu_w, meters.height()),
            );
            skin::vu_meter(
                &painter,
                &mut self.skin_art,
                &ctx,
                left,
                half(true),
                "LOW",
                &p,
            );
            skin::vu_meter(
                &painter,
                &mut self.skin_art,
                &ctx,
                right,
                half(false),
                "HIGH",
                &p,
            );
        } else {
            let both = (half(true) + half(false)) * 0.5;
            skin::vu_meter(
                &painter,
                &mut self.skin_art,
                &ctx,
                meters,
                both,
                "LEVEL",
                &p,
            );
        }
    }

    fn draw_seek(&mut self, ui: &mut Ui, rect: Rect) {
        let p = self.palette();
        let scale = self.prefs.scale;
        let s = move |v: f32| v * scale;
        let pos = self.position_now();
        let dur = self.playback.duration.max(0.0);
        let time_w = s(42.0);
        let left_rect = Rect::from_min_size(rect.min, vec2(time_w, rect.height()));
        let right_rect = Rect::from_min_size(
            pos2(rect.right() - time_w, rect.top()),
            vec2(time_w, rect.height()),
        );
        let slider_rect = Rect::from_min_max(
            pos2(left_rect.right() + s(4.0), rect.top()),
            pos2(right_rect.left() - s(4.0), rect.bottom()),
        );
        ui.painter().text(
            left_rect.center(),
            Align2::CENTER_CENTER,
            fmt_time(pos),
            FontId::proportional(s(9.0)),
            p.muted,
        );
        ui.painter().text(
            right_rect.center(),
            Align2::CENTER_CENTER,
            if dur > 0.0 {
                fmt_time(dur)
            } else {
                "--:--".into()
            },
            FontId::proportional(s(9.0)),
            p.muted,
        );
        let seekable = self.playback.seekable && dur > 0.0;
        let response = ui.interact(
            slider_rect,
            Id::new("deck-seek-hit"),
            Sense::click_and_drag(),
        );
        let result = hardware_slider_rect(
            ui,
            &p,
            pos,
            dur.max(0.001),
            seekable,
            slider_rect,
            Id::new("deck-seek"),
            p.accent,
            false,
            response,
        );
        if seekable && (result.response.dragged() || result.response.clicked()) && result.changed {
            self.seek_to(result.value);
        }
        if seekable && result.response.drag_stopped() {
            self.commit_seek();
        }
        if !seekable {
            result.response.on_hover_text("This source cannot seek");
        }
    }

    fn draw_transport(&mut self, ui: &mut Ui, _ctx: &Context, rect: Rect) {
        let p = self.palette();
        let scale = self.prefs.scale;
        let s = move |v: f32| v * scale;
        let bw = s(33.0);
        let bh = s(25.0);
        let gap = s(3.0);
        let total = bw * 6.0 + gap * 5.0;
        let mut x = rect.center().x - total / 2.0;
        let y = rect.center().y;
        let buttons = [
            (Icon::Prev, "Previous", 1u8),
            (Icon::Play, "Play", 2),
            (Icon::Pause, "Pause", 3),
            (Icon::Stop, "Stop", 4),
            (Icon::Next, "Next", 5),
            (Icon::Eject, "Browse", 6),
        ];
        for (icon, tip, id) in buttons {
            let r = Rect::from_center_size(pos2(x + bw / 2.0, y), vec2(bw, bh));
            let active = matches!(icon, Icon::Play) && self.playback.is_playing();
            let resp = icon_button_rect(
                ui,
                Id::new(("deck-transport", id)),
                r,
                &p,
                icon,
                active,
                true,
                tip,
            );
            if resp.activated(ui) {
                match icon {
                    Icon::Prev => self.previous(),
                    Icon::Play => self.toggle(),
                    Icon::Pause => {
                        if self.playback.is_playing() {
                            self.toggle();
                        }
                    }
                    Icon::Stop => self.stop(),
                    Icon::Next => self.next(),
                    Icon::Eject => self.show_tab(Tab::Browse, ui.ctx()),
                    _ => {}
                }
            }
            x += bw + gap;
        }
        // Shuffle and repeat lamps, right of the transport.
        let lamps = [
            (
                match self.playback.shuffle.as_str() {
                    "smart" => "SMART SHUFFLE",
                    _ => "SHUFFLE",
                },
                self.playback.shuffle != "off",
            ),
            (
                match self.playback.repeat.as_str() {
                    "track" => "REPEAT ONE",
                    "context" => "REPEAT ALL",
                    _ => "REPEAT",
                },
                self.playback.repeat != "off",
            ),
        ];
        let font = if self.skin() == Skin::Phosphor {
            FontId::monospace(s(7.5))
        } else {
            FontId::proportional(s(7.5))
        };
        for (k, (label, on)) in lamps.iter().enumerate() {
            let y = rect.center().y + if k == 0 { -s(6.0) } else { s(6.0) };
            let t = ui.painter().text(
                pos2(rect.right() - s(2.0), y),
                Align2::RIGHT_CENTER,
                *label,
                font,
                if *on { p.foreground } else { p.disabled },
            );
            let lamp = pos2(t.left() - s(6.0), y);
            ui.painter()
                .circle_filled(lamp, s(2.4), if *on { p.accent } else { p.inset });
            if *on {
                ui.painter()
                    .circle_filled(lamp, s(4.0), skin::alpha(p.accent, 0.18));
            }
        }
    }

    fn draw_lower(&mut self, ui: &mut Ui, ctx: &Context, rect: Rect) {
        let p = self.palette();
        let scale = self.prefs.scale;
        let s = move |v: f32| v * scale;
        let btn = s(22.0);
        let gap = s(3.0);
        let mut x = rect.left();
        let mute_icon = if self.playback.muted || self.volume_now() <= 0.0 {
            Icon::Mute
        } else {
            Icon::Volume
        };
        let mute = icon_button_rect(
            ui,
            Id::new("deck-mute"),
            Rect::from_center_size(pos2(x + btn / 2.0, rect.center().y), vec2(btn, btn)),
            &p,
            mute_icon,
            false,
            true,
            "Mute (M)",
        );
        if mute.activated(ui) {
            self.mute();
        }
        x += btn + gap;
        let vol_w = s(104.0);
        let vol_rect =
            Rect::from_center_size(pos2(x + vol_w / 2.0, rect.center().y), vec2(vol_w, s(12.0)));
        let vresp = ui.interact(
            vol_rect,
            Id::new("deck-volume-hit"),
            Sense::click_and_drag(),
        );
        let vol = hardware_slider_rect(
            ui,
            &p,
            self.volume_now(),
            150.0,
            true,
            vol_rect,
            Id::new("deck-volume"),
            p.accent,
            true,
            vresp,
        );
        if vol.changed {
            self.set_volume(vol.value);
        }
        if vol.response.drag_stopped() {
            self.commit_volume();
        }
        x += vol_w + gap;
        ui.painter().text(
            pos2(x + s(2.0), rect.center().y),
            Align2::LEFT_CENTER,
            format!("{:>3.0}%", self.volume_now()),
            FontId::monospace(s(9.0)),
            p.muted,
        );
        x += s(36.0);

        let toggles = [
            (
                Icon::Shuffle,
                "Shuffle (H)",
                self.playback.shuffle != "off",
                11u8,
            ),
            (
                Icon::Repeat,
                "Repeat (R)",
                self.playback.repeat != "off",
                12,
            ),
            (Icon::Skin, "Next skin (K)", false, 13),
            (
                Icon::Playlist,
                "Playlist (1)",
                self.tab == Tab::Playlist,
                14,
            ),
            (Icon::Queue, "Queue (2)", self.tab == Tab::Queue, 15),
            (
                Icon::Search,
                "Search & browse (/)",
                self.tab == Tab::Browse,
                16,
            ),
            (Icon::Settings, "Settings", self.tab == Tab::Settings, 17),
        ];
        for (icon, tip, active, id) in toggles {
            let r = Rect::from_center_size(pos2(x + btn / 2.0, rect.center().y), vec2(btn, btn));
            let resp = icon_button_rect(
                ui,
                Id::new(("deck-toggle", id)),
                r,
                &p,
                icon,
                active,
                true,
                tip,
            );
            if resp.activated(ui) {
                match icon {
                    Icon::Skin => self.set_skin(self.skin().next(), ctx),
                    Icon::Shuffle => self.shuffle(),
                    Icon::Repeat => self.repeat(),
                    Icon::Playlist => self.show_tab(Tab::Playlist, ctx),
                    Icon::Queue => self.show_tab(Tab::Queue, ctx),
                    Icon::Search => {
                        self.browser.focus_search = true;
                        self.show_tab(Tab::Browse, ctx);
                    }
                    Icon::Settings => self.show_tab(Tab::Settings, ctx),
                    _ => {}
                }
            }
            x += btn + gap;
        }
    }

    fn shade_ui(&mut self, ui: &mut Ui, ctx: &Context) {
        let p = self.palette();
        let scale = self.prefs.scale;
        let s = move |v: f32| v * scale;
        let h = s(44.0);
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
        let painter = &ui.painter();
        bevel_rect(painter, rect, p.surface, &p);
        let logo_rect = Rect::from_center_size(
            rect.left_center() + vec2(s(14.0), 0.0),
            vec2(s(16.0), s(16.0)),
        );
        logo(painter, logo_rect, &p);
        let pos = self.position_now();
        let clock = if self.prefs.remaining_time && self.playback.duration > 0.0 {
            format!("-{}", fmt_time((self.playback.duration - pos).max(0.0)))
        } else {
            fmt_time(pos)
        };
        let led_rect = Rect::from_center_size(
            rect.left_center() + vec2(s(72.0), 0.0),
            vec2(s(82.0), s(24.0)),
        );
        inset_rect(painter, led_rect, p.background, &p);
        seven_segment(
            painter,
            led_rect.shrink2(vec2(s(4.0), s(3.0))),
            &clock,
            p.led,
            p.led_dim,
        );
        let title = self
            .playback
            .current
            .as_ref()
            .map(|t| t.title.clone())
            .unwrap_or_else(|| "NIRUAMP".into());
        let title_rect = Rect::from_min_max(
            pos2(led_rect.right() + s(8.0), rect.top() + s(6.0)),
            pos2(rect.right() - s(100.0), rect.bottom() - s(6.0)),
        );
        marquee(
            ui,
            title_rect,
            &title,
            FontId::proportional(s(13.0)),
            p.foreground,
            p.surface,
            self.reduce_motion,
            s(18.0),
        );
        let btn = s(18.0);
        let mut x = rect.right() - btn;
        for (icon, tip) in [
            (Icon::Close, "Close"),
            (Icon::Down, "Unshade"),
            (Icon::Play, "Play/Pause"),
            (Icon::Next, "Next"),
        ] {
            let r = Rect::from_center_size(pos2(x - btn / 2.0, rect.center().y), vec2(btn, btn));
            let resp = icon_button_rect(
                ui,
                Id::new(("shade", icon as u8)),
                r,
                &p,
                icon,
                false,
                true,
                tip,
            );
            if resp.activated(ui) {
                match icon {
                    Icon::Close => ctx.send_viewport_cmd(crate::gui::ViewportCommand::Close),
                    Icon::Down => self.toggle_shade(ctx),
                    Icon::Play => self.toggle(),
                    Icon::Next => self.next(),
                    _ => {}
                }
            }
            x -= btn + s(2.0);
        }
        let drag = ui.interact(
            rect,
            Id::new("shade-drag"),
            Sense {
                click: true,
                drag: true,
                focusable: false,
            },
        );
        if drag.drag_started() {
            self.start_drag(ctx);
        }
        if drag.double_clicked() {
            self.toggle_shade(ctx);
        }
        self.deck_height = h;
    }

    pub(crate) fn toggle_shade(&mut self, ctx: &Context) {
        self.prefs.window_shade = !self.prefs.window_shade;
        if self.prefs.window_shade {
            let inner = ctx.input(|i| i.viewport().inner_rect);
            if let Some(r) = inner {
                if r.height() > self.s(100.0) {
                    self.window_state.normal_width = r.width();
                    self.window_state.normal_height = r.height();
                }
            }
            ctx.send_viewport_cmd(crate::gui::ViewportCommand::InnerSize(vec2(
                self.window_state.normal_width.max(self.s(420.0)),
                self.s(58.0),
            )));
        } else {
            ctx.send_viewport_cmd(crate::gui::ViewportCommand::InnerSize(vec2(
                self.window_state.normal_width.max(self.s(420.0)),
                self.window_state.normal_height.max(self.s(300.0)),
            )));
        }
        self.update_prefs(ctx);
    }

    pub(crate) fn toggle_pin(&mut self, ctx: &Context) {
        self.prefs.always_on_top = !self.prefs.always_on_top;
        if let Some(h) = self.hypr.as_ref() {
            if let Some(c) = h.own_client() {
                h.set_pinned(&c, self.prefs.always_on_top, self.hypr_lua);
            }
        }
        ctx.send_viewport_cmd(crate::gui::ViewportCommand::WindowLevel(
            if self.prefs.always_on_top {
                crate::gui::WindowLevel::AlwaysOnTop
            } else {
                crate::gui::WindowLevel::Normal
            },
        ));
        self.update_prefs(ctx);
    }

    pub(crate) fn show_tab(&mut self, tab: Tab, ctx: &Context) {
        self.tab = tab;
        if tab == Tab::Browse && self.browser.items.is_empty() && self.browser.groups.is_empty() {
            self.default_browse();
        }
        if tab == Tab::Settings && !self.settings.loaded {
            self.load_settings();
        }
        if self.prefs.window_shade {
            self.prefs.window_shade = false;
            ctx.send_viewport_cmd(crate::gui::ViewportCommand::InnerSize(vec2(
                self.window_state.normal_width.max(self.s(420.0)),
                self.window_state.normal_height.max(self.s(300.0)),
            )));
        }
        self.prefs.active_tab = tab.key().to_string();
        self.update_prefs(ctx);
    }
}

/// The display's view of the current track.
struct NowPlaying {
    source: String,
    status: String,
    title: String,
    subtitle: String,
    detail: String,
}
