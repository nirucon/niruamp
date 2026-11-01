//! NIRUAMP application state, protocol orchestration and top-level layout.
use crate::config::{Preferences, WindowState};
use crate::model::*;
use crate::platform::{self, BackendHandle, DependencyError, InstanceGuard};
use crate::protocol::Incoming;
use crate::theme::{self, Palette, ResolvedTheme, ThemeEvent, ThemeWatcher};
use crate::tray::{TrayCommand, TrayHandle};
use egui::{
    Align2, Color32, Context, Frame, Id, Key, LayerId, Order, Rect, RichText, Sense, Stroke,
    TextureHandle, Ui, Vec2,
};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub(crate) const APP_TITLE: &str = "NIRUAMP";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tab {
    Playlist,
    Queue,
    Browse,
    Settings,
    About,
}

impl Tab {
    pub(crate) fn key(self) -> &'static str {
        match self {
            Tab::Playlist => "playlist",
            Tab::Queue => "queue",
            Tab::Browse => "browse",
            Tab::Settings => "settings",
            Tab::About => "about",
        }
    }
    fn from_key(s: &str) -> Self {
        match s {
            "queue" => Tab::Queue,
            "browse" => Tab::Browse,
            "settings" => Tab::Settings,
            "about" => Tab::About,
            _ => Tab::Playlist,
        }
    }
}

pub(crate) struct Toast {
    pub(crate) text: String,
    pub(crate) error: bool,
    pub(crate) created: Instant,
}

pub(crate) enum Modal {
    NewPlaylist {
        name: String,
    },
    SaveQueue {
        name: String,
    },
    RenamePlaylist {
        id: i64,
        name: String,
    },
    Navidrome {
        url: String,
        user: String,
        password: String,
    },
    AddRadio {
        name: String,
        url: String,
    },
}

pub(crate) struct BrowserState {
    pub(crate) input: String,
    pub(crate) source: String,
    pub(crate) section: String,
    pub(crate) groups: Vec<SearchGroup>,
    pub(crate) items: Vec<Track>,
    pub(crate) title: String,
    pub(crate) loading: bool,
    pub(crate) error: String,
    pub(crate) generation: u64,
    pub(crate) browse_generation: u64,
    pub(crate) selected: usize,
    pub(crate) focus_search: bool,
    pub(crate) search_due: Option<Instant>,
    pub(crate) last_query: String,
    pub(crate) history: Vec<(String, String, String)>,
}

impl Default for BrowserState {
    fn default() -> Self {
        Self {
            input: String::new(),
            source: "all".into(),
            section: "all".into(),
            groups: Vec::new(),
            items: Vec::new(),
            title: "Browse".into(),
            loading: false,
            error: String::new(),
            generation: 0,
            browse_generation: 0,
            selected: 0,
            focus_search: false,
            search_due: None,
            last_query: String::new(),
            history: Vec::new(),
        }
    }
}

#[derive(Default)]
pub(crate) struct SettingsState {
    pub(crate) loaded: bool,
    pub(crate) loading: bool,
    pub(crate) error: String,
    pub(crate) values: SettingsResponse,
    pub(crate) pending_toggle: Option<String>,
}

pub(crate) struct VisState {
    pub(crate) requested_start: bool,
    pub(crate) started: bool,
    pub(crate) inflight: bool,
    /// Actual CAVA samples, log-shaped for visibility.
    pub(crate) target: Vec<f32>,
    /// Display easing between samples (fast attack, slower release).
    pub(crate) displayed: Vec<f32>,
    pub(crate) failed: String,
    pub(crate) retry_at: Option<Instant>,
    pub(crate) last_poll: Instant,
    pub(crate) last_step: Instant,
    pub(crate) stop_sent: bool,
}

impl Default for VisState {
    fn default() -> Self {
        Self {
            requested_start: false,
            started: false,
            inflight: false,
            target: Vec::new(),
            displayed: Vec::new(),
            failed: String::new(),
            retry_at: None,
            last_poll: Instant::now() - Duration::from_secs(10),
            last_step: Instant::now(),
            stop_sent: true,
        }
    }
}

#[derive(Default)]
pub(crate) struct ArtworkStore {
    pub(crate) cache: HashMap<String, TextureHandle>,
    pub(crate) order: VecDeque<String>,
    pub(crate) inflight: HashSet<String>,
    pub(crate) missing: HashSet<String>,
    pub(crate) current: Option<String>,
}

pub(crate) enum PendingKind {
    Search { generation: u64, query: String },
    Browse { generation: u64 },
    Playlists,
    PlaylistTracks { id: i64, name: String },
    Settings,
    Artwork { key: String },
    VisStart,
    VisData,
    Play,
    Snapshot,
    Devices,
    PlaylistCreated { tracks: Vec<Track> },
    Expansion { action: ExpandAction, label: String },
    Generic { label: String, quiet: bool },
}

pub(crate) enum ExpandAction {
    Play,
    Queue,
    PlayNext,
    AddToPlaylist(i64, String),
    NewPlaylist,
}

pub(crate) struct Pending {
    pub(crate) kind: PendingKind,
    pub(crate) started: Instant,
}

pub(crate) fn playable_items(items: &[Track]) -> Vec<Track> {
    items.iter().filter(|t| t.is_track()).cloned().collect()
}

pub(crate) struct QueuedPlaylistRequest {
    pub(crate) track: Track,
    pub(crate) playlist_id: i64,
}

pub struct NiruampApp {
    pub(crate) prefs: Preferences,
    pub(crate) prefs_shared: Arc<Mutex<Preferences>>,
    pub(crate) theme: ResolvedTheme,
    pub(crate) theme_watcher: Option<ThemeWatcher>,
    pub(crate) theme_warning: String,
    pub(crate) backend: Option<BackendHandle>,
    pub(crate) backend_error: Option<DependencyError>,
    pub(crate) snapshot: Snapshot,
    pub(crate) playback: Playback,
    pub(crate) sources: Vec<SourceInfo>,
    pub(crate) capabilities: Capabilities,
    pub(crate) revision: u64,
    pub(crate) received_at: Instant,
    pub(crate) pending: HashMap<u64, Pending>,
    pub(crate) tab: Tab,
    pub(crate) playlist_selected: usize,
    pub(crate) queue_selected: usize,
    pub(crate) queue_drag: Option<usize>,
    pub(crate) browser: BrowserState,
    pub(crate) settings: SettingsState,
    pub(crate) vis: VisState,
    pub(crate) artwork: ArtworkStore,
    pub(crate) decode_tx: Sender<(String, String)>,
    pub(crate) decode_rx: Receiver<(String, Result<egui::ColorImage, String>)>,
    pub(crate) toasts: VecDeque<Toast>,
    pub(crate) modal: Option<Modal>,
    pub(crate) playlists: Vec<PlaylistInfo>,
    pub(crate) pending_new_playlist_tracks: Vec<Track>,
    pub(crate) pending_play: Option<(String, String, Instant)>,
    pub(crate) scrub: Option<f64>,
    pub(crate) scrub_sent: Option<Instant>,
    pub(crate) volume_preview: Option<f64>,
    pub(crate) volume_sent: Option<Instant>,
    pub(crate) deck_height: f32,
    pub(crate) instance: Option<InstanceGuard>,
    pub(crate) instance_rx: Receiver<()>,
    pub(crate) activation_requests: usize,
    pub(crate) window_state: WindowState,
    pub(crate) last_geometry_save: Instant,
    pub(crate) last_geometry: Option<(f32, f32, f32, f32)>,
    pub(crate) just_retried: Option<Instant>,
    pub(crate) frame_count: u64,
    pub(crate) screenshot_path: Option<PathBuf>,
    pub(crate) screenshot_at: Option<Instant>,
    pub(crate) reduce_motion: bool,
    pub(crate) keyboard_nav: bool,
    pub(crate) debug_fps: bool,
    pub(crate) fps_start: Instant,
    pub(crate) fps_frames: u64,
    pub(crate) debug_messages: u64,
    pub(crate) ctx: Context,
    pub(crate) fitted_window: bool,
    pub(crate) text_input_active: bool,
    pub(crate) tray: Option<TrayHandle>,
    pub(crate) tray_rx: Receiver<TrayCommand>,
    pub(crate) tray_error: Option<String>,
    pub(crate) hidden_to_tray: bool,
    pub(crate) startup_sound_done: bool,
    pub(crate) immersive: bool,
}

