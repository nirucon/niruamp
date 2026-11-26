//! Optional one-shot UI welcome sound.
//!
//! This is deliberately not routed through NPLAY: it is a short interface
//! effect, not media, so it must not enter the queue, statistics, MPRIS or the
//! backend playback session. The embedded Vorbis file is written once into the
//! user cache and played by a tiny system utility (or mpv, which NPLAY already
//! requires). No long-lived audio process is added.
//!
//! It plays once per login: a marker in `$XDG_RUNTIME_DIR/niruamp` (cleared
//! at logout) records the boot and session it played in, so later starts in
//! the same session stay silent.
use crate::config;
use std::fs;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const STARTUP_OGG: &[u8] = include_bytes!("../assets/niruamp-startup.ogg");

/// Volume steps offered in Settings (percent of the sound's full level).
pub const VOLUME_CHOICES: [u32; 5] = [20, 35, 50, 75, 100];
pub const DEFAULT_VOLUME: u32 = 35;

/// Play the welcome sound if it has not played in this login session yet.
pub fn play_welcome(volume: u32) {
    if std::env::var_os("NIRUAMP_SKIP_STARTUP_SOUND").is_some() {
        return;
    }
    if !first_in_session(&marker_path(), &session_key()) {
        return;
    }
    play(volume);
}

/// Play the sound now (the Settings preview), at `volume` percent.
pub fn play(volume: u32) {
    let volume = volume.clamp(1, 100);
    std::thread::Builder::new()
        .name("niruamp-startup-sound".into())
        .spawn(move || {
            let Some(path) = ensure_asset() else { return };
            for mut command in players(&path, volume) {
                if let Ok(mut child) = command
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                {
                    if child.wait().map(|s| s.success()).unwrap_or(false) {
                        return;
                    }
                }
            }
        })
        .ok();
}

fn marker_path() -> PathBuf {
    config::runtime_home()
        .join("niruamp")
        .join("welcome-played")
}

/// This boot and login session. `$XDG_RUNTIME_DIR` is removed at logout, so
/// the marker normally goes with it; the key also covers a runtime directory
/// that outlives a login (lingering) and the state-directory fallback.
fn session_key() -> String {
    let boot = fs::read_to_string("/proc/sys/kernel/random/boot_id").unwrap_or_default();
    let session = std::env::var("XDG_SESSION_ID").unwrap_or_default();
    format!("{} {}", boot.trim(), session.trim())
}

/// True (and the marker written) when the marker does not hold `key` yet.
fn first_in_session(marker: &Path, key: &str) -> bool {
    if fs::read_to_string(marker).is_ok_and(|text| text.trim() == key.trim()) {
        return false;
    }
    if let Some(dir) = marker.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let _ = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(marker)
        .and_then(|mut f| std::io::Write::write_all(&mut f, key.as_bytes()));
    true
}

fn ensure_asset() -> Option<PathBuf> {
    let dir = config::cache_home().join("niruamp");
    fs::create_dir_all(&dir).ok()?;
    let path = dir.join("startup.ogg");
    if fs::metadata(&path).map(|m| m.len()).ok() == Some(STARTUP_OGG.len() as u64) {
        return Some(path);
    }
    let tmp = dir.join(format!("startup.ogg.{}.tmp", std::process::id()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&tmp)
        .ok()?;
    use std::io::Write;
    file.write_all(STARTUP_OGG).ok()?;
    file.sync_all().ok()?;
    fs::rename(&tmp, &path).ok()?;
    Some(path)
}

fn in_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

/// Player commands for `volume` percent of the sound's amplitude. pw-play
/// and ffplay scale linearly; paplay (`pa_volume_t`, 65536 = 100 %) and mpv
/// use a cubic volume scale, so they get the cube root to sound the same.
fn players(path: &Path, volume: u32) -> Vec<Command> {
    players_with(path, volume, |name| in_path(name).is_some())
}

fn players_with(path: &Path, volume: u32, installed: impl Fn(&str) -> bool) -> Vec<Command> {
    let gain = volume.clamp(1, 100) as f64 / 100.0;
    let cubic = gain.cbrt();
    let mut out = Vec::new();
    if installed("pw-play") {
        let mut command = Command::new("pw-play");
        command
            .arg("--media-role=Notification")
            .arg(format!("--volume={gain:.3}"))
            .arg(path);
        out.push(command);
    }
    if installed("paplay") {
        let mut command = Command::new("paplay");
        command
            .arg(format!("--volume={}", (cubic * 65536.0).round() as u32))
            .arg(path);
        out.push(command);
    }
    if installed("ffplay") {
        let mut command = Command::new("ffplay");
        command
            .args(["-nodisp", "-autoexit", "-loglevel", "quiet", "-volume"])
            .arg(format!("{}", (gain * 100.0).round() as u32))
            .arg(path);
        out.push(command);
    }
    if installed("mpv") {
        let mut command = Command::new("mpv");
        command
            .args(["--no-video", "--no-terminal", "--really-quiet"])
            .arg(format!("--volume={}", (cubic * 100.0).round() as u32))
            .arg(path);
        out.push(command);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_startup_sound_is_ogg_vorbis() {
        assert!(STARTUP_OGG.len() > 10_000);
        assert_eq!(&STARTUP_OGG[..4], b"OggS");
    }

    #[test]
    fn welcome_plays_once_per_session() {
        let dir = std::env::temp_dir().join(format!("niruamp-sound-{}", std::process::id()));
        let marker = dir.join("niruamp").join("welcome-played");
        assert!(first_in_session(&marker, "boot-a 3"));
        assert!(!first_in_session(&marker, "boot-a 3"));
        assert!(!first_in_session(&marker, "boot-a 3"));
        // A new login (or boot) plays it again.
        assert!(first_in_session(&marker, "boot-a 4"));
        assert!(first_in_session(&marker, "boot-b 4"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn every_player_gets_the_same_level() {
        let args: Vec<String> = players_with(Path::new("x.ogg"), DEFAULT_VOLUME, |_| true)
            .iter()
            .map(|c| {
                let mut v = vec![c.get_program().to_string_lossy().to_string()];
                v.extend(c.get_args().map(|a| a.to_string_lossy().to_string()));
                v.join(" ")
            })
            .collect();
        assert_eq!(args.len(), 4, "{args:?}");
        assert!(args[0].contains("--volume=0.350"), "{}", args[0]);
        // 0.35 linear: cube root 0.705 of PulseAudio's 65536 and mpv's 100.
        assert!(args[1].contains("--volume=46185"), "{}", args[1]);
        assert!(args[2].ends_with("-volume 35 x.ogg"), "{}", args[2]);
        assert!(args[3].contains("--volume=70"), "{}", args[3]);
    }
}
