//! Playlist, queue and shared dense track rows.
use crate::app::{NiruampApp, Tab};
use crate::model::Track;
use crate::views::common::*;
use egui::{pos2, vec2, Align2, Context, FontId, Id, Rect, RichText, Sense, Stroke, Ui};

pub(crate) enum RowAction {
    Play,
    PlayNext,
    Queue,
    Favorite,
    AddToPlaylist(i64, String),
    NewPlaylist,
    RemoveQueue,
    MoveUp,
    MoveDown,
    Info,
}

pub(crate) struct RowOutcome {
    pub(crate) clicked: bool,
    pub(crate) double_clicked: bool,
    pub(crate) drag_started: bool,
    pub(crate) drag_stopped: bool,
    pub(crate) hovered: bool,
    pub(crate) action: Option<RowAction>,
    pub(crate) rect: Rect,
}

pub(crate) fn track_row(
    app: &NiruampApp,
    ui: &mut Ui,
    track: &Track,
    number: &str,
    selected: bool,
    current: bool,
    row_h: f32,
    show_duration: bool,
    draggable: bool,
) -> RowOutcome {
    let p = app.palette();
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(
        vec2(width, row_h),
        Sense {
            click: true,
            drag: draggable,
            focusable: false,
        },
    );
    let painter = ui.painter();
    if selected {
        painter.rect_filled(rect, egui::Rounding::ZERO, p.selected);
        painter.rect_filled(
            Rect::from_min_max(rect.left_top(), pos2(rect.left() + 3.0, rect.bottom())),
            egui::Rounding::ZERO,
            p.accent,
        );
    } else if response.hovered() {
        painter.rect_filled(rect, egui::Rounding::ZERO, p.surface_hi);
    }
    painter.line_segment(
        [
            pos2(rect.left(), rect.bottom()),
            pos2(rect.right(), rect.bottom()),
        ],
        Stroke::new(1.0_f32, p.surface),
    );
    let num_rect = Rect::from_min_size(rect.min, vec2(app.s(26.0), rect.height()));
    painter.text(
        num_rect.center(),
        Align2::CENTER_CENTER,
        number,
        FontId::monospace(app.s(9.0)),
        if current { p.accent } else { p.muted },
    );
    if current {
        let tri = Rect::from_center_size(
            pos2(num_rect.right() + app.s(4.0), rect.center().y),
            vec2(app.s(7.0), app.s(7.0)),
        );
        draw_icon(painter, Icon::Play, tri, p.accent);
    }
    let dur_w = if show_duration { app.s(42.0) } else { 0.0 };
    let text_left = num_rect.right() + app.s(10.0);
    let text_right = rect.right() - dur_w - app.s(8.0);
    let title_color = if current && !p.light {
        p.accent
    } else {
        p.foreground
    };
    let title_font = FontId::proportional(app.s(11.5));
    let title_rect = Rect::from_min_max(
        pos2(text_left, rect.top() + app.s(1.5)),
        pos2(text_right, rect.center().y + app.s(1.0)),
    );
    let sub_rect = Rect::from_min_max(
        pos2(text_left, rect.center().y),
        pos2(text_right, rect.bottom() - app.s(1.0)),
    );
    let sub = if track.artist.is_empty() {
        track.album.clone()
    } else if track.album.is_empty() || track.album == track.title {
        track.artist.clone()
    } else {
        format!("{} · {}", track.artist, track.album)
    };
    marquee(
        ui,
        title_rect,
        &track.title,
        title_font,
        title_color,
        p.surface,
        true,
        app.s(20.0),
    );
    marquee(
        ui,
        sub_rect,
        &sub,
        FontId::proportional(app.s(9.0)),
        p.muted,
        p.surface,
        true,
        app.s(16.0),
    );
    if show_duration {
        painter.text(
            pos2(rect.right() - app.s(8.0), rect.center().y),
            Align2::RIGHT_CENTER,
            fmt_duration(track.duration),
            FontId::monospace(app.s(9.0)),
            p.muted,
        );
    }
    let mut outcome = RowOutcome {
        clicked: response.clicked(),
        double_clicked: response.double_clicked(),
        drag_started: response.drag_started(),
        drag_stopped: response.drag_stopped(),
        hovered: response.hovered(),
        action: None,
        rect,
    };
    let playlists = app.playlists.clone();
    response.context_menu(|ui| {
        ui.set_min_width(app.s(190.0));
        if ui.button("Play now").clicked() {
            outcome.action = Some(RowAction::Play);
            ui.close_menu();
        }
        if ui.button("Play next").clicked() {
            outcome.action = Some(RowAction::PlayNext);
            ui.close_menu();
        }
        if ui.button("Add to queue").clicked() {
            outcome.action = Some(RowAction::Queue);
            ui.close_menu();
        }
        ui.menu_button("Add to playlist", |ui| {
            if playlists.is_empty() {
                ui.label(RichText::new("No local playlists yet").color(p.muted));
            }
            for playlist in &playlists {
                if ui.button(&playlist.name).clicked() {
                    outcome.action =
                        Some(RowAction::AddToPlaylist(playlist.id, playlist.name.clone()));
                    ui.close_menu();
                }
            }
            ui.separator();
            if ui.button("New playlist…").clicked() {
                outcome.action = Some(RowAction::NewPlaylist);
                ui.close_menu();
            }
        });
        if ui.button("Favorite").clicked() {
            outcome.action = Some(RowAction::Favorite);
            ui.close_menu();
        }
        if ui.button("Track details").clicked() {
            outcome.action = Some(RowAction::Info);
            ui.close_menu();
        }
    });
    if response.double_clicked() {
        outcome.action = Some(RowAction::Play);
    }
    outcome
}