impl NiruampApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        prefs: Preferences,
        initial: Result<BackendHandle, DependencyError>,
        instance: Option<InstanceGuard>,
        window_state: WindowState,
    ) -> Self {
        let ctx = cc.egui_ctx.clone();
        let theme = theme::resolve(&prefs).unwrap_or_else(|e| {
            let mut t = theme::resolve(&Preferences::default()).expect("classic theme");
            t.warnings.push(e);
            t
        });
        theme::install_style(&ctx, &theme, &prefs);
        let prefs_shared = Arc::new(Mutex::new(prefs.clone()));
        let watcher = ThemeWatcher::spawn(prefs_shared.clone(), ctx.clone());
        let (decode_tx, decode_job_rx) = mpsc::channel::<(String, String)>();
        let (decode_done_tx, decode_rx) =
            mpsc::channel::<(String, Result<egui::ColorImage, String>)>();
        std::thread::Builder::new()
            .name("niruamp-artwork-decode".into())
            .spawn(move || {
                while let Ok((key, data)) = decode_job_rx.recv() {
                    let result = decode_artwork(&data);
                    let _ = decode_done_tx.send((key, result));
                }
            })
            .ok();
        let (backend, backend_error) = match initial {
            Ok(b) => (Some(b), None),
            Err(e) => (None, Some(e)),
        };
        let tab = Tab::from_key(&prefs.active_tab);
        let (tray_tx, tray_rx) = mpsc::channel();
        let (tray, tray_error) = if prefs.tray_enabled {
            match TrayHandle::spawn(tray_tx, ctx.clone(), &theme.palette) {
                Ok(tray) => (Some(tray), None),
                Err(e) => {
                    eprintln!("niruamp: system tray unavailable: {e}");
                    (None, Some(e))
                }
            }
        } else {
            (None, None)
        };
        let (instance, instance_rx) = match instance {
            Some(mut guard) => {
                let (tx, rx) = mpsc::channel();
                guard.start_listener(tx, ctx.clone());
                (Some(guard), rx)
            }
            None => {
                let (_tx, rx) = mpsc::channel();
                (None, rx)
            }
        };
        let mut app = Self {
            prefs,
            prefs_shared,
            theme,
            theme_watcher: Some(watcher),
            theme_warning: String::new(),
            backend,
            backend_error,
            snapshot: Snapshot::default(),
            playback: Playback::default(),
            sources: Vec::new(),
            capabilities: Capabilities::default(),
            revision: 0,
            received_at: Instant::now(),
            pending: HashMap::new(),
            tab,
            playlist_selected: 0,
            queue_selected: 0,
            queue_drag: None,
            browser: BrowserState::default(),
            settings: SettingsState::default(),
            vis: VisState::default(),
            artwork: ArtworkStore::default(),
            decode_tx,
            decode_rx,
            toasts: VecDeque::new(),
            modal: None,
            playlists: Vec::new(),
            pending_new_playlist_tracks: Vec::new(),
            pending_play: None,
            scrub: None,
            scrub_sent: None,
            volume_preview: None,
            volume_sent: None,
            deck_height: 250.0,
            instance,
            instance_rx,
            activation_requests: 0,
            window_state,
            last_geometry_save: Instant::now(),
            last_geometry: None,
            just_retried: None,
            frame_count: 0,
            screenshot_path: std::env::var_os("NIRUAMP_SCREENSHOT").map(PathBuf::from),
            screenshot_at: std::env::var("NIRUAMP_SCREENSHOT_DELAY")
                .ok()
                .and_then(|v| v.parse::<f32>().ok())
                .map(|secs| Instant::now() + Duration::from_secs_f32(secs.max(0.5))),
            reduce_motion: false,
            keyboard_nav: false,
            debug_fps: std::env::var_os("NIRUAMP_DEBUG_FPS").is_some(),
            fps_start: Instant::now(),
            fps_frames: 0,
            debug_messages: 0,
            ctx: ctx.clone(),
            fitted_window: false,
            text_input_active: false,
            tray,
            tray_rx,
            tray_error,
            hidden_to_tray: false,
            startup_sound_done: false,
            immersive: false,
        };
        if let Some(b) = &app.backend {
            let c = ctx.clone();
            b.client.set_wake(Arc::new(move || c.request_repaint()));
        }
        app.initialize_from_backend(&ctx);
        app
    }

    fn initialize_from_backend(&mut self, ctx: &Context) {
        self.reduce_motion = theme::reduced_motion(&self.theme, &self.prefs);
        if self.prefs.always_on_top {
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
                egui::WindowLevel::AlwaysOnTop,
            ));
        }
        for w in self.theme.warnings.clone() {
            self.toast(w, true);
        }
        if let Some(b) = self.backend.as_ref() {
            let snap = b.snapshot.clone();
            self.apply_snapshot(snap, ctx);
            self.refresh_tray();
            // Subscriptions make the connection receive ordered state events.
            // The response itself is a fresh snapshot and covers the gap between
            // handshake and subscription.
            self.request("state.subscribe", json!({}), Some(PendingKind::Snapshot));
            self.request("playlists.list", json!({}), Some(PendingKind::Playlists));
        }
        if self.prefs.window_shade {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(
                self.window_state.normal_width.max(420.0),
                self.deck_height,
            )));
        }
    }

    fn apply_snapshot(&mut self, snap: Snapshot, ctx: &Context) {
        self.revision = snap.revision;
        self.playback = snap.playback.clone();
        self.sources = snap.sources.clone();
        self.capabilities = snap.capabilities.clone();
        self.snapshot = snap;
        self.received_at = Instant::now();
        self.pending_play = None;
        self.vis.failed.clear();
        self.vis.retry_at = None;
        self.update_artwork(ctx);
        self.clamp_selection();
    }

    fn clamp_selection(&mut self) {
        if self.playlist_selected >= self.playback.context.len() {
            self.playlist_selected = self.playback.context.len().saturating_sub(1);
        }
        if self.queue_selected >= self.playback.queue.len() {
            self.queue_selected = self.playback.queue.len().saturating_sub(1);
        }
        let len = self.browser_row_count();
        if self.browser.selected >= len {
            self.browser.selected = len.saturating_sub(1);
        }
    }

    pub(crate) fn palette(&self) -> Palette {
        self.theme.palette
    }

    pub(crate) fn s(&self, value: f32) -> f32 {
        value * self.prefs.scale
    }

    pub(crate) fn position_now(&self) -> f64 {
        if let Some(v) = self.scrub {
            return v.max(0.0);
        }
        let mut pos = self.playback.position;
        if self.playback.status == "playing" {
            pos += self.received_at.elapsed().as_secs_f64();
        }
        let dur = self.playback.duration;
        if dur > 0.0 {
            pos = pos.min(dur);
        }
        pos.max(0.0)
    }

    pub(crate) fn volume_now(&self) -> f64 {
        self.volume_preview.unwrap_or(self.playback.volume)
    }

    pub(crate) fn current_track(&self) -> Option<&Track> {
        self.playback.current.as_ref()
    }

    pub(crate) fn toast(&mut self, text: impl Into<String>, error: bool) {
        let text = text.into();
        if text.is_empty() {
            return;
        }
        if let Some(last) = self.toasts.back() {
            if last.text == text
                && last.error == error
                && last.created.elapsed() < Duration::from_secs(2)
            {
                return;
            }
        }
        self.toasts.push_back(Toast {
            text,
            error,
            created: Instant::now(),
        });
        while self.toasts.len() > 5 {
            self.toasts.pop_front();
        }
    }

    pub(crate) fn request(&mut self, method: &str, params: Value, kind: Option<PendingKind>) {
        let is_vis_data = matches!(kind, Some(PendingKind::VisData));
        let sent = match self.backend.as_ref() {
            Some(b) => b.client.send(method, params),
            None => return,
        };
        match sent {
            Ok(id) => {
                if let Some(kind) = kind {
                    self.pending.insert(
                        id,
                        Pending {
                            kind,
                            started: Instant::now(),
                        },
                    );
                }
                // Responses do not drive their own repaint storm. One bounded
                // poll picks them up; visualization frames use the capped
                // animation clock instead.
                if !is_vis_data {
                    self.ctx.request_repaint_after(Duration::from_millis(25));
                }
            }
            Err(e) => {
                self.connection_lost(e.to_string());
            }
        }
    }

    fn connection_lost(&mut self, message: String) {
        if self.backend_error.is_some() {
            return;
        }
        self.backend = None;
        self.pending.clear();
        self.pending_play = None;
        self.vis.started = false;
        self.vis.inflight = false;
        self.backend_error = Some(DependencyError {
            title: "NPLAY connection lost".into(),
            message,
            remedy: "NIRUAMP will not start a second playback engine. Reconnect to the running NPLAY service, or restart NPLAY.".into(),
            log_tail: String::new(),
        });
    }

    fn reconnect(&mut self, ctx: &Context) {
        self.just_retried = Some(Instant::now());
        match platform::ensure_backend(&self.prefs, true) {
            Ok(b) => {
                let c = ctx.clone();
                b.client.set_wake(Arc::new(move || c.request_repaint()));
                self.snapshot = b.snapshot.clone();
                self.backend = Some(b);
                self.backend_error = None;
                self.pending.clear();
                self.initialize_from_backend(ctx);
                self.toast("NPLAY connected", false);
            }
            Err(e) => {
                self.backend_error = Some(e);
            }
        }
    }

    fn drain_backend(&mut self, ctx: &Context) {
        let mut messages = Vec::new();
        if let Some(b) = self.backend.as_ref() {
            while let Some(msg) = b.client.try_recv() {
                messages.push(msg);
                if messages.len() >= 128 {
                    break;
                }
            }
        }
        if self.debug_fps {
            self.debug_messages += messages.len() as u64;
        }
        for msg in messages {
            match msg {
                Incoming::Response { id, result } => self.handle_response(id, result, ctx),
                Incoming::Notification { method, params } => {
                    self.handle_notification(&method, params, ctx)
                }
                Incoming::Disconnected(message) => self.connection_lost(message),
            }
        }
        let now = Instant::now();
        let expired: Vec<u64> = self
            .pending
            .iter()
            .filter(|(_, p)| now.duration_since(p.started) > Duration::from_secs(35))
            .map(|(id, _)| *id)
            .collect();
        for id in expired {
            if let Some(p) = self.pending.remove(&id) {
                if !matches!(p.kind, PendingKind::Artwork { .. } | PendingKind::VisData) {
                    self.toast("NPLAY request timed out", true);
                }
            }
        }
        // Decoded artwork never touches the UI thread while decoding.
        while let Ok((key, result)) = self.decode_rx.try_recv() {
            match result {
                Ok(image) => {
                    let texture =
                        ctx.load_texture(key.clone(), image, egui::TextureOptions::LINEAR);
                    self.artwork.cache.insert(key.clone(), texture);
                    self.artwork.order.push_back(key);
                    while self.artwork.order.len() > 8 {
                        if let Some(old) = self.artwork.order.pop_front() {
                            self.artwork.cache.remove(&old);
                        }
                    }
                }
                Err(_) => {
                    self.artwork.missing.insert(key);
                }
            }
        }
        while let Ok(()) = self.instance_rx.try_recv() {
            self.activation_requests += 1;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            self.toast("NIRUAMP is already running", false);
        }
        let mut theme_events = Vec::new();
        if let Some(w) = self.theme_watcher.as_ref() {
            while let Ok(event) = w.rx.try_recv() {
                theme_events.push(event);
            }
        }
        for event in theme_events {
            match event {
                ThemeEvent::Updated(theme) => {
                    self.theme = *theme;
                    theme::install_style(ctx, &self.theme, &self.prefs);
                    self.reduce_motion = theme::reduced_motion(&self.theme, &self.prefs);
                    self.refresh_tray_icon();
                    self.theme_warning.clear();
                }
                ThemeEvent::Warning(msg) => {
                    if msg != self.theme_warning {
                        self.theme_warning = msg.clone();
                        self.toast(format!("Theme not applied: {msg}"), true);
                    }
                }
            }
        }
        if let Some(due) = self.browser.search_due {
            if Instant::now() >= due {
                self.browser.search_due = None;
                let query = self.browser.input.trim().to_string();
                if query.len() >= 2 && query != self.browser.last_query {
                    self.start_search();
                }
            }
        }
        self.maybe_visualization(ctx);
        self.tick_geometry(ctx);
        self.frame_count += 1;
    }

    fn handle_notification(&mut self, method: &str, params: Value, ctx: &Context) {
        match method {
            "state.changed" => {
                match serde_json::from_value::<StateChanged>(params) {
                    Ok(change) => {
                        if change.revision < self.revision {
                            return;
                        }
                        let previous_key = self.playback.current_key().map(str::to_string);
                        self.revision = change.revision;
                        self.playback = change.playback;
                        if previous_key != self.playback.current_key().map(str::to_string) {
                            self.vis.failed.clear();
                            self.vis.retry_at = None;
                        }
                        if !change.sources.is_empty() {
                            self.sources = change.sources;
                        }
                        self.capabilities = change.capabilities;
                        self.received_at = Instant::now();
                        self.pending_play = None;
                        self.scrub = None;
                        self.volume_preview = None;
                        self.update_artwork(ctx);
                        self.clamp_selection();
                        self.refresh_tray();
                        if self.settings.loaded {
                            // Source enablement and playback modes can change from
                            // the terminal; refresh without stealing focus.
                            self.settings.loading = false;
                        }
                    }
                    Err(e) => self.toast(format!("Invalid state event: {e}"), true),
                }
            }
            "library.scanned" => {
                let text = if let Some(err) = params.get("error").and_then(Value::as_str) {
                    format!("Library scan failed: {err}")
                } else if let Some(result) = params.get("result") {
                    format!(
                        "Library scan · {} checked · {} new · {} updated · {} removed",
                        result.get("checked").and_then(Value::as_u64).unwrap_or(0),
                        result.get("new").and_then(Value::as_u64).unwrap_or(0),
                        result.get("updated").and_then(Value::as_u64).unwrap_or(0),
                        result.get("removed").and_then(Value::as_u64).unwrap_or(0),
                    )
                } else {
                    "Library scan finished".to_string()
                };
                self.toast(text, params.get("error").is_some());
                self.load_settings();
            }
            "spotify.auth" => {
                if params.get("connected").and_then(Value::as_bool) == Some(true) {
                    self.toast("Spotify connected", false);
                } else {
                    self.toast(
                        format!(
                            "Spotify authorization failed: {}",
                            params
                                .get("error")
                                .and_then(Value::as_str)
                                .unwrap_or("unknown error")
                        ),
                        true,
                    );
                }
                self.load_settings();
            }
            _ => {}
        }
    }

    fn handle_response(
        &mut self,
        id: u64,
        result: Result<Value, crate::protocol::RpcError>,
        ctx: &Context,
    ) {
        let Some(pending) = self.pending.remove(&id) else {
            return;
        };
        match (pending.kind, result) {
            (PendingKind::Search { generation, query }, Ok(value)) => {
                if generation != self.browser.generation || query != self.browser.input.trim() {
                    return;
                }
                match serde_json::from_value::<SearchResponse>(value) {
                    Ok(mut response) => {
                        for group in &mut response.groups {
                            if !group.error.is_empty() {
                                self.toast(
                                    format!("{} search failed: {}", group.label, group.error),
                                    true,
                                );
                                group.error.clear();
                            }
                        }
                        self.browser.groups = response.groups;
                        self.browser.items.clear();
                        self.browser.title = format!("Search · {}", response.query);
                        self.browser.loading = false;
                        self.browser.error.clear();
                        self.browser.selected = 0;
                        self.browser.last_query = response.query;
                    }
                    Err(e) => {
                        self.browser.loading = false;
                        self.browser.error = e.to_string();
                    }
                }
            }
            (PendingKind::Browse { generation }, Ok(value)) => {
                if generation != self.browser.browse_generation {
                    return;
                }
                match serde_json::from_value::<BrowseResponse>(value) {
                    Ok(response) => {
                        self.browser.items = response.items;
                        self.browser.groups.clear();
                        self.browser.title = response.title;
                        self.browser.loading = false;
                        self.browser.error.clear();
                        self.browser.selected = 0;
                        self.browser.source = response.source;
                        self.browser.section = response.section;
                    }
                    Err(e) => {
                        self.browser.loading = false;
                        self.browser.error = e.to_string();
                    }
                }
            }
            (PendingKind::Playlists, Ok(value)) => {
                if let Ok(r) = serde_json::from_value::<PlaylistsResponse>(value) {
                    self.playlists = r.items;
                }
            }
            (PendingKind::PlaylistTracks { id, name }, Ok(value)) => {
                if let Ok(r) = serde_json::from_value::<PlaylistTracks>(value) {
                    self.browser.items = r.items;
                    self.browser.groups.clear();
                    self.browser.title = if r.name.is_empty() { name } else { r.name };
                    self.browser.source = "library".into();
                    self.browser.section = "playlist".into();
                    self.browser.loading = false;
                    self.browser.selected = 0;
                    self.tab = Tab::Browse;
                    let _ = id;
                }
            }
            (PendingKind::Settings, Ok(value)) => {
                match serde_json::from_value::<SettingsResponse>(value) {
                    Ok(r) => {
                        self.settings.values = r;
                        self.settings.loaded = true;
                        self.settings.loading = false;
                        self.settings.error.clear();
                    }
                    Err(e) => {
                        self.settings.loading = false;
                        self.settings.error = e.to_string();
                    }
                }
            }
            (PendingKind::Artwork { key }, Ok(value)) => {
                self.artwork.inflight.remove(&key);
                if let Ok(r) = serde_json::from_value::<ArtworkResponse>(value) {
                    if r.empty || r.data.is_empty() {
                        self.artwork.missing.insert(key);
                    } else {
                        let _ = self.decode_tx.send((key, r.data));
                    }
                }
            }
            (PendingKind::VisStart, Ok(_)) => {
                self.vis.started = true;
                self.vis.requested_start = false;
                self.vis.inflight = false;
                self.vis.failed.clear();
                self.vis.retry_at = None;
                self.vis.stop_sent = false;
            }
            (PendingKind::VisData, Ok(value)) => {
                self.vis.inflight = false;
                if let Ok(r) = serde_json::from_value::<VisualizationData>(value) {
                    let shaped: Vec<f32> = r
                        .bars
                        .into_iter()
                        .map(|b| ((b as f32 / 255.0).clamp(0.0, 1.0)).powf(0.62))
                        .collect();
                    if shaped.len() != self.vis.displayed.len() {
                        self.vis.displayed = shaped.clone();
                    }
                    self.vis.target = shaped;
                }
            }
            (PendingKind::Play, Ok(_)) => {}
            (PendingKind::Snapshot, Ok(value)) => {
                if let Ok(snap) = serde_json::from_value::<Snapshot>(value) {
                    self.apply_snapshot(snap, ctx);
                }
            }
            (PendingKind::Devices, Ok(value)) => {
                if let Ok(response) = serde_json::from_value::<BrowseResponse>(value) {
                    self.browser.items = response.items;
                    self.browser.groups.clear();
                    self.browser.title = "Spotify · Devices".into();
                    self.browser.source = "spotify".into();
                    self.browser.section = "devices".into();
                    self.browser.loading = false;
                    self.tab = Tab::Browse;
                }
            }
            (PendingKind::PlaylistCreated { tracks }, Ok(value)) => {
                if let Some(id) = value.get("id").and_then(Value::as_i64) {
                    if !tracks.is_empty() {
                        let count = tracks.len();
                        self.request(
                            "playlists.add_many",
                            json!({"id": id, "tracks": tracks}),
                            Some(PendingKind::Generic {
                                label: format!("Added {count} tracks to new playlist"),
                                quiet: false,
                            }),
                        );
                    }
                    self.request("playlists.list", json!({}), Some(PendingKind::Playlists));
                }
            }
            (PendingKind::Expansion { action, label }, Ok(value)) => {
                match serde_json::from_value::<BrowseResponse>(value) {
                    Ok(response) => self.apply_expansion(action, &response.items, &label),
                    Err(e) => self.toast(format!("Cannot read {label}: {e}"), true),
                }
            }
            (PendingKind::Generic { label, quiet }, Ok(_)) => {
                if !quiet {
                    self.toast(label, false);
                }
            }
            (kind, Err(err)) => {
                let label = match &kind {
                    PendingKind::Search { .. } => {
                        self.browser.loading = false;
                        self.browser.error = err.to_string();
                        "Search failed".to_string()
                    }
                    PendingKind::Browse { .. } => {
                        self.browser.loading = false;
                        self.browser.error = err.to_string();
                        "Browse failed".to_string()
                    }
                    PendingKind::Settings => {
                        self.settings.loading = false;
                        self.settings.error = err.to_string();
                        "Settings unavailable".to_string()
                    }
                    PendingKind::Play => {
                        self.pending_play = None;
                        self.request("state.snapshot", json!({}), Some(PendingKind::Snapshot));
                        "Playback failed".to_string()
                    }
                    PendingKind::VisStart | PendingKind::VisData => {
                        self.vis.started = false;
                        self.vis.inflight = false;
                        self.vis.requested_start = false;
                        self.vis.failed = err.to_string();
                        self.vis.retry_at = Some(Instant::now() + Duration::from_secs(4));
                        ctx.request_repaint_after(Duration::from_secs(4));
                        String::new()
                    }
                    PendingKind::Artwork { key } => {
                        self.artwork.inflight.remove(key);
                        String::new()
                    }
                    PendingKind::Playlists => "Playlists unavailable".to_string(),
                    PendingKind::PlaylistTracks { .. } => {
                        self.browser.loading = false;
                        self.browser.error = err.to_string();
                        "Playlist unavailable".to_string()
                    }
                    PendingKind::Snapshot => "State refresh failed".to_string(),
                    PendingKind::Devices => "Spotify devices unavailable".to_string(),
                    PendingKind::PlaylistCreated { .. } => "Playlist creation failed".to_string(),
                    PendingKind::Expansion { label, .. } => format!("{label} unavailable"),
                    PendingKind::Generic { label, .. } => label.clone(),
                };
                if !label.is_empty() {
                    self.toast(format!("{label}: {err}"), true);
                }
            }
        }
    }

    pub(crate) fn load_settings(&mut self) {
        if self.settings.loading {
            return;
        }
        self.settings.loading = true;
        self.request("settings.get", json!({}), Some(PendingKind::Settings));
    }

    fn refresh_tray(&mut self) {
        let Some(tray) = self.tray.as_mut() else {
            return;
        };
        let title = self
            .playback
            .current
            .as_ref()
            .map(|t| t.title.clone())
            .unwrap_or_default();
        let detail = self
            .playback
            .current
            .as_ref()
            .map(|t| {
                let status = self.playback.status.to_uppercase();
                let mut parts = vec![status];
                if !t.artist.is_empty() {
                    parts.push(t.artist.clone());
                }
                if !t.album.is_empty() && t.album != t.title {
                    parts.push(t.album.clone());
                }
                parts.join(" · ")
            })
            .unwrap_or_else(|| "NPLAY frontend · stopped".into());
        tray.update(&title, &detail, self.playback.is_playing());
    }

    fn refresh_tray_icon(&mut self) {
        let palette = self.theme.palette;
        if let Some(tray) = self.tray.as_mut() {
            tray.set_palette(&palette);
        }
    }

    fn drain_tray(&mut self, ctx: &Context) {
        let mut commands = Vec::new();
        while let Ok(command) = self.tray_rx.try_recv() {
            commands.push(command);
        }
        for command in commands {
            match command {
                TrayCommand::Show => self.show_from_tray(ctx),
                TrayCommand::Toggle => self.toggle(),
                TrayCommand::Next => self.next(),
                TrayCommand::Previous => self.previous(),
                TrayCommand::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                TrayCommand::Volume(delta) => self.volume_delta(delta as f64),
            }
        }
    }

    pub(crate) fn hide_to_tray(&mut self, ctx: &Context) {
        if self.tray.is_some() && self.prefs.tray_enabled {
            self.hidden_to_tray = true;
            self.vis.target.clear();
            self.vis.displayed.clear();
            self.vis.started = false;
            self.request("visualization.stop", json!({}), None);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
    }

    pub(crate) fn show_from_tray(&mut self, ctx: &Context) {
        self.hidden_to_tray = false;
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        ctx.request_repaint();
    }

    pub(crate) fn set_tray_enabled(&mut self, ctx: &Context, enabled: bool) {
        self.prefs.tray_enabled = enabled;
        if enabled && self.tray.is_none() {
            let (tx, rx) = mpsc::channel();
            match TrayHandle::spawn(tx, ctx.clone(), &self.theme.palette) {
                Ok(tray) => {
                    self.tray = Some(tray);
                    self.tray_error = None;
                    self.refresh_tray_icon();
                    self.refresh_tray();
                }
                Err(e) => {
                    self.tray_error = Some(e.clone());
                    self.toast(format!("System tray unavailable: {e}"), true);
                }
            }
            self.tray_rx = rx;
        }
        if !enabled {
            if let Some(tray) = self.tray.take() {
                tray.shutdown();
            }
        }
        self.update_prefs(ctx);
    }

    fn update_artwork(&mut self, ctx: &Context) {
        let key = self.playback.current.as_ref().map(|t| t.key.clone());
        if key == self.artwork.current {
            return;
        }
        self.artwork.current = key.clone();
        let Some(track) = self.playback.current.as_ref() else {
            return;
        };
        if !track.has_art || !self.capabilities.artwork {
            return;
        }
        let key = track.key.clone();
        if self.artwork.cache.contains_key(&key)
            || self.artwork.missing.contains(&key)
            || self.artwork.inflight.contains(&key)
        {
            return;
        }
        self.artwork.inflight.insert(key.clone());
        self.request(
            "artwork.get",
            json!({"track": {"key": track.key}}),
            Some(PendingKind::Artwork { key }),
        );
        ctx.request_repaint();
    }

    fn maybe_visualization(&mut self, ctx: &Context) {
        let visible = self.immersive || self.tab != Tab::Settings || self.prefs.show_visualization;
        let should = (self.prefs.show_visualization || self.immersive)
            && self.capabilities.visualization
            && visible
            && matches!(self.playback.status.as_str(), "playing" | "paused")
            && !self.hidden_to_tray
            && !ctx.input(|i| i.viewport().minimized.unwrap_or(false));
        if !should {
            self.vis.retry_at = None;
        } else if self.vis.retry_at.is_some_and(|at| Instant::now() >= at) {
            self.vis.failed.clear();
            self.vis.retry_at = None;
        }
        if should && !self.vis.started && !self.vis.requested_start && self.vis.failed.is_empty() {
            self.vis.requested_start = true;
            self.vis.stop_sent = false;
            self.request(
                "visualization.start",
                json!({}),
                Some(PendingKind::VisStart),
            );
        }
        if should && self.vis.started && !self.vis.inflight {
            let interval = if self.reduce_motion {
                Duration::from_millis(120)
            } else {
                Duration::from_millis(33)
            };
            if self.vis.last_poll.elapsed() >= interval {
                self.vis.last_poll = Instant::now();
                self.vis.inflight = true;
                self.request(
                    "visualization.data",
                    json!({"bars": 32}),
                    Some(PendingKind::VisData),
                );
            }
        }
        if !should && self.vis.started && !self.vis.stop_sent {
            self.vis.stop_sent = true;
            self.vis.started = false;
            self.vis.target.clear();
            self.vis.displayed.clear();
            self.request("visualization.stop", json!({}), None);
        }
        if !self.capabilities.visualization {
            self.vis.started = false;
        }
    }

    fn advance_visualization(&mut self) {
        if self.vis.displayed.is_empty() || self.vis.target.len() != self.vis.displayed.len() {
            return;
        }
        let now = Instant::now();
        let dt = (now - self.vis.last_step).as_secs_f32().clamp(0.001, 0.2);
        self.vis.last_step = now;
        let attack = 1.0 - (-dt / 0.045).exp();
        let release = 1.0 - (-dt / 0.14).exp();
        for (shown, target) in self.vis.displayed.iter_mut().zip(self.vis.target.iter()) {
            let delta = *target - *shown;
            let factor = if delta >= 0.0 { attack } else { release };
            *shown += delta * factor;
        }
    }

    fn tick_geometry(&mut self, ctx: &Context) {
        if !self.fitted_window {
            if let Some(monitor) = ctx.input(|i| i.viewport().monitor_size) {
                if let Some(inner) = ctx.input(|i| i.viewport().inner_rect) {
                    let max_w = (monitor.x * 0.96).max(400.0);
                    let max_h = (monitor.y * 0.92).max(120.0);
                    if inner.width() > max_w || inner.height() > max_h {
                        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(
                            inner.width().min(max_w),
                            if self.prefs.window_shade {
                                inner.height()
                            } else {
                                inner.height().min(max_h)
                            },
                        )));
                    }
                }
                self.fitted_window = true;
            }
        }
        let now = Instant::now();
        if now.duration_since(self.last_geometry_save) < Duration::from_secs(2) {
            return;
        }
        self.last_geometry_save = now;
        let rect = ctx.input(|i| i.viewport().inner_rect);
        let outer = ctx.input(|i| i.viewport().outer_rect);
        if let Some(rect) = rect {
            let x = outer.map(|r| r.min.x).or(self.window_state.x);
            let y = outer.map(|r| r.min.y).or(self.window_state.y);
            let current = (
                rect.width(),
                rect.height(),
                x.unwrap_or(0.0),
                y.unwrap_or(0.0),
            );
            if self
                .last_geometry
                .map(|g| {
                    (g.0 - current.0).abs() > 1.0
                        || (g.1 - current.1).abs() > 1.0
                        || (g.2 - current.2).abs() > 1.0
                        || (g.3 - current.3).abs() > 1.0
                })
                .unwrap_or(true)
            {
                self.last_geometry = Some(current);
                if !self.prefs.window_shade {
                    self.window_state.width = rect.width();
                    self.window_state.height = rect.height();
                    self.window_state.normal_width = rect.width();
                    self.window_state.normal_height = rect.height();
                }
                self.window_state.x = x;
                self.window_state.y = y;
                let _ = self.window_state.save();
            }
        }
    }

    pub(crate) fn update_prefs(&mut self, ctx: &Context) {
        {
            if let Ok(mut p) = self.prefs_shared.lock() {
                *p = self.prefs.clone();
            }
        }
        if let Err(e) = self.prefs.save() {
            self.toast(format!("Cannot save NIRUAMP preferences: {e}"), true);
        }
        let _ = ctx;
    }

    pub(crate) fn reload_theme(&mut self, ctx: &Context) {
        match theme::resolve(&self.prefs) {
            Ok(theme) => {
                self.theme = theme;
                theme::install_style(ctx, &self.theme, &self.prefs);
                self.reduce_motion = theme::reduced_motion(&self.theme, &self.prefs);
                self.refresh_tray_icon();
                self.theme_warning.clear();
            }
            Err(e) => self.toast(format!("Theme not applied: {e}"), true),
        }
        self.update_prefs(ctx);
    }

    // -- actions ---------------------------------------------------------
    pub(crate) fn toggle(&mut self) {
        self.request(
            "playback.toggle",
            json!({}),
            Some(PendingKind::Generic {
                label: String::new(),
                quiet: true,
            }),
        );
    }

    pub(crate) fn stop(&mut self) {
        self.request(
            "playback.stop",
            json!({}),
            Some(PendingKind::Generic {
                label: String::new(),
                quiet: true,
            }),
        );
        self.scrub = None;
    }

    pub(crate) fn next(&mut self) {
        self.request(
            "playback.next",
            json!({}),
            Some(PendingKind::Generic {
                label: String::new(),
                quiet: true,
            }),
        );
    }

    pub(crate) fn previous(&mut self) {
        self.request(
            "playback.previous",
            json!({}),
            Some(PendingKind::Generic {
                label: String::new(),
                quiet: true,
            }),
        );
    }

    pub(crate) fn seek_relative(&mut self, delta: f64) {
        self.request(
            "playback.seek",
            json!({"delta": delta}),
            Some(PendingKind::Generic {
                label: String::new(),
                quiet: true,
            }),
        );
    }

    pub(crate) fn seek_to(&mut self, position: f64) {
        self.scrub = Some(position.max(0.0));
        let now = Instant::now();
        let send = self
            .scrub_sent
            .map(|t| now.duration_since(t) > Duration::from_millis(120))
            .unwrap_or(true);
        if send {
            self.scrub_sent = Some(now);
            self.request(
                "playback.seek",
                json!({"position": position.max(0.0)}),
                Some(PendingKind::Generic {
                    label: String::new(),
                    quiet: true,
                }),
            );
        }
    }

    pub(crate) fn commit_seek(&mut self) {
        if let Some(pos) = self.scrub.take() {
            self.request(
                "playback.seek",
                json!({"position": pos}),
                Some(PendingKind::Generic {
                    label: String::new(),
                    quiet: true,
                }),
            );
        }
    }

    pub(crate) fn set_volume(&mut self, volume: f64) {
        let volume = volume.clamp(0.0, 150.0);
        self.volume_preview = Some(volume);
        let now = Instant::now();
        if self
            .volume_sent
            .map(|t| now.duration_since(t) > Duration::from_millis(110))
            .unwrap_or(true)
        {
            self.volume_sent = Some(now);
            self.request(
                "playback.volume",
                json!({"value": volume}),
                Some(PendingKind::Generic {
                    label: String::new(),
                    quiet: true,
                }),
            );
        }
    }

    pub(crate) fn commit_volume(&mut self) {
        if let Some(v) = self.volume_preview.take() {
            self.request(
                "playback.volume",
                json!({"value": v}),
                Some(PendingKind::Generic {
                    label: String::new(),
                    quiet: true,
                }),
            );
        }
    }

    pub(crate) fn volume_delta(&mut self, delta: f64) {
        let v = (self.volume_now() + delta).clamp(0.0, 150.0);
        self.set_volume(v);
        self.commit_volume();
    }

    pub(crate) fn mute(&mut self) {
        self.request(
            "playback.mute",
            json!({}),
            Some(PendingKind::Generic {
                label: String::new(),
                quiet: true,
            }),
        );
    }

    pub(crate) fn repeat(&mut self) {
        self.request(
            "playback.repeat",
            json!({}),
            Some(PendingKind::Generic {
                label: String::new(),
                quiet: true,
            }),
        );
    }

    pub(crate) fn shuffle(&mut self) {
        self.request(
            "playback.shuffle",
            json!({}),
            Some(PendingKind::Generic {
                label: String::new(),
                quiet: true,
            }),
        );
    }

    pub(crate) fn play_group(&mut self, tracks: &[Track], index: usize) {
        let Some(selected) = tracks.get(index).cloned() else {
            return;
        };
        if !selected.is_track() {
            self.open_item(&selected);
            return;
        }
        let playable = playable_items(tracks);
        let position = playable
            .iter()
            .position(|t| t.key == selected.key)
            .unwrap_or(0);
        let Some(track) = playable.get(position).cloned() else {
            return;
        };
        self.playback.status = "loading".into();
        self.playback.current = Some(track.clone());
        self.playback.position = 0.0;
        self.playback.duration = track.duration;
        self.playback.context = playable.clone();
        self.playback.context_index = position as i32;
        self.received_at = Instant::now();
        self.pending_play = Some((track.key.clone(), track.title.clone(), Instant::now()));
        self.request(
            "playback.play",
            json!({"track": track, "context": playable, "index": position}),
            Some(PendingKind::Play),
        );
    }

    pub(crate) fn play_now(&mut self, track: &Track) {
        let (context, index) = self.context_for(track);
        self.play_group(&context, index);
    }

    fn context_for(&self, track: &Track) -> (Vec<Track>, usize) {
        if let Some(group) = self
            .browser
            .groups
            .iter()
            .find(|g| g.items.iter().any(|t| t.key == track.key))
        {
            let idx = group
                .items
                .iter()
                .position(|t| t.key == track.key)
                .unwrap_or(0);
            return (group.items.clone(), idx);
        }
        if let Some(idx) = self.browser.items.iter().position(|t| t.key == track.key) {
            return (self.browser.items.clone(), idx);
        }
        if let Some(idx) = self
            .playback
            .context
            .iter()
            .position(|t| t.key == track.key)
        {
            return (self.playback.context.clone(), idx);
        }
        (vec![track.clone()], 0)
    }

    pub(crate) fn queue_add(&mut self, track: &Track, next: bool) {
        if !track.is_track() {
            let action = if next {
                ExpandAction::PlayNext
            } else {
                ExpandAction::Queue
            };
            self.expand_container(track, action);
            return;
        }
        self.request(
            "queue.add",
            json!({"track": track, "next": next}),
            Some(PendingKind::Generic {
                label: if next {
                    "Queued next"
                } else {
                    "Added to queue"
                }
                .into(),
                quiet: false,
            }),
        );
    }

    pub(crate) fn queue_add_many(&mut self, tracks: &[Track], next: bool) {
        let tracks = playable_items(tracks);
        if tracks.is_empty() {
            return;
        }
        self.request(
            "queue.add_many",
            json!({"tracks": tracks, "next": next}),
            Some(PendingKind::Generic {
                label: format!("Queued {} tracks", tracks.len()),
                quiet: false,
            }),
        );
    }

    pub(crate) fn play_queue_index(&mut self, index: usize) {
        self.request(
            "queue.play",
            json!({"index": index}),
            Some(PendingKind::Generic {
                label: String::new(),
                quiet: true,
            }),
        );
    }

    pub(crate) fn remove_queue(&mut self, index: usize) {
        self.request(
            "queue.remove",
            json!({"index": index}),
            Some(PendingKind::Generic {
                label: "Removed from queue".into(),
                quiet: false,
            }),
        );
    }

    pub(crate) fn move_queue(&mut self, index: usize, delta: i64) {
        self.request(
            "queue.move",
            json!({"index": index, "delta": delta}),
            Some(PendingKind::Generic {
                label: String::new(),
                quiet: true,
            }),
        );
    }

    pub(crate) fn reorder_queue(&mut self, from: usize, to: usize) {
        self.queue_selected = to;
        self.request(
            "queue.reorder",
            json!({"from": from, "to": to}),
            Some(PendingKind::Generic {
                label: String::new(),
                quiet: true,
            }),
        );
    }

    pub(crate) fn clear_queue(&mut self) {
        self.request(
            "queue.clear",
            json!({}),
            Some(PendingKind::Generic {
                label: "Queue cleared".into(),
                quiet: false,
            }),
        );
    }

    pub(crate) fn save_queue(&mut self) {
        self.modal = Some(Modal::SaveQueue {
            name: "Queue".into(),
        });
    }

    pub(crate) fn open_item(&mut self, track: &Track) {
        if track.is_track() {
            self.play_now(track);
            return;
        }
        let (source, section, args) = match (track.source.as_str(), track.kind.as_str()) {
            ("local", "artist") => (
                "local",
                "artist",
                json!({"name": track.meta.get("local_artist").and_then(Value::as_str).unwrap_or(&track.title)}),
            ),
            ("local", "album") => (
                "local",
                "album",
                json!({"artist": track.meta.get("local_artist").and_then(Value::as_str).unwrap_or(""), "album": track.album}),
            ),
            ("local", "folder") => ("local", "folder", json!({"path": track.id})),
            ("navidrome", "album") => (
                "navidrome",
                "album",
                json!({"id": track.meta.get("album_id").and_then(Value::as_str).unwrap_or(&track.id)}),
            ),
            ("navidrome", "artist") => (
                "navidrome",
                "artist",
                json!({"id": track.meta.get("artist_id").and_then(Value::as_str).unwrap_or(&track.id)}),
            ),
            ("navidrome", "playlist") => (
                "navidrome",
                "playlist",
                json!({"id": track.meta.get("playlist_id").and_then(Value::as_str).unwrap_or(&track.id)}),
            ),
            ("spotify", "album") => ("spotify", "album", json!({"id": track.id})),
            ("spotify", "playlist") => ("spotify", "playlist", json!({"id": track.id})),
            ("spotify", "artist") => {
                self.browser.input = track.title.clone();
                self.start_search_source(vec!["spotify".to_string()]);
                return;
            }
            ("spotify", "device") => {
                self.request(
                    "spotify.transfer",
                    json!({"device_id": track.id, "play": false}),
                    Some(PendingKind::Generic {
                        label: format!("Spotify device · {}", track.title),
                        quiet: false,
                    }),
                );
                return;
            }
            ("sr", "program") => (
                "sr",
                "program",
                json!({"id": track.meta.get("program_id").and_then(Value::as_str).unwrap_or(&track.id)}),
            ),
            ("library", "playlist") => {
                let id = track.id.parse::<i64>().unwrap_or(0);
                self.request(
                    "playlists.tracks",
                    json!({"id": id}),
                    Some(PendingKind::PlaylistTracks {
                        id,
                        name: track.title.clone(),
                    }),
                );
                return;
            }
            _ => {
                self.toast(
                    format!("{} items are not directly playable", track.kind),
                    true,
                );
                return;
            }
        };
        self.browse(source, section, "", args);
    }

    pub(crate) fn start_search(&mut self) {
        self.start_search_source(Vec::new());
    }

    pub(crate) fn start_search_source(&mut self, sources: Vec<String>) {
        let query = self.browser.input.trim().to_string();
        if query.is_empty() {
            return;
        }
        self.browser.generation += 1;
        self.browser.loading = true;
        self.browser.error.clear();
        self.browser.history.push((
            self.browser.source.clone(),
            self.browser.section.clone(),
            self.browser.title.clone(),
        ));
        let mut params = json!({"query": query, "limit": 30});
        if !sources.is_empty() {
            if let Value::Object(ref mut map) = params {
                map.insert("sources".into(), json!(sources));
            }
        }
        self.request(
            "search",
            params,
            Some(PendingKind::Search {
                generation: self.browser.generation,
                query,
            }),
        );
        self.tab = Tab::Browse;
    }

    pub(crate) fn browse(&mut self, source: &str, section: &str, query: &str, args: Value) {
        self.browser.browse_generation += 1;
        self.browser.source = source.into();
        self.browser.section = section.into();
        self.browser.loading = true;
        self.browser.error.clear();
        self.browser.groups.clear();
        self.browser
            .history
            .push((source.into(), section.into(), self.browser.title.clone()));
        self.tab = Tab::Browse;
        let mut params = json!({"source": source, "section": section, "limit": 1000, "args": args});
        if !query.is_empty() {
            if let Value::Object(ref mut map) = params {
                map.insert("query".into(), json!(query));
            }
        }
        self.request(
            "browse",
            params,
            Some(PendingKind::Browse {
                generation: self.browser.browse_generation,
            }),
        );
    }

    pub(crate) fn default_browse(&mut self) {
        let (source, section) = match self.browser.source.as_str() {
            "local" => ("local", "all"),
            "navidrome" => ("navidrome", "newest"),
            "spotify" => ("spotify", "playlists"),
            "sr" => ("sr", "live"),
            "youtube" => ("youtube", "search"),
            "library" => ("library", "playlists"),
            "radio" => ("radio", "all"),
            _ => ("local", "all"),
        };
        self.browse(source, section, "", json!({}));
    }

    pub(crate) fn open_playlists(&mut self) {
        self.browse("library", "playlists", "", json!({}));
    }

    pub(crate) fn container_request(
        &self,
        track: &Track,
    ) -> Option<(String, String, Value, String)> {
        let meta = &track.meta;
        let value = |key: &str| meta.get(key).and_then(Value::as_str).unwrap_or("");
        let label = |noun: &str, title: &str| format!("{noun} · {title}");
        match (track.source.as_str(), track.kind.as_str()) {
            ("local", "album") => {
                let artist = if !value("local_artist").is_empty() {
                    value("local_artist").to_string()
                } else {
                    track.artist.clone()
                };
                let album = if !track.album.is_empty() {
                    track.album.clone()
                } else {
                    track.title.clone()
                };
                Some((
                    "local".into(),
                    "album".into(),
                    json!({"artist": artist, "album": album}),
                    label("album", &album),
                ))
            }
            ("local", "folder") => Some((
                "local".into(),
                "folder".into(),
                json!({"path": track.id}),
                label("folder", &track.title),
            )),
            ("navidrome", "album") => {
                let id = if value("album_id").is_empty() {
                    track.id.clone()
                } else {
                    value("album_id").to_string()
                };
                Some((
                    "navidrome".into(),
                    "album".into(),
                    json!({"id": id}),
                    label("album", &track.title),
                ))
            }
            ("navidrome", "playlist") => {
                let id = if value("playlist_id").is_empty() {
                    track.id.clone()
                } else {
                    value("playlist_id").to_string()
                };
                Some((
                    "navidrome".into(),
                    "playlist".into(),
                    json!({"id": id}),
                    label("playlist", &track.title),
                ))
            }
            ("spotify", "album") => Some((
                "spotify".into(),
                "album".into(),
                json!({"id": track.id}),
                label("album", &track.title),
            )),
            ("spotify", "playlist") => Some((
                "spotify".into(),
                "playlist".into(),
                json!({"id": track.id}),
                label("playlist", &track.title),
            )),
            ("sr", "program") => {
                let id = if value("program_id").is_empty() {
                    track.id.clone()
                } else {
                    value("program_id").to_string()
                };
                Some((
                    "sr".into(),
                    "program".into(),
                    json!({"id": id}),
                    label("programme", &track.title),
                ))
            }
            ("library", "playlist") => {
                let id = track.id.parse::<i64>().ok()?;
                Some((
                    "library".into(),
                    "playlist".into(),
                    json!({"id": id}),
                    label("playlist", &track.title),
                ))
            }
            _ => None,
        }
    }

    pub(crate) fn expand_container(&mut self, track: &Track, action: ExpandAction) -> bool {
        let Some((source, section, args, label)) = self.container_request(track) else {
            self.toast(
                format!(
                    "Open this {} first; its tracks are resolved by NPLAY",
                    track.kind
                ),
                true,
            );
            return false;
        };
        self.request(
            "browse",
            json!({"source": source, "section": section, "limit": 1000, "args": args}),
            Some(PendingKind::Expansion { action, label }),
        );
        true
    }

    pub(crate) fn apply_expansion(&mut self, action: ExpandAction, items: &[Track], label: &str) {
        let tracks = playable_items(items);
        if tracks.is_empty() {
            self.toast(format!("{label} has no playable tracks"), true);
            return;
        }
        match action {
            ExpandAction::Play => self.play_group(&tracks, 0),
            ExpandAction::Queue => self.queue_add_many(&tracks, false),
            ExpandAction::PlayNext => self.queue_add_many(&tracks, true),
            ExpandAction::AddToPlaylist(id, name) => {
                let count = tracks.len();
                self.request(
                    "playlists.add_many",
                    json!({"id": id, "tracks": tracks}),
                    Some(PendingKind::Generic {
                        label: format!("Added {count} tracks to {name}"),
                        quiet: false,
                    }),
                );
            }
            ExpandAction::NewPlaylist => {
                self.pending_new_playlist_tracks = tracks;
                self.modal = Some(Modal::NewPlaylist {
                    name: String::new(),
                });
            }
        }
    }

    pub(crate) fn add_to_playlist(&mut self, track: &Track, playlist_id: i64, name: &str) {
        if !track.is_track() {
            self.expand_container(
                track,
                ExpandAction::AddToPlaylist(playlist_id, name.to_string()),
            );
            return;
        }
        self.request(
            "playlists.add",
            json!({"id": playlist_id, "track": track}),
            Some(PendingKind::Generic {
                label: format!("Added to {name}"),
                quiet: false,
            }),
        );
    }

    pub(crate) fn open_new_playlist(&mut self, track: Option<Track>) {
        self.pending_new_playlist_tracks = track.into_iter().collect();
        self.modal = Some(Modal::NewPlaylist {
            name: String::new(),
        });
    }

    pub(crate) fn new_playlist(&mut self, name: String) {
        self.modal = None;
        let tracks = std::mem::take(&mut self.pending_new_playlist_tracks);
        self.request(
            "playlists.create",
            json!({"name": name}),
            Some(PendingKind::PlaylistCreated { tracks }),
        );
    }

    pub(crate) fn favorite(&mut self, track: &Track) {
        self.request(
            "favorites.toggle",
            json!({"track": track}),
            Some(PendingKind::Generic {
                label: format!("Favorite · {}", track.title),
                quiet: false,
            }),
        );
    }

    pub(crate) fn retry_backend(&mut self, ctx: &Context) {
        self.reconnect(ctx);
    }

    // -- browser keyboard helpers ---------------------------------------
    pub(crate) fn browser_row_count(&self) -> usize {
        if self.browser.groups.is_empty() {
            self.browser.items.len()
        } else {
            self.browser.groups.iter().map(|g| g.items.len()).sum()
        }
    }

    pub(crate) fn active_list_len(&self) -> usize {
        match self.tab {
            Tab::Playlist => self.playback.context.len(),
            Tab::Queue => self.playback.queue.len(),
            Tab::Browse => self.browser_row_count(),
            _ => 0,
        }
    }

    pub(crate) fn activate_selected(&mut self) {
        match self.tab {
            Tab::Playlist => {
                let ctx_tracks = self.playback.context.clone();
                if !ctx_tracks.is_empty() {
                    self.play_group(&ctx_tracks, self.playlist_selected);
                }
            }
            Tab::Queue => {
                if !self.playback.queue.is_empty() {
                    let idx = self.queue_selected;
                    self.play_queue_index(idx);
                }
            }
            Tab::Browse => {
                if let Some(track) = self.browser_selected_track().cloned() {
                    self.play_now(&track);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn browser_selected_track(&self) -> Option<&Track> {
        if self.browser.groups.is_empty() {
            self.browser.items.get(self.browser.selected)
        } else {
            let mut idx = self.browser.selected;
            for group in &self.browser.groups {
                if idx < group.items.len() {
                    return group.items.get(idx);
                }
                idx = idx.saturating_sub(group.items.len());
            }
            None
        }
    }

    fn handle_keys(&mut self, ctx: &Context) {
        if self.backend_error.is_some() {
            if ctx.input(|i| i.key_pressed(Key::Enter)) {
                self.retry_backend(ctx);
            }
            return;
        }
        if ctx.input(|i| i.key_pressed(Key::F11)) {
            self.toggle_immersive(ctx);
            return;
        }
        if self.immersive && ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.toggle_immersive(ctx);
            return;
        }
        if self.modal.is_some() {
            if ctx.input(|i| i.key_pressed(Key::Escape)) {
                self.modal = None;
            }
            return;
        }
        let focused = ctx.memory(|m| m.focused().is_some());
        let typing = self.text_input_active;
        let shift = ctx.input(|i| i.modifiers.shift);
        let alt = ctx.input(|i| i.modifiers.alt);
        let ctrl = ctx.input(|i| i.modifiers.command || i.modifiers.ctrl);
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            if typing || focused {
                ctx.memory_mut(|m| m.surrender_focus(Id::NULL));
            } else if self.tab != Tab::Playlist {
                self.tab = Tab::Playlist;
            }
            return;
        }
        if typing {
            return;
        }
        if ctrl && ctx.input(|i| i.key_pressed(Key::F)) {
            self.tab = Tab::Browse;
            self.browser.focus_search = true;
        }
        if ctx.input(|i| i.key_pressed(Key::Slash)) {
            self.tab = Tab::Browse;
            self.browser.focus_search = true;
        }
        if ctx.input(|i| i.key_pressed(Key::Space)) {
            self.toggle();
        }
        if ctx.input(|i| i.key_pressed(Key::S)) {
            self.stop();
        }
        if ctx.input(|i| i.key_pressed(Key::N)) {
            self.next();
        }
        if ctx.input(|i| i.key_pressed(Key::P)) {
            self.previous();
        }
        if ctx.input(|i| i.key_pressed(Key::M)) {
            self.mute();
        }
        if ctx.input(|i| i.key_pressed(Key::R)) {
            self.repeat();
        }
        if ctx.input(|i| i.key_pressed(Key::H)) {
            self.shuffle();
        }
        if ctx.input(|i| i.key_pressed(Key::V)) {
            self.prefs.show_visualization = !self.prefs.show_visualization;
            self.update_prefs(ctx);
        }
        if ctx.input(|i| i.key_pressed(Key::F)) {
            if let Some(t) = self.playback.current.clone() {
                self.favorite(&t);
            }
        }
        if ctx.input(|i| i.key_pressed(Key::B)) {
            self.tab = Tab::Browse;
        }
        if ctx.input(|i| i.key_pressed(Key::Num1)) {
            self.show_tab(Tab::Playlist, ctx);
        }
        if ctx.input(|i| i.key_pressed(Key::Num2)) {
            self.show_tab(Tab::Queue, ctx);
        }
        if ctx.input(|i| i.key_pressed(Key::Num3)) {
            self.show_tab(Tab::Browse, ctx);
        }
        if ctx.input(|i| i.key_pressed(Key::Num4)) {
            self.show_tab(Tab::Settings, ctx);
        }
        if ctx.input(|i| i.key_pressed(Key::Num5)) {
            self.show_tab(Tab::About, ctx);
        }
        if ctx.input(|i| i.key_pressed(Key::W)) {
            self.toggle_shade(ctx);
        }
        if ctx.input(|i| i.key_pressed(Key::T)) {
            self.toggle_pin(ctx);
        }
        if ctx.input(|i| i.key_pressed(Key::ArrowRight)) {
            self.seek_relative(if shift { 30.0 } else { 5.0 });
        }
        if ctx.input(|i| i.key_pressed(Key::ArrowLeft)) {
            self.seek_relative(if shift { -30.0 } else { -5.0 });
        }
        if ctx.input(|i| i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals)) {
            self.volume_delta(5.0);
        }
        if ctx.input(|i| i.key_pressed(Key::Minus)) {
            self.volume_delta(-5.0);
        }
        let del = ctx.input(|i| i.key_pressed(Key::Delete));
        let enter = ctx.input(|i| i.key_pressed(Key::Enter));
        let up = ctx.input(|i| i.key_pressed(Key::ArrowUp));
        let down = ctx.input(|i| i.key_pressed(Key::ArrowDown));
        if up || down {
            self.keyboard_nav = true;
            let delta: isize = if down { 1 } else { -1 };
            match self.tab {
                Tab::Playlist => {
                    let len = self.playback.context.len();
                    if len > 0 {
                        self.playlist_selected = (self.playlist_selected as isize + delta)
                            .clamp(0, len as isize - 1)
                            as usize;
                    }
                }
                Tab::Queue => {
                    if alt {
                        if up || down {
                            self.move_queue(self.queue_selected, delta as i64);
                        }
                    } else {
                        let len = self.playback.queue.len();
                        if len > 0 {
                            self.queue_selected = (self.queue_selected as isize + delta)
                                .clamp(0, len as isize - 1)
                                as usize;
                        }
                    }
                }
                Tab::Browse => {
                    let len = self.browser_row_count();
                    if len > 0 {
                        self.browser.selected = (self.browser.selected as isize + delta)
                            .clamp(0, len as isize - 1)
                            as usize;
                    }
                }
                _ => {}
            }
        }
        if enter && !focused {
            self.activate_selected();
        }
        if del && self.tab == Tab::Queue {
            let idx = self.queue_selected;
            if idx < self.playback.queue.len() {
                self.remove_queue(idx);
            }
        }
    }

    fn draw_title_and_body(&mut self, ctx: &Context) {
        let p = self.palette();
        egui::CentralPanel::default()
            .frame(
                Frame::none()
                    .fill(p.background)
                    .inner_margin(egui::Margin::same(0.0)),
            )
            .show(ctx, |ui| {
                if self.backend_error.is_some() && self.backend.is_none() {
                    self.draw_dependency_error(ui, ctx);
                    return;
                }
                let width = ui.available_width();
                let margin = self.s(8.0);
                ui.add_space(margin);
                let deck_rect = Rect::from_min_size(
                    ui.cursor().min + egui::vec2(margin, 0.0),
                    egui::vec2(width - margin * 2.0, self.deck_height),
                );
                let mut deck_ui =
                    ui.child_ui(deck_rect, egui::Layout::top_down(egui::Align::Min), None);
                self.deck_ui(&mut deck_ui, ctx);
                self.deck_height = deck_ui.min_rect().height();
                ui.add_space(self.deck_height);
                if !self.prefs.window_shade {
                    ui.add_space(margin * 0.75);
                    let rest = ui.available_size();
                    let panel_rect = Rect::from_min_size(
                        ui.cursor().min,
                        egui::vec2(width - margin * 2.0, rest.y - margin),
                    );
                    ui.allocate_ui_at_rect(panel_rect, |ui| {
                        self.panel_ui(ui, ctx);
                    });
                }
                if !self.prefs.window_shade {
                    let size = self.s(14.0);
                    let grip = Rect::from_min_size(
                        ui.max_rect().right_bottom() - egui::vec2(size, size),
                        egui::vec2(size, size),
                    );
                    let resp = ui.interact(
                        grip,
                        Id::new("niruamp-resize-grip"),
                        Sense {
                            click: true,
                            drag: true,
                            focusable: false,
                        },
                    );
                    if resp.dragged() {
                        let delta = ctx.input(|i| i.pointer.delta());
                        if let Some(inner) = ctx.input(|i| i.viewport().inner_rect) {
                            let target = egui::vec2(
                                (inner.width() + delta.x).max(self.s(400.0)),
                                (inner.height() + delta.y).max(self.s(120.0)),
                            );
                            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(target));
                        }
                    }
                    if resp.hovered() || resp.dragged() {
                        ctx.set_cursor_icon(egui::CursorIcon::ResizeSouthEast);
                    }
                    let p = self.palette();
                    for i in 0..3 {
                        let off = self.s(3.0) + i as f32 * self.s(3.0);
                        ui.painter().line_segment(
                            [
                                grip.right_bottom() - egui::vec2(off, self.s(2.0)),
                                grip.right_bottom() - egui::vec2(self.s(2.0), off),
                            ],
                            Stroke::new(1.0_f32, p.muted),
                        );
                    }
                }
            });
    }

    fn draw_dependency_error(&mut self, ui: &mut Ui, ctx: &Context) {
        let p = self.palette();
        let margin = self.s(16.0);
        ui.add_space(margin);
        ui.horizontal(|ui| {
            ui.add_space(margin);
            ui.vertical(|ui| {
                ui.set_max_width(ui.available_width() - margin);
                let err = self.backend_error.clone().unwrap();
                ui.label(RichText::new("NPLAY REQUIRED").size(self.s(11.0)).color(p.warning).strong());
                ui.label(RichText::new(err.title).size(self.s(22.0)).color(p.foreground).strong());
                ui.add_space(8.0);
                ui.label(RichText::new(err.message).size(self.s(14.0)).color(p.foreground));
                ui.add_space(10.0);
                Frame::none().fill(p.inset).inner_margin(egui::Margin::same(12.0)).show(ui, |ui| {
                    ui.label(RichText::new(err.remedy).size(self.s(13.0)).color(p.muted));
                });
                if !err.log_tail.is_empty() {
                    ui.add_space(8.0);
                    ui.label(RichText::new("NPLAY service log").size(self.s(10.0)).color(p.muted).strong());
                    Frame::none().fill(p.inset).inner_margin(egui::Margin::same(8.0)).show(ui, |ui| {
                        ui.add(egui::Label::new(RichText::new(&err.log_tail).monospace().size(self.s(10.0)).color(p.muted)).wrap());
                    });
                }
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new(RichText::new("RETRY CONNECTION").strong())).clicked() {
                        self.retry_backend(ctx);
                    }
                    if ui.add(egui::Button::new("QUIT")).clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.add_space(8.0);
                ui.label(RichText::new("NIRUAMP never bundles a second playback engine. NPLAY owns playback, queue, library and statistics.").size(self.s(11.0)).color(p.disabled));
            });
        });
    }

    fn draw_toasts(&mut self, ctx: &Context) {
        if self.toasts.is_empty() {
            return;
        }
        let now = Instant::now();
        self.toasts
            .retain(|t| now.duration_since(t.created) < Duration::from_secs(6));
        if self.toasts.is_empty() {
            return;
        }
        let p = self.palette();
        let layer = LayerId::new(Order::Foreground, Id::new("niruamp-toasts"));
        let painter = ctx.layer_painter(layer);
        let screen = ctx.screen_rect();
        let width = (screen.width() * 0.72).min(self.s(520.0));
        let mut y = screen.bottom() - self.s(18.0);
        for toast in self.toasts.iter().rev() {
            let age = now.duration_since(toast.created).as_secs_f32();
            let alpha = if age > 4.5 {
                ((6.0 - age) / 1.5).clamp(0.0, 1.0)
            } else {
                1.0
            };
            let rect = Rect::from_min_size(
                egui::pos2(screen.center().x - width / 2.0, y - self.s(34.0)),
                egui::vec2(width, self.s(30.0)),
            );
            let fill = if toast.error { p.error } else { p.surface_hi };
            let text_color = if toast.error {
                Color32::WHITE
            } else {
                p.foreground
            };
            painter.rect_filled(rect, egui::Rounding::same(2.0), fill.gamma_multiply(alpha));
            painter.rect_stroke(
                rect,
                egui::Rounding::same(2.0),
                Stroke::new(1.0_f32, p.border.gamma_multiply(alpha)),
            );
            painter.text(
                rect.left_center() + egui::vec2(self.s(10.0), 0.0),
                Align2::LEFT_CENTER,
                &toast.text,
                egui::FontId::proportional(self.s(12.0)),
                text_color.gamma_multiply(alpha),
            );
            y -= self.s(38.0);
        }
    }

    fn draw_modal(&mut self, ctx: &Context) {
        let Some(mut modal) = self.modal.take() else {
            return;
        };
        let p = self.palette();
        let scale = self.prefs.scale;
        let s = move |v: f32| v * scale;
        let mut close = false;
        let mut submit = false;
        let mut text_focus = false;
        let is_save_queue = matches!(&modal, Modal::SaveQueue { .. });
        egui::Area::new(Id::new("niruamp-modal"))
            .order(Order::Foreground)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                Frame::popup(&ctx.style())
                    .fill(p.panel)
                    .stroke(Stroke::new(1.0_f32, p.border))
                    .inner_margin(egui::Margin::same(16.0))
                    .show(ui, |ui| {
                        ui.set_min_width(s(360.0));
                        match &mut modal {
                            Modal::NewPlaylist { name } | Modal::SaveQueue { name } => {
                                let title = if is_save_queue { "SAVE QUEUE AS PLAYLIST" } else { "NEW PLAYLIST" };
                                ui.label(RichText::new(title).strong().color(p.foreground));
                                ui.add_space(6.0);
                                let resp = ui.add(egui::TextEdit::singleline(name).hint_text("Playlist name"));
                                if !resp.has_focus() && resp.ctx.memory(|m| m.focused().is_none()) {
                                    resp.request_focus();
                                }
                                text_focus |= resp.has_focus();
                                text_focus |= resp.has_focus();
                                ui.add_space(8.0);
                                ui.horizontal(|ui| {
                                    submit = ui.button("SAVE").clicked();
                                    close = ui.button("CANCEL").clicked();
                                });
                            }
                            Modal::RenamePlaylist { name, .. } => {
                                ui.label(RichText::new("RENAME PLAYLIST").strong().color(p.foreground));
                                ui.add_space(6.0);
                                let resp = ui.add(egui::TextEdit::singleline(name).hint_text("Playlist name"));
                                if !resp.has_focus() && resp.ctx.memory(|m| m.focused().is_none()) {
                                    resp.request_focus();
                                }
                                text_focus |= resp.has_focus();
                                text_focus |= resp.has_focus();
                                ui.add_space(8.0);
                                ui.horizontal(|ui| {
                                    submit = ui.button("RENAME").clicked();
                                    close = ui.button("CANCEL").clicked();
                                });
                            }
                            Modal::Navidrome { url, user, password } => {
                                ui.label(RichText::new("CONFIGURE NAVIDROME").strong().color(p.foreground));
                                ui.label(RichText::new("Credentials are stored by NPLAY with mode 0600 and are never returned to this UI.").small().color(p.muted));
                                ui.add_space(6.0);
                                text_focus |= ui.add(egui::TextEdit::singleline(url).hint_text("https://music.example.org")).has_focus();
                                text_focus |= ui.add(egui::TextEdit::singleline(user).hint_text("Username")).has_focus();
                                text_focus |= ui.add(egui::TextEdit::singleline(password).hint_text("Password").password(true)).has_focus();
                                ui.add_space(8.0);
                                ui.horizontal(|ui| {
                                    submit = ui.button("CONNECT").clicked();
                                    close = ui.button("CANCEL").clicked();
                                });
                            }
                            Modal::AddRadio { name, url } => {
                                ui.label(RichText::new("ADD CUSTOM RADIO").strong().color(p.foreground));
                                ui.add_space(6.0);
                                text_focus |= ui.add(egui::TextEdit::singleline(name).hint_text("Station name")).has_focus();
                                text_focus |= ui.add(egui::TextEdit::singleline(url).hint_text("https://stream.example.org/live.mp3")).has_focus();
                                ui.add_space(8.0);
                                ui.horizontal(|ui| {
                                    submit = ui.button("ADD STATION").clicked();
                                    close = ui.button("CANCEL").clicked();
                                });
                            }
                        }
                    });
            });
        self.text_input_active = text_focus;
        if close {
            if matches!(modal, Modal::NewPlaylist { .. }) {
                self.pending_new_playlist_tracks.clear();
            }
            self.modal = None;
            return;
        }
        if submit {
            match modal {
                Modal::NewPlaylist { name } => {
                    let name = name.trim().to_string();
                    if name.is_empty() {
                        self.toast("Playlist name is required", true);
                        self.modal = Some(Modal::NewPlaylist { name });
                    } else {
                        self.new_playlist(name);
                    }
                }
                Modal::SaveQueue { name } => {
                    let name = name.trim().to_string();
                    if name.is_empty() {
                        self.toast("Playlist name is required", true);
                        self.modal = Some(Modal::SaveQueue { name });
                    } else {
                        self.request(
                            "queue.save",
                            json!({"name": name}),
                            Some(PendingKind::Generic {
                                label: "Queue saved as playlist".into(),
                                quiet: false,
                            }),
                        );
                        self.request("playlists.list", json!({}), Some(PendingKind::Playlists));
                    }
                }
                Modal::RenamePlaylist { id, name } => {
                    let name = name.trim().to_string();
                    if name.is_empty() {
                        self.toast("Playlist name is required", true);
                        self.modal = Some(Modal::RenamePlaylist { id, name });
                    } else {
                        self.request(
                            "playlists.rename",
                            json!({"id": id, "name": name}),
                            Some(PendingKind::Generic {
                                label: "Playlist renamed".into(),
                                quiet: false,
                            }),
                        );
                        self.request("playlists.list", json!({}), Some(PendingKind::Playlists));
                    }
                }
                Modal::Navidrome {
                    url,
                    user,
                    password,
                } => {
                    self.request(
                        "navidrome.setup",
                        json!({"url": url.trim(), "user": user.trim(), "password": password}),
                        Some(PendingKind::Generic {
                            label: "Navidrome connected".into(),
                            quiet: false,
                        }),
                    );
                    self.load_settings();
                }
                Modal::AddRadio { name, url } => {
                    self.request(
                        "radio.add",
                        json!({"name": name.trim(), "url": url.trim()}),
                        Some(PendingKind::Generic {
                            label: "Radio station saved".into(),
                            quiet: false,
                        }),
                    );
                    self.modal = None;
                }
            }
        } else {
            self.modal = Some(modal);
        }
    }

    fn handle_dropped_files(&mut self, ctx: &Context) {
        let files = ctx.input(|i| i.raw.dropped_files.clone());
        if files.is_empty() || self.backend.is_none() {
            return;
        }
        let mut first = true;
        for (index, file) in files.into_iter().enumerate() {
            let mut path = file.path.clone();
            if path.is_none() {
                if let Some(bytes) = file.bytes.as_ref() {
                    let dir = crate::config::state_home().join("niruamp/drops");
                    let _ = std::fs::create_dir_all(&dir);
                    let name = if file.name.is_empty() {
                        format!("drop-{index}")
                    } else {
                        file.name.clone()
                    };
                    let safe: String = name
                        .chars()
                        .map(|c| {
                            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                                c
                            } else {
                                '_'
                            }
                        })
                        .collect();
                    let target = dir.join(safe);
                    if std::fs::write(&target, bytes).is_ok() {
                        path = Some(target);
                    }
                }
            }
            if let Some(path) = path {
                let title = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Dropped media")
                    .to_string();
                let params = if first {
                    json!({"path": path, "title": title})
                } else {
                    json!({"path": path, "title": title, "queue": true})
                };
                let kind = if first {
                    PendingKind::Play
                } else {
                    PendingKind::Generic {
                        label: "Queued dropped file".into(),
                        quiet: false,
                    }
                };
                self.request("playback.play_external", params, Some(kind));
                first = false;
            } else if !file.name.is_empty() && file.name.starts_with("http") {
                let params = if first {
                    json!({"url": file.name})
                } else {
                    json!({"url": file.name, "queue": true})
                };
                let kind = if first {
                    PendingKind::Play
                } else {
                    PendingKind::Generic {
                        label: "Queued stream".into(),
                        quiet: false,
                    }
                };
                self.request("playback.play_external", params, Some(kind));
                first = false;
            }
        }
        if !first {
            self.toast("Dropped media sent to NPLAY", false);
        }
    }

    fn draw_screenshot_hook(&mut self, ctx: &Context) {
        let Some(path) = self.screenshot_path.clone() else {
            return;
        };
        let due = self
            .screenshot_at
            .map(|at| Instant::now() >= at)
            .unwrap_or(true);
        if due {
            eprintln!("niruamp: requesting screenshot to {}", path.display());
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
        }
        let shot = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = shot {
            eprintln!(
                "niruamp: screenshot event {}x{}",
                image.size[0], image.size[1]
            );
            let [w, h] = image.size;
            let mut rgba = Vec::with_capacity(w * h * 4);
            for pixel in &image.pixels {
                rgba.extend_from_slice(&[pixel.r(), pixel.g(), pixel.b(), pixel.a()]);
            }
            if let Some(buffer) = image::RgbaImage::from_raw(w as u32, h as u32, rgba) {
                if let Err(e) = buffer.save(&path) {
                    eprintln!("niruamp: screenshot save failed: {e}");
                    self.toast(format!("Cannot save screenshot: {e}"), true);
                } else {
                    eprintln!("niruamp: screenshot saved {}", path.display());
                    self.toast(format!("Screenshot saved to {}", path.display()), false);
                }
            }
            self.screenshot_path = None;
        }
    }
}

