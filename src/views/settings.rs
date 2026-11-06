//! Settings, source configuration and honest product information.
use crate::app::{Modal, NiruampApp, PendingKind};
use crate::config;
use crate::theme;
use crate::views::common::*;
use egui::{pos2, vec2, Align2, Context, FontId, Frame, RichText, Sense, Stroke, Ui};
use serde_json::{json, Value};

impl NiruampApp {
    pub(crate) fn settings_ui(&mut self, ui: &mut Ui, ctx: &Context) {
        let p = self.palette();
        let scale = self.prefs.scale;
        let s = move |v: f32| v * scale;
        if !self.settings.loaded && !self.settings.loading {
            self.load_settings();
        }
        if !self.settings.error.is_empty() {
            ui.label(
                RichText::new(&self.settings.error)
                    .color(p.error)
                    .size(s(10.0)),
            );
            if ui.button("RETRY").clicked() {
                self.settings.loaded = false;
                self.load_settings();
            }
        }
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(s(2.0));
                self.settings_appearance(ui, ctx);
                ui.add_space(s(10.0));
                self.settings_visualization(ui, ctx);
                ui.add_space(s(10.0));
                self.settings_playback(ui, ctx);
                ui.add_space(s(10.0));
                self.settings_sources(ui, ctx);
                ui.add_space(s(10.0));
                self.settings_library(ui, ctx);
                ui.add_space(s(10.0));
                self.settings_system(ui, ctx);
                ui.add_space(s(12.0));
            });
    }

    fn setting_frame<R>(
        p: &crate::theme::Palette,
        scale: f32,
        ui: &mut Ui,
        title: &str,
        add: impl FnOnce(&mut Ui) -> R,
    ) -> R {
        let s = move |v: f32| v * scale;
        let mut out = None;
        Frame::none()
            .fill(p.surface)
            .stroke(Stroke::new(1.0_f32, p.border))
            .inner_margin(egui::Margin::same(10.0))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.label(RichText::new(title).size(s(9.0)).strong().color(p.accent));
                ui.add_space(s(4.0));
                out = Some(add(ui));
            });
        out.unwrap()
    }

    fn setting_bool(&self, key: &str, default: bool) -> bool {
        self.settings
            .values
            .settings
            .get(key)
            .and_then(Value::as_bool)
            .unwrap_or(default)
    }

    fn set_setting(&mut self, key: &str, value: Value, label: &str) {
        if let Ok(mut map) = serde_json::from_value::<std::collections::HashMap<String, Value>>(
            json!({key: value.clone()}),
        ) {
            for (k, v) in map.drain() {
                self.settings.values.settings.insert(k, v);
            }
        }
        self.request(
            "settings.set",
            json!({"settings": {key: value}}),
            Some(PendingKind::Generic {
                label: label.to_string(),
                quiet: false,
            }),
        );
    }

    fn settings_appearance(&mut self, ui: &mut Ui, ctx: &Context) {
        let p = self.palette();
        let scale = self.prefs.scale;
        let s = move |v: f32| v * scale;
        Self::setting_frame(&p, scale, ui, "APPEARANCE", |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Theme").size(s(11.0)).color(p.foreground));
                let mut selected = self.prefs.theme.clone();
                // Inside NIRU (and wherever no Omarchy theme exists) the
                // labels are the 0.4 ones.
                let omarchy = crate::omarchy::current().is_some() && !crate::session::niru();
                let auto_label = if omarchy {
                    "Auto · follow Omarchy"
                } else {
                    "Auto · follow NIRUX"
                };
                egui::ComboBox::from_id_salt("theme-select")
                    .selected_text(match selected.as_str() {
                        "auto" => auto_label.to_string(),
                        "classic" => "NIRUAMP Classic".to_string(),
                        "nirux" => "NIRUX · current theme".to_string(),
                        "omarchy" => "Omarchy · current theme".to_string(),
                        _ => self.theme.label.clone(),
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut selected, "auto".into(), auto_label)
                            .clicked();
                        ui.selectable_value(
                            &mut selected,
                            "classic".into(),
                            "NIRUAMP Classic · standalone",
                        )
                        .clicked();
                        if omarchy || self.prefs.theme == "omarchy" {
                            ui.selectable_value(
                                &mut selected,
                                "omarchy".into(),
                                "Omarchy · current theme",
                            )
                            .clicked();
                            ui.selectable_value(
                                &mut selected,
                                "nirux".into(),
                                "NIRUX · current theme",
                            )
                            .clicked();
                        }
                        for (key, label) in theme::available_themes() {
                            if ui
                                .selectable_value(&mut selected, key.clone(), label)
                                .clicked()
                            {}
                        }
                    });
                if selected != self.prefs.theme {
                    self.prefs.theme = selected;
                    self.reload_theme(ctx);
                }
                ui.label(
                    RichText::new(&self.theme.source)
                        .size(s(9.0))
                        .color(p.muted),
                );
            });
            ui.horizontal(|ui| {
                ui.label(RichText::new("Scale").size(s(11.0)).color(p.foreground));
                let mut scale = self.prefs.scale;
                let resp = ui.add(
                    egui::Slider::new(&mut scale, 0.75..=1.6)
                        .step_by(0.05)
                        .fixed_decimals(2),
                );
                if resp.changed() {
                    self.prefs.scale = scale;
                    theme::install_style(ctx, &self.theme, &self.prefs);
                    self.update_prefs(ctx);
                }
                ui.label(
                    RichText::new(format!("{:.0}%", self.prefs.scale * 100.0))
                        .size(s(9.0))
                        .color(p.muted),
                );
            });
            ui.horizontal(|ui| {
                let mut top = self.prefs.always_on_top;
                // Wayland leaves stacking to the compositor; NIRUAMP can ask
                // Hyprland (pin), not other compositors.
                let stacking = self.hypr.is_some()
                    || crate::session::backend() == crate::session::Backend::X11;
                let resp = ui.add_enabled(
                    stacking,
                    egui::Checkbox::new(&mut top, RichText::new("Always on top").size(s(11.0))),
                );
                let resp = if stacking {
                    resp
                } else {
                    resp.on_disabled_hover_text(
                        "This Wayland compositor decides stacking itself. Use a compositor \
                         rule, or NIRUAMP_BACKEND=x11 to run through XWayland.",
                    )
                };
                if resp.changed() {
                    self.toggle_pin(ctx);
                }
                let mut visual = self.prefs.show_visualization;
                if ui
                    .checkbox(&mut visual, RichText::new("Visualizer").size(s(11.0)))
                    .changed()
                {
                    self.prefs.show_visualization = visual;
                    if visual {
                        self.vis.failed.clear();
                        self.vis.retry_at = None;
                    }
                    self.update_prefs(ctx);
                }
                let mut startup_sound = self.prefs.startup_sound;
                if ui
                    .checkbox(
                        &mut startup_sound,
                        RichText::new("Startup sound").size(s(11.0)),
                    )
                    .changed()
                {
                    self.prefs.startup_sound = startup_sound;
                    self.update_prefs(ctx);
                }
                let mut remaining = self.prefs.remaining_time;
                if ui
                    .checkbox(
                        &mut remaining,
                        RichText::new("Show remaining time").size(s(11.0)),
                    )
                    .changed()
                {
                    self.prefs.remaining_time = remaining;
                    self.update_prefs(ctx);
                }
            });
            ui.horizontal(|ui| {
                ui.label(RichText::new("Motion").size(s(11.0)).color(p.foreground));
                let before = self.prefs.reduced_motion;
                let current = match self.prefs.reduced_motion {
                    Some(true) => "reduced",
                    Some(false) => "normal",
                    None => "follow",
                };
                egui::ComboBox::from_id_salt("motion-select")
                    .selected_text(match current {
                        "reduced" => "Reduced",
                        "normal" => "Normal",
                        _ => "Follow NIRUX",
                    })
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(current == "follow", "Follow NIRUX")
                            .clicked()
                        {
                            self.prefs.reduced_motion = None;
                        }
                        if ui
                            .selectable_label(current == "reduced", "Reduced")
                            .clicked()
                        {
                            self.prefs.reduced_motion = Some(true);
                        }
                        if ui.selectable_label(current == "normal", "Normal").clicked() {
                            self.prefs.reduced_motion = Some(false);
                        }
                    });
                if self.prefs.reduced_motion != before {
                    // Saved only on change: this used to rewrite (and fsync)
                    // config.toml on every frame while Settings was open.
                    self.reduce_motion = theme::reduced_motion(&self.theme, &self.prefs);
                    self.update_prefs(ctx);
                }
                let mut keep = self.prefs.keep_playing;
                if ui
                    .checkbox(
                        &mut keep,
                        RichText::new("Keep playing after GUI exit").size(s(11.0)),
                    )
                    .changed()
                {
                    self.prefs.keep_playing = keep;
                    self.update_prefs(ctx);
                    self.toast(
                        "Keep-playing applies to the next NPLAY service launch",
                        false,
                    );
                }
                let mut tray = self.prefs.tray_enabled;
                if ui
                    .checkbox(
                        &mut tray,
                        RichText::new("Minimize to system tray").size(s(11.0)),
                    )
                    .changed()
                {
                    self.set_tray_enabled(ctx, tray);
                }
                if let Some(err) = self.tray_error.as_ref() {
                    ui.label(
                        RichText::new(format!("Tray unavailable: {err}"))
                            .size(s(8.0))
                            .color(p.warning),
                    );
                }
            });
        });
    }

    fn settings_visualization(&mut self, ui: &mut Ui, ctx: &Context) {
        let p = self.palette();
        let scale = self.prefs.scale;
        let s = move |v: f32| v * scale;
        Self::setting_frame(&p, scale, ui, "VISUALIZATION", |ui| {
            ui.horizontal_wrapped(|ui| {
                let mut pause = self.prefs.pause_hidden_visualization;
                if ui
                    .checkbox(
                        &mut pause,
                        RichText::new("Pause spectrum when the window is not visible")
                            .size(s(11.0)),
                    )
                    .changed()
                {
                    self.prefs.pause_hidden_visualization = pause;
                    self.update_prefs(ctx);
                }
                ui.label(
                    RichText::new("Frame rate")
                        .size(s(11.0))
                        .color(p.foreground),
                );
                let mut fps = self.prefs.visualization_fps;
                egui::ComboBox::from_id_salt("vis-fps-select")
                    .selected_text(format!("{fps} fps"))
                    .show_ui(ui, |ui| {
                        for choice in config::VIS_FPS_CHOICES {
                            ui.selectable_value(&mut fps, choice, format!("{choice} fps"));
                        }
                    });
                if fps != self.prefs.visualization_fps {
                    self.prefs.visualization_fps = fps;
                    self.update_prefs(ctx);
                }
            });
            ui.label(
                RichText::new(
                    "Hidden: another workspace, fully covered or minimized. Lower rates save power; reduced motion uses 8 fps.",
                )
                .size(s(9.0))
                .color(p.muted),
            );
        });
    }

    fn settings_playback(&mut self, ui: &mut Ui, _ctx: &Context) {
        let p = self.palette();
        let scale = self.prefs.scale;
        let s = move |v: f32| v * scale;
        Self::setting_frame(&p, scale, ui, "PLAYBACK", |ui| {
            let toggles = [
                ("normalization", "ReplayGain / normalization", true),
                ("gapless", "Gapless playback", false),
                ("track_notifications", "Track notifications", true),
                ("notification_artwork", "Notification artwork", true),
                ("mpris_enabled", "MPRIS / media keys", true),
            ];
            ui.horizontal_wrapped(|ui| {
                for (key, label, default) in toggles {
                    let mut value = self.setting_bool(key, default);
                    if ui
                        .checkbox(&mut value, RichText::new(label).size(s(11.0)))
                        .changed()
                    {
                        self.set_setting(key, Value::Bool(value), label);
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label(RichText::new("Shuffle").size(s(11.0)).color(p.foreground));
                if ui.button(self.playback.shuffle.to_uppercase()).clicked() {
                    self.shuffle();
                }
                ui.label(RichText::new("Repeat").size(s(11.0)).color(p.foreground));
                if ui.button(self.playback.repeat.to_uppercase()).clicked() {
                    self.repeat();
                }
                ui.label(
                    RichText::new(
                        "Playback is owned by NPLAY; this window never opens a second engine.",
                    )
                    .size(s(9.0))
                    .color(p.muted),
                );
            });
        });
    }

    /// Local playback (librespot) authorization. Shown only when NPLAY
    /// advertises `spotify_local_authorize` (1.8.1+); older NPLAY keeps the
    /// 0.5.1 layout.
    fn settings_spotify_local(&mut self, ui: &mut Ui, scale: f32) {
        if !self.capabilities.spotify_local_authorize {
            return;
        }
        let p = self.palette();
        let s = move |v: f32| v * scale;
        let status = &self.settings.values.spotify;
        let available = self.capabilities.spotify_local || status.local_available;
        let authorized = self.capabilities.spotify_local_authorized;
        let authorizing = self.spotify_local_authorizing || status.local_authorizing;
        ui.horizontal(|ui| {
            ui.add_space(s(22.0));
            ui.label(
                RichText::new("Local playback")
                    .size(s(10.0))
                    .color(p.foreground),
            );
            if !available {
                ui.label(
                    RichText::new("install librespot")
                        .size(s(8.0))
                        .color(p.warning),
                );
                return;
            }
            let (text, color) = if authorizing {
                ("authorizing · complete it in your browser", p.warning)
            } else if authorized {
                ("authorized", p.success)
            } else {
                ("not authorized", p.warning)
            };
            ui.label(RichText::new(text).size(s(9.0)).color(color));
            let label = if authorized {
                "REAUTHORIZE LOCAL"
            } else {
                "AUTHORIZE LOCAL"
            };
            let button = ui
                .add_enabled(!authorizing, egui::Button::new(label).small())
                .on_hover_text(
                    "NPLAY opens Spotify's sign-in page for its local librespot receiver. Needed once; librespot keeps the credentials.",
                );
            if button.clicked() {
                self.authorize_spotify_local();
            }
        });
    }

    fn settings_sources(&mut self, ui: &mut Ui, ctx: &Context) {
        let p = self.palette();
        let scale = self.prefs.scale;
        let s = move |v: f32| v * scale;
        Self::setting_frame(&p, scale, ui, "SOURCES", |ui| {
            let rows = [
                ("navidrome", "Navidrome / Subsonic", "navidrome_enabled"),
                ("spotify", "Spotify", "spotify_enabled"),
                ("sr", "Sveriges Radio", "sr_enabled"),
                ("youtube", "YouTube", "youtube_enabled"),
                ("radio", "Custom radio", "custom_radio_enabled"),
            ];
            for (id, label, key) in rows {
                let info = self.sources.iter().find(|s| s.id == id);
                let configured = info.map(|s| s.configured).unwrap_or(false);
                let mut enabled = self.setting_bool(key, id != "spotify");
                ui.horizontal(|ui| {
                    if ui
                        .checkbox(&mut enabled, RichText::new(label).size(s(11.0)))
                        .changed()
                    {
                        self.set_setting(key, Value::Bool(enabled), label);
                    }
                    let status = if configured {
                        "configured"
                    } else {
                        "not configured"
                    };
                    ui.label(RichText::new(status).size(s(9.0)).color(if configured {
                        p.success
                    } else {
                        p.muted
                    }));
                    if id == "navidrome" && ui.small_button("SETUP").clicked() {
                        let url = self.settings.values.navidrome.url.clone();
                        let user = self.settings.values.navidrome.user.clone();
                        self.modal = Some(Modal::Navidrome {
                            url,
                            user,
                            password: String::new(),
                        });
                    }
                    if id == "spotify" {
                        if configured {
                            if ui.small_button("DISCONNECT").clicked() {
                                self.request(
                                    "spotify.disconnect",
                                    json!({}),
                                    Some(PendingKind::Generic {
                                        label: "Spotify disconnected".into(),
                                        quiet: false,
                                    }),
                                );
                                self.load_settings();
                            }
                        } else if ui.small_button("CONNECT").clicked() {
                            self.request(
                                "spotify.connect",
                                json!({}),
                                Some(PendingKind::Generic {
                                    label: "Spotify authorization started in your browser".into(),
                                    quiet: false,
                                }),
                            );
                        }
                    }
                    if id == "youtube" && !configured {
                        ui.label(
                            RichText::new("install yt-dlp")
                                .size(s(8.0))
                                .color(p.warning),
                        );
                    }
                });
                if id == "spotify" && enabled {
                    self.settings_spotify_local(ui, scale);
                }
            }
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Preferred source")
                        .size(s(11.0))
                        .color(p.foreground),
                );
                let current = self
                    .settings
                    .values
                    .settings
                    .get("preferred_source")
                    .and_then(Value::as_str)
                    .unwrap_or("local")
                    .to_string();
                egui::ComboBox::from_id_salt("preferred-source")
                    .selected_text(current.to_uppercase())
                    .show_ui(ui, |ui| {
                        for option in ["local", "navidrome", "spotify", "youtube"] {
                            if ui
                                .selectable_label(current == option, option.to_uppercase())
                                .clicked()
                            {
                                self.set_setting(
                                    "preferred_source",
                                    Value::String(option.into()),
                                    "Preferred source updated",
                                );
                            }
                        }
                    });
                if ui.button("REFRESH").clicked() {
                    self.load_settings();
                }
                let _ = ctx;
            });
            let note = "Credentials and OAuth tokens stay in NPLAY's 0600 secrets file and never cross the control protocol.";
            ui.label(RichText::new(note).size(s(9.0)).color(p.muted));
        });
    }

    fn settings_library(&mut self, ui: &mut Ui, _ctx: &Context) {
        let p = self.palette();
        let scale = self.prefs.scale;
        let s = move |v: f32| v * scale;
        Self::setting_frame(&p, scale, ui, "LIBRARY", |ui| {
            let roots = self.settings.values.library.roots.join(" : ");
            ui.label(
                RichText::new(format!(
                    "{} indexed tracks",
                    self.settings.values.library.tracks
                ))
                .size(s(11.0))
                .color(p.foreground),
            );
            ui.label(RichText::new(roots).size(s(9.0)).color(p.muted));
            ui.horizontal(|ui| {
                if ui.button("SCAN CHANGES").clicked() {
                    self.request(
                        "library.scan",
                        json!({"force": false}),
                        Some(PendingKind::Generic {
                            label: "Library scan started".into(),
                            quiet: false,
                        }),
                    );
                }
                if ui.button("FULL RESCAN").clicked() {
                    self.request(
                        "library.scan",
                        json!({"force": true}),
                        Some(PendingKind::Generic {
                            label: "Full library rescan started".into(),
                            quiet: false,
                        }),
                    );
                }
                if ui.button("BROWSE PLAYLISTS").clicked() {
                    self.open_playlists();
                }
            });
            ui.label(RichText::new("Music folders are configured in NPLAY. The scan is incremental and preserves offline roots.").size(s(9.0)).color(p.muted));
        });
    }

    fn settings_system(&mut self, ui: &mut Ui, ctx: &Context) {
        let p = self.palette();
        let scale = self.prefs.scale;
        let s = move |v: f32| v * scale;
        Self::setting_frame(&p, scale, ui, "SYSTEM", |ui| {
            let version = self.capabilities.backend_version.clone();
            let socket = self
                .backend
                .as_ref()
                .map(|b| b.socket.display().to_string())
                .unwrap_or_else(|| "<offline>".into());
            let pid = self
                .backend
                .as_ref()
                .map(|b| b.snapshot.backend.pid)
                .unwrap_or(0);
            let rows = [
                ("NPLAY", version),
                ("Protocol", self.capabilities.protocol_version.to_string()),
                ("Socket", socket),
                ("PID", pid.to_string()),
                (
                    "mpv",
                    if self.capabilities.mpv {
                        "available"
                    } else {
                        "missing"
                    }
                    .into(),
                ),
                (
                    "CAVA",
                    if self.capabilities.cava {
                        "available"
                    } else {
                        "missing"
                    }
                    .into(),
                ),
                (
                    "System tray",
                    if self.tray.is_some() && self.prefs.tray_enabled {
                        "active"
                    } else if self.tray_error.is_some() {
                        "unavailable"
                    } else {
                        "off"
                    }
                    .into(),
                ),
                (
                    "NIRUAMP config",
                    config::config_path().display().to_string(),
                ),
                ("NPLAY log", self.settings.values.system.log.clone()),
            ];
            for (key, value) in rows {
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [s(100.0), s(14.0)],
                        egui::Label::new(RichText::new(key).size(s(9.0)).color(p.muted)),
                    );
                    ui.label(RichText::new(value).size(s(9.0)).color(p.foreground));
                });
            }
            ui.horizontal(|ui| {
                if ui.button("RETRY NPLAY CONNECTION").clicked() {
                    self.retry_backend(ctx);
                }
                if ui.button("STOP VISUALIZER").clicked() {
                    self.stop_visualization();
                    // Not stop_sent: the next visible frame starts it again,
                    // exactly as the 0.3 button did.
                    self.vis.stop_sent = false;
                    self.toast("Visualizer stopped", false);
                }
            });
            ui.label(RichText::new("Standalone `nplay`, its diagnostics, terminal UI and regression suite never require NIRUAMP or a display server.").size(s(9.0)).color(p.muted));
        });
    }

    pub(crate) fn about_ui(&mut self, ui: &mut Ui, _ctx: &Context) {
        let p = self.palette();
        let scale = self.prefs.scale;
        let s = move |v: f32| v * scale;
        ui.add_space(s(2.0));
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(s(46.0), s(46.0)), Sense::hover());
            logo(ui.painter(), rect, &p);
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("NIRUAMP")
                        .size(s(24.0))
                        .strong()
                        .color(p.foreground),
                );
                ui.label(
                    RichText::new("A Winamp-inspired Rust frontend for NPLAY, designed for NIRUX.")
                        .size(s(11.0))
                        .color(p.muted),
                );
            });
        });
        ui.add_space(s(8.0));
        Self::setting_frame(&p, scale, ui, "ARCHITECTURE", |ui| {
            let lines = [
                "Rust graphical client · original artwork and vector controls.",
                "NPLAY owns playback, queue, providers, library, statistics and MPRIS.",
                "A versioned JSON-RPC service over a private Unix socket connects both UIs.",
                "No bundled skins and no emulated .wsz compatibility in this release.",
                "Equalizer is intentionally absent: NPLAY exposes no functional EQ DSP.",
                "Visualization uses NPLAY's CAVA path and shows nothing synthetic when unavailable.",
            ];
            for line in lines {
                ui.label(
                    RichText::new(format!("• {line}"))
                        .size(s(10.0))
                        .color(p.foreground),
                );
            }
        });
        ui.add_space(s(8.0));
        Self::setting_frame(&p, scale, ui, "KEYBOARD", |ui| {
            let keys = [
                ("Space", "Play / pause"),
                ("N / P", "Next / previous"),
                ("S", "Stop"),
                ("← / →", "Seek 5s (Shift: 30s)"),
                ("+ / -", "Volume"),
                ("M", "Mute"),
                ("R / H", "Repeat / shuffle"),
                ("/ or Ctrl+F", "Search"),
                ("B", "Browse"),
                ("F", "Favorite current track"),
                ("↑ / ↓", "Move selection"),
                ("Alt+↑ / ↓", "Reorder queue"),
                ("Esc", "Back / close panel"),
            ];
            for (key, desc) in keys {
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [s(80.0), s(14.0)],
                        egui::Label::new(
                            RichText::new(key).monospace().size(s(9.0)).color(p.accent),
                        ),
                    );
                    ui.label(RichText::new(desc).size(s(10.0)).color(p.foreground));
                });
            }
        });
        ui.add_space(s(8.0));
        Self::setting_frame(&p, scale, ui, "LEGAL", |ui| {
            ui.label(RichText::new("MIT licensed. Independent project; not affiliated with Winamp. Original icons and panel artwork are drawn in code.").size(s(9.0)).color(p.muted));
            ui.label(
                RichText::new(format!(
                    "NIRUAMP {} · NPLAY {} · protocol {}",
                    env!("CARGO_PKG_VERSION"),
                    self.capabilities.backend_version,
                    self.capabilities.protocol_version
                ))
                .size(s(9.0))
                .color(p.disabled),
            );
        });
        let _ = FontId::proportional(10.0);
        let _ = Align2::LEFT_TOP;
        let _ = pos2(0.0, 0.0);
    }
}