impl NiruampApp {
    pub(crate) fn panel_ui(&mut self, ui: &mut Ui, ctx: &Context) {
        let p = self.palette();
        let scale = self.prefs.scale;
        let s = move |v: f32| v * scale;
        let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(rect, egui::Rounding::same(3.0), p.panel);
        painter.rect_stroke(
            rect,
            egui::Rounding::same(3.0),
            Stroke::new(1.0_f32, p.border),
        );
        painter.line_segment(
            [rect.left_top(), rect.right_top()],
            Stroke::new(1.0_f32, p.border_hi),
        );
        let inner = rect.shrink(s(6.0));
        let header_h = s(24.0);
        let header = Rect::from_min_size(inner.min, vec2(inner.width(), header_h));
        let body = Rect::from_min_max(
            pos2(inner.left(), header.bottom() + s(4.0)),
            pos2(inner.right(), inner.bottom() - s(16.0)),
        );
        let footer = Rect::from_min_max(pos2(inner.left(), inner.bottom() - s(14.0)), inner.max);
        self.panel_header(ui, ctx, header);
        ui.painter().line_segment(
            [
                pos2(inner.left(), header.bottom()),
                pos2(inner.right(), header.bottom()),
            ],
            Stroke::new(1.0_f32, p.border),
        );
        let mut child = ui.child_ui(body, egui::Layout::top_down(egui::Align::Min), None);
        child.set_clip_rect(body);
        match self.tab {
            Tab::Playlist => self.playlist_body(&mut child, ctx, false),
            Tab::Queue => self.playlist_body(&mut child, ctx, true),
            Tab::Browse => self.browser_ui(&mut child, ctx),
            Tab::Settings => self.settings_ui(&mut child, ctx),
            Tab::About => self.about_ui(&mut child, ctx),
        }
        let hint =
            "SPACE play/pause · ←/→ seek · N/P track · +/- volume · / search · B browse · ESC back";
        ui.painter().text(
            footer.left_center(),
            Align2::LEFT_CENTER,
            hint,
            FontId::proportional(s(8.0)),
            p.disabled,
        );
        let right = if let Some(b) = self.backend.as_ref() {
            format!(
                "NPLAY {} · protocol {} · r{}",
                b.snapshot.backend.version, b.snapshot.protocol_version, self.revision
            )
        } else {
            "NPLAY offline".to_string()
        };
        ui.painter().text(
            footer.right_center(),
            Align2::RIGHT_CENTER,
            right,
            FontId::proportional(s(8.0)),
            p.disabled,
        );
    }

