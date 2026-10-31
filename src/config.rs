//! NIRUAMP-owned preferences and window state. Playback data stays in NPLAY.
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

pub fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
}

pub fn config_home() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| home().join(".config"))
}

pub fn state_home() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| home().join(".local/state"))
}

pub fn data_home() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| home().join(".local/share"))
}

pub fn runtime_home() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| state_home().join("niruamp/run"))
}

pub fn config_dir() -> PathBuf {
    config_home().join("niruamp")
}

pub fn state_dir() -> PathBuf {
    state_home().join("niruamp")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn window_path() -> PathBuf {
    state_dir().join("window.toml")
}

pub fn service_log_path() -> PathBuf {
    state_dir().join("nplay-service.log")
}

pub fn instance_socket() -> PathBuf {
    runtime_home().join("niruamp").join("instance.sock")
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    /// `auto` follows NIRUX, `classic` is the standalone fallback, or a theme name.
    pub theme: String,
    pub scale: f32,
    pub always_on_top: bool,
    pub keep_playing: bool,
    pub show_visualization: bool,
    /// `None` follows the NIRUX motion setting.
    pub reduced_motion: Option<bool>,
    pub remaining_time: bool,
    pub active_tab: String,
    pub playlist_visible: bool,
    pub window_shade: bool,
    pub nplay_bin: String,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: "auto".into(),
            scale: 1.0,
            always_on_top: false,
            keep_playing: false,
            show_visualization: true,
            reduced_motion: None,
            remaining_time: false,
            active_tab: "playlist".into(),
            playlist_visible: true,
            window_shade: false,
            nplay_bin: "nplay".into(),
        }
    }
}

impl Preferences {
    pub fn load() -> Self {
        let path = config_path();
        let mut prefs = fs::read_to_string(&path)
            .ok()
            .and_then(|text| toml::from_str::<Self>(&text).ok())
            .unwrap_or_default();
        prefs.scale = if prefs.scale.is_finite() {
            prefs.scale.clamp(0.75, 1.6)
        } else {
            1.0
        };
        if prefs.theme.trim().is_empty() || prefs.theme.len() > 80 {
            prefs.theme = "auto".into();
        }
        if prefs.nplay_bin.trim().is_empty() {
            prefs.nplay_bin = "nplay".into();
        }
        if !matches!(
            prefs.active_tab.as_str(),
            "playlist" | "queue" | "browse" | "settings" | "about"
        ) {
            prefs.active_tab = "playlist".into();
        }
        prefs
    }

    pub fn save(&self) -> Result<(), String> {
        atomic_toml(&config_path(), self)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowState {
    pub width: f32,
    pub height: f32,
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub normal_width: f32,
    pub normal_height: f32,
}

impl WindowState {
    pub fn load() -> Self {
        let mut state: Self = fs::read_to_string(window_path())
            .ok()
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default();
        if !state.width.is_finite() || state.width < 320.0 {
            state.width = 620.0;
        }
        if !state.height.is_finite() || state.height < 80.0 {
            state.height = 560.0;
        }
        if !state.normal_width.is_finite() || state.normal_width < 320.0 {
            state.normal_width = state.width;
        }
        if !state.normal_height.is_finite() || state.normal_height < 80.0 {
            state.normal_height = state.height;
        }
        state
    }

    pub fn save(&self) -> Result<(), String> {
        atomic_toml(&window_path(), self)
    }
}

fn ensure_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|e| format!("cannot create {}: {e}", path.display()))?;
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o700));
    Ok(())
}

pub fn atomic_toml<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "path has no parent".to_string())?;
    ensure_dir(parent)?;
    let text = toml::to_string_pretty(value).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("toml.tmp");
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&tmp)
        .map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
    f.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
    f.sync_all().map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| format!("cannot replace {}: {e}", path.display()))?;
    Ok(())
}
