//! Async search and source browsing.
use crate::app::NiruampApp;
use crate::views::common::*;
use crate::views::playlist::track_row;
use egui::{pos2, vec2, Align2, Context, FontId, Frame, Id, Rect, RichText, Sense, Stroke, Ui};
use std::time::{Duration, Instant};

const SOURCES: &[(&str, &str)] = &[
    ("all", "ALL"),
    ("local", "LOCAL"),
    ("navidrome", "NAVIDROME"),
    ("spotify", "SPOTIFY"),
    ("sr", "SVERIGES RADIO"),
    ("youtube", "YOUTUBE"),
    ("library", "LIBRARY"),
    ("radio", "RADIO"),
];

fn sections(source: &str) -> &'static [(&'static str, &'static str)] {
    match source {
        "local" => &[
            ("all", "ALL"),
            ("artists", "ARTISTS"),
            ("albums", "ALBUMS"),
            ("recent", "RECENT"),
            ("random", "RANDOM"),
            ("folders", "FOLDERS"),
        ],
        "navidrome" => &[
            ("newest", "RECENT"),
            ("albums", "ALBUMS"),
            ("artists", "ARTISTS"),
            ("playlists", "PLAYLISTS"),
            ("random_tracks", "RANDOM"),
        ],
        "spotify" => &[
            ("playlists", "PLAYLISTS"),
            ("top_tracks", "TOP TRACKS"),
            ("top_artists", "TOP ARTISTS"),
            ("recent", "RECENT"),
            ("devices", "DEVICES"),
        ],
        "sr" => &[("live", "LIVE"), ("programs", "PROGRAMMES")],
        "library" => &[
            ("playlists", "PLAYLISTS"),
            ("favorites", "FAVORITES"),
            ("history", "HISTORY"),
            ("discoveries", "DISCOVERIES"),
            ("bookmarks", "BOOKMARKS"),
        ],
        "radio" => &[("all", "STATIONS")],
        _ => &[],
    }
}

fn default_section(source: &str) -> &'static str {
    sections(source).first().map(|(k, _)| *k).unwrap_or("all")
}