    fn panel_header(&mut self, ui: &mut Ui, ctx: &Context, rect: Rect) {
        let p = self.palette();
        let scale = self.prefs.scale;
        let s = move |v: f32| v * scale;
        let mut x = rect.left();
        let tabs = [
            (Tab::Playlist, "PLAYLIST"),
            (Tab::Queue, "QUEUE"),
            (Tab::Browse, "BROWSE"),
            (Tab::Settings, "SETTINGS"),
            (Tab::About, "ABOUT"),
        ];
        for (tab, label) in tabs {
            let w = s(13.0) + label.chars().count() as f32 * s(6.3);
            let r = Rect::from_min_size(pos2(x, rect.top()), vec2(w, rect.height()));
            let resp = ui.interact(r, Id::new(("panel-tab", tab.key())), Sense::click());
            resp.draw_focus(ui.painter(), r, &p);
            let active = self.tab == tab;
            if active {
                ui.painter().rect_filled(
                    Rect::from_min_max(pos2(r.left(), r.bottom() - s(2.0)), r.right_bottom()),
                    egui::Rounding::ZERO,
                    p.accent,
                );
            }
            ui.painter().text(
                r.center(),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(s(9.5)),
                if active { p.foreground } else { p.muted },
            );
            if resp.activated(ui) {
                self.show_tab(tab, ctx);
            }
            x += w + s(4.0);
        }
        let btn = s(18.0);
        let mut bx = rect.right() - btn;
        let actions = [
            (Icon::Refresh, "Refresh current view"),
            (Icon::Add, "New playlist"),
            (Icon::Search, "Focus search"),
        ];
        for (icon, tip) in actions {
            let r = Rect::from_center_size(pos2(bx - btn / 2.0, rect.center().y), vec2(btn, btn));
            let resp = icon_button_rect(
                ui,
                Id::new(("panel-action", icon as u8)),
                r,
                &p,
                icon,
                false,
                true,
                tip,
            );
            if resp.activated(ui) {
                match icon {
                    Icon::Refresh => match self.tab {
                        Tab::Browse => self.default_browse(),
                        Tab::Settings => self.load_settings(),
                        _ => self.request("state.snapshot", serde_json::json!({}), None),
                    },
                    Icon::Add => {
                        self.modal = Some(crate::app::Modal::NewPlaylist {
                            name: String::new(),
                        })
                    }
                    Icon::Search => {
                        self.browser.focus_search = true;
                        self.show_tab(Tab::Browse, ctx);
                    }
                    _ => {}
                }
            }
            bx -= btn + s(2.0);
        }
    }

    fn playlist_body(&mut self, ui: &mut Ui, ctx: &Context, queue: bool) {
        let p = self.palette();
        let scale = self.prefs.scale;
        let s = move |v: f32| v * scale;
        let tracks = if queue {
            self.playback.queue.clone()
        } else {
            self.playback.context.clone()
        };
        let selected = if queue {
            self.queue_selected
        } else {
            self.playlist_selected
        };
        let total: f64 = tracks.iter().map(|t| t.duration).sum();
        let header = Rect::from_min_size(ui.cursor().min, vec2(ui.available_width(), s(22.0)));
        let title = if queue { "QUEUE" } else { "PLAYLIST" };
        ui.painter().text(
            pos2(header.left() + s(2.0), header.center().y),
            Align2::LEFT_CENTER,
            format!(
                "{title} · {} TRACKS · {}",
                tracks.len(),
                fmt_duration(total)
            ),
            FontId::proportional(s(9.0)),
            p.muted,
        );
        let btn_w = s(48.0);
        let btn_h = s(18.0);
        let mut bx = header.right() - btn_w;
        let play_label = "PLAY";
        if small_button(
            ui,
            Rect::from_center_size(
                pos2(bx + btn_w / 2.0, header.center().y),
                vec2(btn_w, btn_h),
            ),
            play_label,
            &p,
        ) {
            if queue {
                if !tracks.is_empty() {
                    let idx = selected.min(tracks.len() - 1);
                    self.play_queue_index(idx);
                }
            } else if !tracks.is_empty() {
                self.play_group(&tracks, selected.min(tracks.len() - 1));
            }
        }
        bx -= btn_w + s(3.0);
        if small_button(
            ui,
            Rect::from_center_size(
                pos2(bx + btn_w / 2.0, header.center().y),
                vec2(btn_w, btn_h),
            ),
            "QUEUE",
            &p,
        ) {
            if let Some(t) = tracks.get(selected) {
                self.queue_add(t, false);
            }
        }
        bx -= btn_w + s(3.0);
        if queue {
            if small_button(
                ui,
                Rect::from_center_size(
                    pos2(bx + btn_w / 2.0, header.center().y),
                    vec2(btn_w, btn_h),
                ),
                "SAVE",
                &p,
            ) {
                self.save_queue();
            }
            bx -= btn_w + s(3.0);
            if small_button(
                ui,
                Rect::from_center_size(
                    pos2(bx + btn_w / 2.0, header.center().y),
                    vec2(btn_w, btn_h),
                ),
                "CLEAR",
                &p,
            ) {
                self.clear_queue();
            }
            bx -= btn_w + s(3.0);
            if small_button(
                ui,
                Rect::from_center_size(
                    pos2(bx + btn_w / 2.0, header.center().y),
                    vec2(btn_w, btn_h),
                ),
                "REMOVE",
                &p,
            ) && selected < tracks.len()
            {
                self.remove_queue(selected);
            }
        }
        let list_rect = Rect::from_min_max(
            pos2(ui.cursor().min.x, header.bottom() + s(3.0)),
            ui.max_rect().max,
        );
        let mut list_ui = ui.child_ui(list_rect, egui::Layout::top_down(egui::Align::Min), None);
        if tracks.is_empty() {
            list_ui.add_space(s(20.0));
            list_ui.label(
                RichText::new(if queue {
                    "Queue is empty. Right-click a track to queue it."
                } else {
                    "No playback context yet. Browse or search for music."
                })
                .color(p.disabled)
                .size(s(11.0)),
            );
            return;
        }
        let row_h = s(22.0);
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show_rows(&mut list_ui, row_h, tracks.len(), |ui, range| {
                for i in range {
                    let t = &tracks[i];
                    let current = self.playback.current_key() == Some(t.key.as_str());
                    let outcome = track_row(
                        self,
                        ui,
                        t,
                        &format!("{:>2}", i + 1),
                        i == selected,
                        current,
                        row_h,
                        true,
                        queue,
                    );
                    if outcome.drag_started {
                        self.queue_drag = Some(i);
                        self.queue_selected = i;
                    }
                    if queue && self.queue_drag.is_some() && outcome.hovered {
                        if let Some(from) = self.queue_drag {
                            if ui.input(|i| i.pointer.any_released()) && from != i {
                                self.reorder_queue(from, i);
                                self.queue_drag = None;
                            }
                        }
                        if self.queue_drag.is_some() {
                            let y = outcome.rect.top();
                            ui.painter().line_segment(
                                [
                                    egui::pos2(outcome.rect.left(), y),
                                    egui::pos2(outcome.rect.right(), y),
                                ],
                                Stroke::new(2.0_f32, p.accent),
                            );
                        }
                    }
                    if outcome.clicked {
                        if queue {
                            self.queue_selected = i;
                        } else {
                            self.playlist_selected = i;
                        }
                    }
                    if self.keyboard_nav && i == selected {
                        ui.scroll_to_rect(outcome.rect, Some(egui::Align::Center));
                    }
                    if let Some(action) = outcome.action {
                        self.apply_row_action(action, &tracks, i, queue, ctx);
                    }
                }
            });
        if self.queue_drag.is_some() {
            if ctx.input(|i| i.pointer.any_released()) {
                self.queue_drag = None;
            } else {
                ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
            }
        }
        self.keyboard_nav = false;
    }