impl eframe::App for NiruampApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        if self.debug_fps {
            self.fps_frames += 1;
            if self.fps_start.elapsed() >= Duration::from_secs(5) {
                eprintln!(
                    "niruamp: fps={:.1} messages/s={:.1}",
                    self.fps_frames as f64 / self.fps_start.elapsed().as_secs_f64(),
                    self.debug_messages as f64 / self.fps_start.elapsed().as_secs_f64()
                );
                self.fps_frames = 0;
                self.debug_messages = 0;
                self.fps_start = Instant::now();
            }
        }
        self.drain_backend(ctx);
        self.drain_tray(ctx);
        self.handle_dropped_files(ctx);
        self.handle_keys(ctx);
        self.text_input_active = false;
        self.advance_visualization();
        if self.immersive {
            self.draw_immersive(ctx);
        } else {
            self.draw_title_and_body(ctx);
            self.draw_modal(ctx);
            self.draw_toasts(ctx);
            self.draw_screenshot_hook(ctx);
        }
        if self.playback.status == "playing" {
            // The LED shows whole seconds; one bounded clock tick per second is
            // enough and avoids a continuous repaint loop on software renderers.
            ctx.request_repaint_after(Duration::from_millis(1000));
        } else if self.playback.status == "loading" {
            ctx.request_repaint_after(Duration::from_millis(400));
        }
        if self.vis.started
            && !self.reduce_motion
            && (self.prefs.show_visualization || self.immersive)
        {
            ctx.request_repaint_after(Duration::from_millis(33));
        }
        if !self.toasts.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
        if let Some(due) = self.browser.search_due {
            let wait = due.saturating_duration_since(Instant::now());
            ctx.request_repaint_after(wait + Duration::from_millis(10));
        }
        if self.screenshot_path.is_some() {
            ctx.request_repaint();
        }
        if !self.startup_sound_done {
            self.startup_sound_done = true;
            if self.prefs.startup_sound {
                crate::sound::play_startup();
            }
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(tray) = self.tray.take() {
            tray.shutdown();
        }
        self.window_state.save().ok();
        self.prefs.save().ok();
        self.request("visualization.stop", json!({}), None);
        if let Some(b) = self.backend.take() {
            b.shutdown(self.prefs.keep_playing);
        }
    }
}

