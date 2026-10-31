//! The player deck: digital clock, artwork, spectrum and transport controls.
use crate::app::{NiruampApp, Tab};
use crate::views::common::*;
use egui::{pos2, vec2, Align2, Color32, Context, FontId, Id, Rect, Sense, Stroke, Ui};
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
        let painter = ui.painter();
        let panel = rect.shrink(0.5);
        painter.rect_filled(panel, egui::Rounding::same(3.0), p.surface);
        painter.rect_stroke(
            panel,
            egui::Rounding::same(3.0),
            Stroke::new(1.0_f32, p.border),
        );
        painter.line_segment(
            [panel.left_top(), panel.right_top()],
            Stroke::new(1.0_f32, p.border_hi),
        );
        painter.line_segment(
            [panel.left_top(), panel.left_bottom()],
            Stroke::new(1.0_f32, p.border_hi),
        );
        painter.line_segment(
            [panel.right_top(), panel.right_bottom()],
            Stroke::new(1.0_f32, p.shadow),
        );
        painter.line_segment(
            [panel.left_bottom(), panel.right_bottom()],
            Stroke::new(1.0_f32, p.shadow),
        );

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

        self.draw_title_bar(ui, ctx, title);
        self.draw_display(ui, display);
        self.draw_seek(ui, seek);
        self.draw_transport(ui, ctx, transport);
        self.draw_lower(ui, ctx, lower);
        self.deck_height = deck_h;
    }

    fn draw_title_bar(&mut self, ui: &mut Ui, ctx: &Context, rect: Rect) {
        let p = self.palette();
        let scale = self.prefs.scale;
        let s = move |v: f32| v * scale;
        let painter = ui.painter();
        let logo_rect = Rect::from_min_size(
            rect.left_center() + vec2(2.0, -s(8.0)),
            vec2(s(16.0), s(16.0)),
        );
        logo(painter, logo_rect, &p);
        painter.text(
            pos2(logo_rect.right() + s(6.0), rect.center().y - s(1.0)),
            Align2::LEFT_CENTER,
            "NIRUAMP",
            FontId::proportional(s(12.0)),
            p.foreground,
        );
        let name_width = s(78.0);
        painter.text(
            pos2(
                logo_rect.right() + s(6.0) + name_width,
                rect.center().y - s(1.0),
            ),
            Align2::LEFT_CENTER,
            format!("v{}", env!("CARGO_PKG_VERSION")),
            FontId::proportional(s(8.0)),
            p.disabled,
        );
        let dot = pos2(
            logo_rect.right() + s(6.0) + name_width + s(46.0),
            rect.center().y,
        );
        let status_color = if self.backend.is_some() {
            if self.playback.is_playing() {
                p.success
            } else {
                p.muted
            }
        } else {
            p.error
        };
        painter.circle_filled(dot, s(2.6), status_color);
        painter.text(
            pos2(dot.x + s(7.0), rect.center().y - s(1.0)),
            Align2::LEFT_CENTER,
            if self.backend.is_some() {
                "NPLAY"
            } else {
                "OFFLINE"
            },
            FontId::proportional(s(8.0)),
            p.muted,
        );

        let btn = s(17.0);
        let mut x = rect.right() - btn;
        let actions = [
            (Icon::Close, "Close NIRUAMP"),
            (Icon::Minimize, "Minimize"),
            (Icon::Up, "Windowshade"),
            (Icon::Pin, "Always on top"),
            (Icon::Settings, "Settings"),
        ];
        for (icon, tip) in actions {
            let r = Rect::from_center_size(pos2(x - btn / 2.0, rect.center().y), vec2(btn, btn));
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
                    Icon::Close => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                    Icon::Minimize => self.hide_to_tray(ctx),
                    Icon::Down => self.toggle_shade(ctx),
                    Icon::Pin => self.toggle_pin(ctx),
                    Icon::Settings => self.show_tab(Tab::Settings, ctx),
                    _ => {}
                }
            }
            x -= btn + s(2.0);
        }
        let drag_rect = Rect::from_min_max(rect.min, pos2(x - s(4.0), rect.max.y));
        let drag = ui.interact(
            drag_rect,
            Id::new("deck-title-drag"),
            Sense::click_and_drag(),
        );
        if drag.drag_started() {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        if drag.double_clicked() {
            self.toggle_shade(ctx);
        }
        ui.painter().line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            Stroke::new(1.0_f32, p.border),
        );
    }

    fn draw_display(&mut self, ui: &mut Ui, rect: Rect) {
        let p = self.palette();
        let scale = self.prefs.scale;
        let s = move |v: f32| v * scale;
        inset_rect(ui.painter(), rect, p.inset, &p);
        let pad = s(6.0);
        let art_size = rect.height() - pad * 2.0;
        let art_rect = Rect::from_min_size(rect.min + vec2(pad, pad), vec2(art_size, art_size));
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
                    egui::Rounding::same(1.0),
                    Stroke::new(1.0_f32, p.border_hi),
                );
            } else {
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                key.hash(&mut hasher);
                draw_art_placeholder(ui.painter(), art_rect, &p, hasher.finish(), "");
            }
        } else {
            draw_art_placeholder(ui.painter(), art_rect, &p, 3, "NIRUAMP");
        }
        let info = Rect::from_min_max(
            pos2(art_rect.right() + pad * 1.4, rect.top() + pad),
            rect.max - vec2(pad, pad),
        );
        let badge_h = s(15.0);
        let current = self.playback.current.clone();
        let (source, status, title, subtitle) = if let Some(t) = current {
            let radio = self.playback.radio.clone();
            let get = |k: &str| {
                radio
                    .get(k)
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            };
            let radio_title = get("program");
            let radio_song = get("song");
            let radio_episode = get("episode");
            let radio_line = [
                radio_song.clone(),
                radio_episode.clone(),
                get("next_program"),
            ]
            .into_iter()
            .find(|x| !x.is_empty())
            .unwrap_or_default();
            let title = if !radio_title.is_empty() {
                radio_title
            } else {
                t.title.clone()
            };
            let sub = if t.kind == "radio" {
                [t.title.clone(), radio_line]
                    .into_iter()
                    .filter(|x| !x.is_empty())
                    .collect::<Vec<_>>()
                    .join(" · ")
            } else {
                t.subtitle()
            };
            (
                t.source_label().to_string(),
                self.playback.status_label().to_string(),
                title,
                sub,
            )
        } else {
            (
                "NPLAY".to_string(),
                "STOPPED".to_string(),
                "Nothing playing".to_string(),
                "Browse or search to begin".to_string(),
            )
        };
        let badge_color = if self.playback.is_playing() {
            p.accent
        } else if self.playback.is_loading() {
            p.warning
        } else {
            p.muted
        };
        let badge_w = s(10.0) + source.chars().count() as f32 * s(6.2);
        let badge = Rect::from_min_size(
            info.left_top(),
            vec2(badge_w.min(info.width() * 0.58), badge_h),
        );
        ui.painter()
            .rect_filled(badge, egui::Rounding::same(1.0), p.surface_hi);
        ui.painter().text(
            badge.center(),
            Align2::CENTER_CENTER,
            &source,
            FontId::proportional(s(8.0)),
            badge_color,
        );
        ui.painter().text(
            pos2(badge.right() + s(6.0), badge.center().y),
            Align2::LEFT_CENTER,
            &status,
            FontId::proportional(s(8.0)),
            if self.playback.is_loading() {
                p.warning
            } else {
                p.muted
            },
        );
        if let Some(t) = self.playback.current.as_ref() {
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
            if !detail.is_empty() {
                ui.painter().text(
                    info.right_top() + vec2(0.0, badge_h * 0.5),
                    Align2::RIGHT_CENTER,
                    detail.join(" · "),
                    FontId::proportional(s(8.0)),
                    p.disabled,
                );
            }
        }

        let time_h = s(25.0);
        let time_w = s(91.0);
        let led_rect = Rect::from_min_size(
            pos2(info.left(), info.bottom() - time_h),
            vec2(time_w, time_h),
        );
        let vis_rect = Rect::from_min_max(
            pos2(led_rect.right() + s(8.0), info.bottom() - time_h),
            info.max,
        );
        let title_rect = Rect::from_min_max(
            pos2(info.left(), badge.bottom() + s(4.0)),
            pos2(info.right(), led_rect.top() - s(2.0)),
        );
        let title_font = FontId::proportional(s(18.0));
        let title_color = p.foreground;
        marquee(
            ui,
            Rect::from_min_max(
                title_rect.min,
                pos2(
                    title_rect.right(),
                    title_rect.top() + title_rect.height() * 0.55,
                ),
            ),
            &title,
            title_font,
            title_color,
            p.inset,
            self.reduce_motion,
            s(22.0),
        );
        let sub_rect = Rect::from_min_max(
            pos2(
                title_rect.left(),
                title_rect.top() + title_rect.height() * 0.54,
            ),
            title_rect.max,
        );
        marquee(
            ui,
            sub_rect,
            &subtitle,
            FontId::proportional(s(11.0)),
            p.muted,
            p.inset,
            true,
            s(18.0),
        );

        let position = self.position_now();
        let duration = self.playback.duration.max(0.0);
        let remaining = (duration - position).max(0.0);
        let clock = if self.prefs.remaining_time && duration > 0.0 {
            format!("-{}", fmt_time(remaining))
        } else {
            fmt_time(position)
        };
        inset_rect(ui.painter(), led_rect, p.background, &p);
        seven_segment(
            ui.painter(),
            led_rect.shrink2(vec2(s(4.0), s(3.0))),
            &clock,
            p.led,
            p.led_dim,
        );
        let led_resp = ui.interact(led_rect, Id::new("deck-led"), Sense::click());
        led_resp.draw_focus(ui.painter(), led_rect, &p);
        if led_resp.activated(ui) {
            self.prefs.remaining_time = !self.prefs.remaining_time;
            self.update_prefs(ui.ctx());
        }
        led_resp.on_hover_text("Toggle elapsed / remaining time (Enter when focused)");

        self.draw_visualizer(ui, vis_rect);
    }

    fn draw_visualizer(&mut self, ui: &mut Ui, rect: Rect) {
        let p = self.palette();
        inset_rect(ui.painter(), rect, p.background, &p);
        let inner = rect.shrink2(vec2(3.0, 2.0));
        let painter = ui.painter();
        for i in 1..4 {
            let y = inner.top() + inner.height() * i as f32 / 4.0;
            painter.line_segment(
                [pos2(inner.left(), y), pos2(inner.right(), y)],
                Stroke::new(1.0_f32, p.surface),
            );
        }
        if !self.prefs.show_visualization {
            painter.text(
                inner.center(),
                Align2::CENTER_CENTER,
                "VISUALIZER OFF",
                FontId::proportional(self.s(8.0)),
                p.disabled,
            );
            return;
        }
        if !self.capabilities.visualization {
            painter.text(
                inner.center(),
                Align2::CENTER_CENTER,
                "CAVA NOT INSTALLED",
                FontId::proportional(self.s(8.0)),
                p.disabled,
            );
            return;
        }
        if !self.vis.failed.is_empty() {
            painter.text(
                inner.center(),
                Align2::CENTER_CENTER,
                "VISUALIZER UNAVAILABLE",
                FontId::proportional(self.s(8.0)),
                p.warning,
            );
            return;
        }
        if self.vis.displayed.is_empty() {
            let label = if self.playback.is_playing() {
                "ACQUIRING CAVA"
            } else {
                "PLAYBACK IS STOPPED"
            };
            painter.text(
                inner.center(),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(self.s(8.0)),
                p.disabled,
            );
            return;
        }
        let n = self.vis.displayed.len().min(48);
        let gap = 1.0;
        let bw = ((inner.width() - gap * (n.saturating_sub(1) as f32)) / n.max(1) as f32).max(1.0);
        for i in 0..n {
            let v = self.vis.displayed[i].clamp(0.0, 1.0);
            let h = (inner.height() * (0.06 + v * 0.94)).max(1.0);
            let x = inner.left() + i as f32 * (bw + gap);
            let bar = Rect::from_min_max(pos2(x, inner.bottom() - h), pos2(x + bw, inner.bottom()));
            let color = if v > 0.82 {
                p.vis_peak
            } else if v > 0.45 {
                p.vis
            } else {
                p.led_dim
            };
            painter.rect_filled(bar, egui::Rounding::same(1.0), color);
            if v > 0.18 {
                painter.line_segment(
                    [bar.left_top(), bar.right_top()],
                    Stroke::new(1.0_f32, p.accent),
                );
            }
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
        let mode_text = format!(
            "{} · {} · {}",
            self.playback.status,
            self.playback.shuffle.to_uppercase(),
            self.playback.repeat.to_uppercase()
        );
        ui.painter().text(
            rect.right_center() + vec2(0.0, 0.0),
            Align2::RIGHT_CENTER,
            mode_text,
            FontId::proportional(s(7.5)),
            p.disabled,
        );
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
            "Mute",
        );
        if mute.clicked() {
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
                "Shuffle",
                self.playback.shuffle != "off",
                11u8,
            ),
            (Icon::Repeat, "Repeat", self.playback.repeat != "off", 12),
            (Icon::Equalizer, "Equalizer", false, 13),
            (Icon::Playlist, "Playlist", self.tab == Tab::Playlist, 14),
            (Icon::Queue, "Queue", self.tab == Tab::Queue, 15),
            (Icon::Search, "Search & browse", self.tab == Tab::Browse, 16),
            (Icon::Settings, "Settings", self.tab == Tab::Settings, 17),
        ];
        for (icon, tip, active, id) in toggles {
            let r = Rect::from_center_size(pos2(x + btn / 2.0, rect.center().y), vec2(btn, btn));
            let enabled = !matches!(icon, Icon::Equalizer);
            let mut resp = icon_button_rect(
                ui,
                Id::new(("deck-toggle", id)),
                r,
                &p,
                icon,
                active,
                enabled,
                tip,
            );
            if matches!(icon, Icon::Equalizer) {
                resp = resp.on_hover_text("Equalizer unavailable: NPLAY exposes no EQ DSP for mpv, and Spotify is not routed through mpv.");
            }
            if resp.activated(ui) {
                match icon {
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
        let repeat_text = match self.playback.repeat.as_str() {
            "track" => "REPEAT 1",
            "context" => "REPEAT ALL",
            _ => "",
        };
        let shuffle_text = match self.playback.shuffle.as_str() {
            "smart" => "SMART SHUFFLE",
            "shuffle" => "SHUFFLE",
            _ => "",
        };
        let extra = [repeat_text, shuffle_text]
            .into_iter()
            .filter(|x| !x.is_empty())
            .collect::<Vec<_>>()
            .join(" · ");
        if !extra.is_empty() {
            ui.painter().text(
                rect.right_center(),
                Align2::RIGHT_CENTER,
                extra,
                FontId::proportional(s(7.5)),
                p.muted,
            );
        }
    }

    fn shade_ui(&mut self, ui: &mut Ui, ctx: &Context) {
        let p = self.palette();
        let scale = self.prefs.scale;
        let s = move |v: f32| v * scale;
        let h = s(44.0);
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
        let painter = ui.painter();
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
                    Icon::Close => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
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
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
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
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(vec2(
                self.window_state.normal_width.max(self.s(420.0)),
                self.s(58.0),
            )));
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(vec2(
                self.window_state.normal_width.max(self.s(420.0)),
                self.window_state.normal_height.max(self.s(300.0)),
            )));
        }
        self.update_prefs(ctx);
    }

    pub(crate) fn toggle_pin(&mut self, ctx: &Context) {
        self.prefs.always_on_top = !self.prefs.always_on_top;
        ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
            if self.prefs.always_on_top {
                egui::WindowLevel::AlwaysOnTop
            } else {
                egui::WindowLevel::Normal
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
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(vec2(
                self.window_state.normal_width.max(self.s(420.0)),
                self.window_state.normal_height.max(self.s(300.0)),
            )));
        }
        self.prefs.active_tab = tab.key().to_string();
        self.update_prefs(ctx);
    }
}