    pub(crate) fn apply_row_action(
        &mut self,
        action: RowAction,
        tracks: &[Track],
        index: usize,
        queue: bool,
        ctx: &Context,
    ) {
        let track = tracks.get(index).cloned();
        match action {
            RowAction::Play => {
                if queue {
                    self.play_queue_index(index);
                } else if !tracks.is_empty() {
                    self.play_group(tracks, index);
                }
            }
            RowAction::PlayNext => {
                if let Some(t) = track {
                    self.queue_add(&t, true);
                }
            }
            RowAction::Queue => {
                if let Some(t) = track {
                    self.queue_add(&t, false);
                }
            }
            RowAction::Favorite => {
                if let Some(t) = track {
                    self.favorite(&t);
                }
            }
            RowAction::AddToPlaylist(id, name) => {
                if let Some(t) = track {
                    self.add_to_playlist(&t, id, &name);
                }
            }
            RowAction::NewPlaylist => {
                self.modal = Some(crate::app::Modal::NewPlaylist {
                    name: String::new(),
                });
            }
            RowAction::RemoveQueue => {
                if queue {
                    self.remove_queue(index);
                }
            }
            RowAction::MoveUp => {
                if queue {
                    self.move_queue(index, -1);
                }
            }
            RowAction::MoveDown => {
                if queue {
                    self.move_queue(index, 1);
                }
            }
            RowAction::Info => {
                if let Some(t) = track {
                    let mut bits = vec![
                        t.title.clone(),
                        t.artist.clone(),
                        t.album.clone(),
                        t.source_label().to_string(),
                    ];
                    if let Some(y) = t.year() {
                        bits.push(y.to_string());
                    }
                    if let Some(b) = t.bitrate() {
                        bits.push(format!("{b} kbps"));
                    }
                    let text = bits
                        .into_iter()
                        .filter(|x| !x.is_empty())
                        .collect::<Vec<_>>()
                        .join(" · ");
                    self.toast(text, false);
                }
            }
        }
        let _ = ctx;
    }
}

fn small_button(ui: &mut Ui, rect: Rect, label: &str, p: &crate::theme::Palette) -> bool {
    let resp = ui.interact(
        rect,
        Id::new(("small-btn", label, rect.min.x as i32)),
        Sense::click(),
    );
    let fill = if resp.is_pointer_button_down_on() {
        p.inset
    } else if resp.hovered() {
        p.surface_hi
    } else {
        p.surface
    };
    bevel_rect(ui.painter(), rect, fill, p);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(rect.height() * 0.48),
        if resp.hovered() {
            p.foreground
        } else {
            p.muted
        },
    );
    resp.activated(ui)
}