pub(crate) fn decode_artwork(data: &str) -> Result<egui::ColorImage, String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.as_bytes())
        .map_err(|e| e.to_string())?;
    let image = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
    let image = if image.width() > 256 || image.height() > 256 {
        image.resize(256, 256, image::imageops::FilterType::Triangle)
    } else {
        image
    };
    let rgba = image.to_rgba8();
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        [rgba.width() as usize, rgba.height() as usize],
        rgba.as_raw(),
    ))
}

pub(crate) fn save_tab(prefs: &mut Preferences, tab: Tab) {
    prefs.active_tab = tab.key().to_string();
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    #[test]
    fn decodes_small_png_artwork() {
        let image = image::RgbaImage::from_pixel(8, 6, image::Rgba([10, 20, 30, 255]));
        let mut bytes = Vec::new();
        image
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
        let decoded = decode_artwork(&encoded).unwrap();
        assert_eq!(decoded.size, [8, 6]);
        assert_eq!(decoded.pixels.len(), 48);
    }

    #[test]
    fn artwork_is_downscaled_to_a_small_texture() {
        let image = image::RgbaImage::from_pixel(1024, 768, image::Rgba([1, 2, 3, 255]));
        let mut bytes = Vec::new();
        image
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
        let decoded = decode_artwork(&encoded).unwrap();
        assert!(decoded.size[0] <= 256 && decoded.size[1] <= 256);
        assert_eq!(decoded.size, [256, 192]);
    }

    #[test]
    fn invalid_artwork_is_reported() {
        assert!(decode_artwork("not base64!!").is_err());
    }
}
