//! Optional one-shot UI startup sound.
//!
//! This is deliberately not routed through NPLAY: it is a short interface
//! effect, not media, so it must not enter the queue, statistics, MPRIS or the
//! backend playback session. The embedded Vorbis file is written once into the
//! user cache and played by a tiny system utility (or mpv, which NPLAY already
//! requires). No long-lived audio process is added.
use crate::config;
use std::fs;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const STARTUP_OGG: &[u8] = include_bytes!("../assets/niruamp-startup.ogg");

pub fn play_startup() {
    if std::env::var_os("NIRUAMP_SKIP_STARTUP_SOUND").is_some() {
        return;
    }
    std::thread::Builder::new()
        .name("niruamp-startup-sound".into())
        .spawn(|| {
            let Some(path) = ensure_asset() else { return };
            for mut command in players(&path) {
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

fn players(path: &Path) -> Vec<Command> {
    let mut out = Vec::new();
    if in_path("pw-play").is_some() {
        let mut command = Command::new("pw-play");
        command
            .arg("--media-role=Notification")
            .arg("--volume=0.65")
            .arg(path);
        out.push(command);
    }
    if in_path("paplay").is_some() {
        let mut command = Command::new("paplay");
        command.arg(path);
        out.push(command);
    }
    if in_path("ffplay").is_some() {
        let mut command = Command::new("ffplay");
        command
            .args([
                "-nodisp",
                "-autoexit",
                "-loglevel",
                "quiet",
                "-volume",
                "65",
            ])
            .arg(path);
        out.push(command);
    }
    if in_path("mpv").is_some() {
        let mut command = Command::new("mpv");
        command
            .args([
                "--no-video",
                "--no-terminal",
                "--really-quiet",
                "--volume=65",
            ])
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
}