impl NiruampApp {
    pub(crate) fn browser_ui(&mut self, ui: &mut Ui, ctx: &Context) {
        let p = self.palette();
        let scale = self.prefs.scale;
        let s = move |v: f32| v * scale;
        let width = ui.available_width();
        let search_h = s(26.0);
        let bar = Rect::from_min_size(ui.cursor().min, vec2(width, search_h));
        let back_w = s(24.0);
        let mut bx = bar.left();
        if small_icon(
            ui,
            Rect::from_center_size(
                pos2(bx + back_w / 2.0, bar.center().y),
                vec2(back_w, s(20.0)),
            ),
            Icon::Back,
            "Back",
            &p,
        ) {
            if !self.browser.input.is_empty() {
                self.browser.input.clear();
                self.browser.groups.clear();
                self.browser.items.clear();
                self.browser.title = "Browse".into();
            } else if let Some((source, section, title)) = self.browser.history.pop() {
                self.browser.title = title;
                self.browse(&source, &section, "", serde_json::json!({}));
            }
        }
        bx += back_w + s(3.0);
        let search_rect = Rect::from_min_max(
            pos2(bx, bar.top()),
            pos2(bar.right() - s(28.0), bar.bottom()),
        );
        Frame::none()
            .fill(p.inset)
            .stroke(Stroke::new(1.0_f32, p.border))
            .inner_margin(egui::Margin::symmetric(6.0, 3.0))
            .show(ui, |ui| {
                ui.allocate_ui_at_rect(search_rect, |ui| {
                    ui.horizontal_centered(|ui| {
                        ui.label(RichText::new("SEARCH").size(s(8.0)).color(p.muted));
                        let response = ui.add(
                            egui::TextEdit::singleline(&mut self.browser.input)
                                .hint_text("artists, albums, tracks, radio, streams")
                                .frame(false)
                                .desired_width(f32::INFINITY),
                        );
                        if self.browser.focus_search {
                            response.request_focus();
                            self.browser.focus_search = false;
                        }
                        if response.changed() {
                            self.browser.search_due =
                                Some(Instant::now() + Duration::from_millis(350));
                        }
                        let enter =
                            response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        if enter
                            || (response.has_focus()
                                && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                        {
                            self.browser.search_due = None;
                            self.start_search();
                        }
                    });
                });
            });
        let go_rect = Rect::from_min_size(
            pos2(bar.right() - s(26.0), bar.top() + s(2.0)),
            vec2(s(26.0), s(22.0)),
        );
        if small_icon(ui, go_rect, Icon::Search, "Search", &p) {
            self.browser.search_due = None;
            self.start_search();
        }
        ui.add_space(search_h + s(4.0));

        // Source selector.
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = s(3.0);
            for (source, label) in SOURCES {
                let active = self.browser.source == *source;
                let text = RichText::new(*label).size(s(8.0)).color(if active {
                    p.accent
                } else {
                    p.muted
                });
                let resp = ui.selectable_label(active, text);
                if resp.clicked() {
                    self.select_source(source);
                }
            }
        });
        if let Some(note) = self.source_note() {
            ui.label(RichText::new(note).size(s(9.0)).color(p.warning));
        }
        let secs = sections(&self.browser.source);
        if !secs.is_empty() && self.browser.input.trim().is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = s(3.0);
                for (section, label) in secs {
                    let active = self.browser.section == *section;
                    let resp = ui.selectable_label(
                        active,
                        RichText::new(*label).size(s(8.0)).color(if active {
                            p.accent
                        } else {
                            p.muted
                        }),
                    );
                    if resp.clicked() {
                        let source = self.browser.source.clone();
                        self.browse(&source, section, "", serde_json::json!({}));
                    }
                }
                if self.browser.source == "local" {
                    if ui
                        .selectable_label(false, RichText::new("SCAN").size(s(8.0)).color(p.muted))
                        .clicked()
                    {
                        self.request(
                            "library.scan",
                            serde_json::json!({}),
                            Some(crate::app::PendingKind::Generic {
                                label: "Library scan started".into(),
                                quiet: false,
                            }),
                        );
                    }
                }
                if self.browser.source == "radio" {
                    if ui
                        .selectable_label(
                            false,
                            RichText::new("ADD STATION").size(s(8.0)).color(p.muted),
                        )
                        .clicked()
                    {
                        self.modal = Some(crate::app::Modal::AddRadio {
                            name: String::new(),
                            url: String::new(),
                        });
                    }
                }
            });
        }
        ui.add_space(s(4.0));

        if self.browser.loading {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(
                    RichText::new(if self.browser.groups.is_empty() {
                        "LOADING FROM NPLAY…"
                    } else {
                        "SEARCHING…"
                    })
                    .size(s(9.0))
                    .color(p.muted),
                );
            });
        }
        if !self.browser.error.is_empty() {
            ui.label(
                RichText::new(&self.browser.error)
                    .size(s(10.0))
                    .color(p.error),
            );
        }
        if self.browser.groups.is_empty() && self.browser.items.is_empty() {
            self.browser_landing(ui, ctx);
            return;
        }
        let header_h = s(20.0);
        if !self.browser.items.is_empty() && self.browser.groups.is_empty() {
            let header = Rect::from_min_size(ui.cursor().min, vec2(ui.available_width(), header_h));
            ui.painter().text(
                pos2(header.left() + s(2.0), header.center().y),
                Align2::LEFT_CENTER,
                &self.browser.title,
                FontId::proportional(s(9.0)),
                p.muted,
            );
            let playable = crate::app::playable_items(&self.browser.items);
            if !playable.is_empty() {
                let mut ax = header.right() - s(50.0);
                if mini_action(
                    ui,
                    Rect::from_center_size(pos2(ax, header.center().y), vec2(s(46.0), s(16.0))),
                    "PLAY ALL",
                    &p,
                ) {
                    self.play_group(&playable, 0);
                }
                ax -= s(52.0);
                if mini_action(
                    ui,
                    Rect::from_center_size(pos2(ax, header.center().y), vec2(s(46.0), s(16.0))),
                    "QUEUE ALL",
                    &p,
                ) {
                    self.queue_add_many(&playable, false);
                }
            }
            ui.add_space(header_h + s(2.0));
        }
        let row_h = s(22.0);
        let scroll = Rect::from_min_max(ui.cursor().min, ui.max_rect().max);
        let mut scroll_ui = ui.child_ui(scroll, egui::Layout::top_down(egui::Align::Min), None);
        if !self.browser.groups.is_empty() {
            #[derive(Clone, Copy)]
            enum SearchRow {
                Header(usize),
                Track {
                    group: usize,
                    item: usize,
                    index: usize,
                },
            }
            let groups = self.browser.groups.clone();
            let mut rows = Vec::new();
            let mut index = 0usize;
            for (gi, group) in groups.iter().enumerate() {
                rows.push(SearchRow::Header(gi));
                for (ii, _) in group.items.iter().enumerate() {
                    rows.push(SearchRow::Track {
                        group: gi,
                        item: ii,
                        index,
                    });
                    index += 1;
                }
            }
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show_rows(&mut scroll_ui, row_h, rows.len(), |ui, range| {
                    for row_index in range {
                        match rows[row_index] {
                            SearchRow::Header(gi) => {
                                let group = &groups[gi];
                                let (header, _) = ui.allocate_exact_size(
                                    vec2(ui.available_width(), row_h),
                                    Sense::hover(),
                                );
                                let suffix = if group.items.is_empty() {
                                    " · no matches"
                                } else {
                                    ""
                                };
                                let label = format!(
                                    "{} · {}{}",
                                    group.label.to_uppercase(),
                                    group.items.len(),
                                    suffix
                                );
                                ui.painter().text(
                                    pos2(header.left() + s(2.0), header.center().y),
                                    Align2::LEFT_CENTER,
                                    label,
                                    FontId::proportional(s(9.0)),
                                    p.muted,
                                );
                                let playable = crate::app::playable_items(&group.items);
                                if !playable.is_empty() {
                                    let mut ax = header.right() - s(50.0);
                                    if mini_action(
                                        ui,
                                        Rect::from_center_size(
                                            pos2(ax, header.center().y),
                                            vec2(s(46.0), s(16.0)),
                                        ),
                                        "PLAY ALL",
                                        &p,
                                    ) {
                                        self.play_group(&playable, 0);
                                    }
                                    ax -= s(52.0);
                                    if mini_action(
                                        ui,
                                        Rect::from_center_size(
                                            pos2(ax, header.center().y),
                                            vec2(s(46.0), s(16.0)),
                                        ),
                                        "QUEUE ALL",
                                        &p,
                                    ) {
                                        self.queue_add_many(&playable, false);
                                    }
                                }
                            }
                            SearchRow::Track { group, item, index } => {
                                let group = &groups[group];
                                let track = &group.items[item];
                                let selected = self.browser.selected == index;
                                let current =
                                    self.playback.current_key() == Some(track.key.as_str());
                                let outcome = track_row(
                                    self,
                                    ui,
                                    track,
                                    &format!("{:>2}", item + 1),
                                    selected,
                                    current,
                                    row_h,
                                    true,
                                    false,
                                );
                                if outcome.clicked {
                                    self.browser.selected = index;
                                }
                                if self.keyboard_nav && selected {
                                    ui.scroll_to_rect(outcome.rect, Some(egui::Align::Center));
                                }
                                if let Some(action) = outcome.action {
                                    self.apply_row_action(action, &group.items, item, false, ctx);
                                }
                            }
                        }
                    }
                });
        } else {
            let tracks = self.browser.items.clone();
            let total = tracks.len();
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show_rows(&mut scroll_ui, row_h, total, |ui, range| {
                    for i in range {
                        let track = &tracks[i];
                        let selected = self.browser.selected == i;
                        let current = self.playback.current_key() == Some(track.key.as_str());
                        let outcome = track_row(
                            self,
                            ui,
                            track,
                            &format!("{:>2}", i + 1),
                            selected,
                            current,
                            row_h,
                            true,
                            false,
                        );
                        if outcome.clicked {
                            self.browser.selected = i;
                        }
                        if self.keyboard_nav && selected {
                            ui.scroll_to_rect(outcome.rect, Some(egui::Align::Center));
                        }
                        if let Some(action) = outcome.action {
                            self.apply_row_action(action, &tracks, i, false, ctx);
                        }
                    }
                });
        }
        self.keyboard_nav = false;
    }

    fn source_note(&self) -> Option<String> {
        let info = self.sources.iter().find(|s| s.id == self.browser.source)?;
        if !info.enabled {
            return Some(format!("{} is disabled in NPLAY settings.", info.label));
        }
        if !info.configured
            && matches!(
                self.browser.source.as_str(),
                "navidrome" | "spotify" | "youtube" | "radio"
            )
        {
            return Some(format!(
                "{} is not configured yet. Open Settings for setup details.",
                info.label
            ));
        }
        None
    }

    fn select_source(&mut self, source: &str) {
        self.browser.source = source.into();
        self.browser.groups.clear();
        self.browser.items.clear();
        self.browser.title = "Browse".into();
        self.browser.selected = 0;
        if source == "all" {
            if !self.browser.input.trim().is_empty() {
                self.start_search_source(Vec::new());
            }
            return;
        }
        if matches!(
            source,
            "local" | "navidrome" | "spotify" | "sr" | "library" | "radio"
        ) && !self.browser.input.trim().is_empty()
        {
            self.start_search_source(vec![source.to_string()]);
            return;
        }
        if source == "youtube" {
            self.browser.section = "search".into();
            self.browser.focus_search = true;
            return;
        }
        let section = default_section(source);
        self.browse(source, section, "", serde_json::json!({}));
    }

    fn browser_landing(&mut self, ui: &mut Ui, _ctx: &Context) {
        let p = self.palette();
        let scale = self.prefs.scale;
        let s = move |v: f32| v * scale;
        ui.add_space(s(8.0));
        ui.label(
            RichText::new("SEARCH ALL SOURCES")
                .size(s(13.0))
                .strong()
                .color(p.foreground),
        );
        ui.label(
            RichText::new("Local library, Navidrome, Spotify, Sveriges Radio, YouTube and custom radio. Search is asynchronous and never blocks the deck.")
                .size(s(10.0))
                .color(p.muted),
        );
        ui.add_space(s(10.0));
        let cards = [
            (
                "LOCAL LIBRARY",
                "Tracks, artists, albums, folders",
                "local",
                "all",
            ),
            (
                "PLAYLISTS",
                "Cross-source local playlists",
                "library",
                "playlists",
            ),
            (
                "FAVORITES",
                "Saved across providers",
                "library",
                "favorites",
            ),
            (
                "RADIO",
                "Live, podcasts and custom stations",
                "radio",
                "all",
            ),
            ("HISTORY", "Recently played", "library", "history"),
            (
                "NAVIDROME",
                "Your Subsonic-compatible server",
                "navidrome",
                "newest",
            ),
        ];
        let mut clicked = None;
        ui.horizontal_wrapped(|ui| {
            for (title, subtitle, source, section) in cards {
                let card = vec2(s(168.0), s(46.0));
                let (rect, resp) = ui.allocate_exact_size(card, Sense::click());
                let fill = if resp.hovered() {
                    p.surface_hi
                } else {
                    p.surface
                };
                bevel_rect(ui.painter(), rect, fill, &p);
                ui.painter().text(
                    rect.left_top() + vec2(s(7.0), s(7.0)),
                    Align2::LEFT_TOP,
                    title,
                    FontId::proportional(s(10.0)),
                    p.foreground,
                );
                ui.painter().text(
                    rect.left_bottom() + vec2(s(7.0), -s(7.0)),
                    Align2::LEFT_BOTTOM,
                    subtitle,
                    FontId::proportional(s(8.0)),
                    p.muted,
                );
                resp.draw_focus(ui.painter(), rect, &p);
                if resp.activated(ui) {
                    clicked = Some((source, section));
                }
            }
        });
        if let Some((source, section)) = clicked {
            self.browse(source, section, "", serde_json::json!({}));
        }
        ui.add_space(s(8.0));
        ui.label(
            RichText::new(
                "Tip: right-click any track for queue, playlist, favorite and detail actions.",
            )
            .size(s(9.0))
            .color(p.disabled),
        );
    }
}

fn small_icon(ui: &mut Ui, rect: Rect, icon: Icon, tip: &str, p: &crate::theme::Palette) -> bool {
    let resp = ui.interact(
        rect,
        Id::new(("browser-icon", icon as u8, rect.min.x as i32)),
        Sense::click(),
    );
    let fill = if resp.hovered() {
        p.surface_hi
    } else {
        p.inset
    };
    bevel_rect(ui.painter(), rect, fill, p);
    draw_icon(
        ui.painter(),
        icon,
        rect.shrink(rect.height() * 0.24),
        if resp.hovered() {
            p.foreground
        } else {
            p.muted
        },
    );
    resp.draw_focus(ui.painter(), rect, p);
    let clicked = resp.activated(ui);
    let _ = resp.on_hover_text(tip);
    clicked
}

fn mini_action(ui: &mut Ui, rect: Rect, label: &str, p: &crate::theme::Palette) -> bool {
    let resp = ui.interact(
        rect,
        Id::new(("mini", label, rect.min.x as i32, rect.min.y as i32)),
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
    resp.draw_focus(ui.painter(), rect, p);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(rect.height() * 0.46),
        if resp.hovered() { p.accent } else { p.muted },
    );
    resp.activated(ui)
}
