//! Process, dependency and single-instance integration.
use crate::config::{instance_socket, state_dir, Preferences};
use crate::model::Snapshot;
use crate::protocol::{Client, ClientError};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

pub fn backend_socket_path() -> PathBuf {
    if let Some(p) = std::env::var_os("NPLAY_SERVICE_SOCKET") {
        return PathBuf::from(p);
    }
    if let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR").filter(|v| !v.is_empty()) {
        return PathBuf::from(runtime).join("nplay/control.sock");
    }
    if let Some(state) = std::env::var_os("XDG_STATE_HOME").filter(|v| !v.is_empty()) {
        return PathBuf::from(state).join("nplay/run/control.sock");
    }
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
        .join(".local/state/nplay/run/control.sock")
}

#[derive(Debug, Clone)]
pub struct DependencyError {
    pub title: String,
    pub message: String,
    pub remedy: String,
    pub log_tail: String,
}

impl DependencyError {
    fn new(title: &str, message: impl Into<String>, remedy: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            remedy: remedy.into(),
            log_tail: String::new(),
        }
    }
}

pub struct BackendHandle {
    pub client: Client,
    pub snapshot: Snapshot,
    pub owned: bool,
    pub socket: PathBuf,
}

impl BackendHandle {
    pub fn revision(&self) -> u64 {
        self.snapshot.revision
    }

    pub fn shutdown(self, keep_playing: bool) {
        if self.owned && !keep_playing {
            let _ = self.client.send("service.shutdown", serde_json::json!({}));
            // One-shot at exit, not a watch loop: give the writer thread a
            // moment to deliver the request before the process ends.
            thread::sleep(Duration::from_millis(250));
        }
    }
}

fn find_nplay(prefs: &Preferences) -> Result<(PathBuf, Option<PathBuf>), DependencyError> {
    let configured = prefs.nplay_bin.trim();
    if configured.contains('/') {
        let p = PathBuf::from(configured);
        if p.is_file() {
            return Ok((p, None));
        }
        return Err(DependencyError::new(
            "NPLAY is missing",
            format!("The configured NPLAY executable does not exist: {configured}"),
            "Install NPLAY 1.7.0 or newer, or set the NPLAY binary path in Settings.",
        ));
    }
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(configured);
        if candidate.is_file() {
            return Ok((candidate, None));
        }
    }
    if let Some(dev) = std::env::var_os("NPLAY_DEV_PATH") {
        let root = PathBuf::from(dev);
        if root.join("nplay/app.py").is_file() {
            return Ok((PathBuf::from("python3"), Some(root)));
        }
    }
    Err(DependencyError::new(
        "NPLAY is not installed",
        "NIRUAMP is a graphical frontend; NPLAY is its required playback engine and was not found in PATH.",
        "Install NPLAY 1.7.0 or newer (github.com/nirucon/nplay) and run `nplay --doctor`. \
         For a development checkout, set NPLAY_DEV_PATH to the NPLAY repository.",
    ))
}

fn log_tail(path: &Path) -> String {
    let text = fs::read_to_string(path).unwrap_or_default();
    let lines: Vec<&str> = text.lines().rev().take(18).collect();
    lines.into_iter().rev().collect::<Vec<_>>().join("\n")
}

fn spawn_backend(prefs: &Preferences, socket: &Path) -> Result<BackendHandle, DependencyError> {
    let (program, dev_root) = find_nplay(prefs)?;
    if let Some(parent) = socket.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            DependencyError::new(
                "Cannot prepare NPLAY runtime directory",
                format!("{}: {e}", parent.display()),
                "Check XDG_RUNTIME_DIR permissions and run NIRUAMP again.",
            )
        })?;
        let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o700));
    }
    let log_path = state_dir().join("nplay-service.log");
    if let Some(parent) = log_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(&log_path)
        .ok();
    let mut cmd = Command::new(&program);
    if let Some(root) = dev_root {
        cmd.arg("-m").arg("nplay.app");
        cmd.current_dir(root);
        let old = std::env::var_os("PYTHONPATH").unwrap_or_default();
        let mut value = std::env::split_paths(&old).collect::<Vec<_>>();
        value.push(std::env::current_dir().unwrap_or_default());
        // The dev root is the repository; python must import nplay from it.
        if let Some(root) = cmd.get_current_dir() {
            value.push(root.to_path_buf());
        }
        if let Ok(joined) = std::env::join_paths(value) {
            cmd.env("PYTHONPATH", joined);
        }
    }
    cmd.arg("--serve")
        .arg("--service-socket")
        .arg(socket)
        .arg("--idle-exit")
        .arg(if prefs.keep_playing { "0" } else { "25" })
        .stdin(Stdio::null())
        .stdout(
            log.as_ref()
                .map(|f| Stdio::from(f.try_clone().unwrap()))
                .unwrap_or_else(Stdio::null),
        )
        .stderr(
            log.as_ref()
                .map(|f| Stdio::from(f.try_clone().unwrap()))
                .unwrap_or_else(Stdio::null),
        );
    cmd.process_group(0);
    let mut child = cmd.spawn().map_err(|e| DependencyError {
        title: "NPLAY could not start".into(),
        message: format!("Failed to execute {}: {e}", program.display()),
        remedy: "Install NPLAY 1.7.0 or newer and verify `nplay --doctor` works in a terminal."
            .into(),
        log_tail: log_tail(&log_path),
    })?;

    // Bounded startup wait (at most 12 s, only when NIRUAMP itself launches
    // NPLAY): the child's exit status and the socket appearing are checked
    // every 100 ms. It ends as soon as the service answers; nothing keeps
    // polling afterwards.
    let deadline = Instant::now() + Duration::from_secs(12);
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait().ok().flatten() {
            let tail = log_tail(&log_path);
            return Err(DependencyError {
                title: "NPLAY service exited during startup".into(),
                message: format!("NPLAY {} with status {status}.", program.display()),
                remedy:
                    "NPLAY 1.7.0 or newer is required. Run `nplay --version` and `nplay --doctor`. \
                         Terminal NPLAY must keep working without NIRUAMP."
                        .into(),
                log_tail: tail,
            });
        }
        if UnixStream::connect(socket).is_ok() {
            match Client::connect(socket) {
                Ok((client, snapshot)) => {
                    return Ok(BackendHandle {
                        client,
                        snapshot,
                        owned: true,
                        socket: socket.to_path_buf(),
                    });
                }
                Err(e) => {
                    // The socket may be up while the handshake is still being
                    // written to the log; retry for a moment before failing.
                    if let ClientError::Backend(err) = &e {
                        if err.code != -32010 {
                            let _ = child.kill();
                            return Err(DependencyError {
                                title: "NPLAY service handshake failed".into(),
                                message: e.to_string(),
                                remedy: "Verify that NPLAY is version 1.7.0 or newer.".into(),
                                log_tail: log_tail(&log_path),
                            });
                        }
                    }
                }
            }
        }
        thread::sleep(Duration::from_millis(100));
    }
    let tail = log_tail(&log_path);
    let _ = child.kill();
    Err(DependencyError {
        title: "NPLAY service did not become ready".into(),
        message: format!(
            "No compatible control endpoint appeared at {} within 12 seconds.",
            socket.display()
        ),
        remedy: "Run `nplay --serve` in a terminal to see the error, then check `nplay --doctor`."
            .into(),
        log_tail: tail,
    })
}

pub fn ensure_backend(
    prefs: &Preferences,
    allow_spawn: bool,
) -> Result<BackendHandle, DependencyError> {
    let socket = backend_socket_path();
    if UnixStream::connect(&socket).is_ok() {
        match Client::connect(&socket) {
            Ok((client, snapshot)) => {
                return Ok(BackendHandle {
                    client,
                    snapshot,
                    owned: false,
                    socket,
                })
            }
            Err(ClientError::Backend(err)) if err.code == -32010 => {
                return Err(DependencyError::new(
                    "Incompatible NPLAY protocol",
                    format!("The running NPLAY service rejected protocol version {}: {err}", crate::protocol::PROTOCOL_VERSION),
                    "Upgrade NPLAY to 1.7.0 or newer, or stop the older service and relaunch NIRUAMP.",
                ));
            }
            Err(ClientError::Backend(err)) => {
                return Err(DependencyError::new(
                    "NPLAY rejected the connection",
                    err.to_string(),
                    "Check the NPLAY service log and restart NPLAY.",
                ));
            }
            Err(e) => {
                return Err(DependencyError::new(
                    "NPLAY endpoint is unresponsive",
                    format!("{}: {e}", socket.display()),
                    "Stop the stale NPLAY service or remove the stale socket after verifying no NPLAY process is running.",
                ));
            }
        }
    }
    if !allow_spawn {
        return Err(DependencyError::new(
            "NPLAY is not running",
            "No compatible local NPLAY service endpoint was found.",
            "Start NPLAY with `nplay --serve`, or let NIRUAMP launch it automatically.",
        ));
    }
    spawn_backend(prefs, &socket)
}

pub struct InstanceGuard {
    path: PathBuf,
    listener: Option<UnixListener>,
    /// Set on drop; the listener thread exits on the next accept.
    stop: Arc<AtomicBool>,
}

pub enum InstanceKind {
    Primary(InstanceGuard),
    Secondary,
}

fn short_socket_path() -> PathBuf {
    let candidate = instance_socket();
    if candidate.as_os_str().len() < 100 {
        return candidate;
    }
    PathBuf::from(format!("/tmp/niruamp-{}.sock", libc_uid()))
}

fn libc_uid() -> u32 {
    // std has no direct uid accessor; /proc is portable on Linux.
    fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("Uid:"))
                .and_then(|l| l.split_whitespace().nth(1).map(|v| v.to_string()))
        })
        .and_then(|v| v.parse().ok())
        .unwrap_or(1000)
}

impl InstanceGuard {
    pub fn claim() -> std::io::Result<InstanceKind> {
        let path = short_socket_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
            let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o700));
        }
        match UnixListener::bind(&path) {
            Ok(listener) => {
                let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
                Ok(InstanceKind::Primary(Self {
                    path,
                    listener: Some(listener),
                    stop: Arc::new(AtomicBool::new(false)),
                }))
            }
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                if let Ok(mut stream) = UnixStream::connect(&path) {
                    let _ = stream.write_all(b"activate\n");
                    return Ok(InstanceKind::Secondary);
                }
                let _ = fs::remove_file(&path);
                let listener = UnixListener::bind(&path)?;
                let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
                Ok(InstanceKind::Primary(Self {
                    path,
                    listener: Some(listener),
                    stop: Arc::new(AtomicBool::new(false)),
                }))
            }
            Err(e) => Err(e),
        }
    }

    /// Accept activation requests from later launches. The accept blocks:
    /// the thread sleeps in the kernel until a client connects, and drop
    /// unblocks it by connecting once with the stop flag set.
    pub fn start_listener(&mut self, tx: Sender<()>, ctx: egui::Context) {
        let Some(listener) = self.listener.take() else {
            return;
        };
        let stop = self.stop.clone();
        thread::Builder::new()
            .name("niruamp-instance".into())
            .spawn(move || {
                serve_instance(listener, &stop, || {
                    let _ = tx.send(());
                    ctx.request_repaint();
                })
            })
            .ok();
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if self.listener.is_none() {
            // Wake the blocking accept so the thread sees the stop flag.
            let _ = UnixStream::connect(&self.path);
        }
        let _ = fs::remove_file(&self.path);
    }
}

fn serve_instance(listener: UnixListener, stop: &AtomicBool, mut activate: impl FnMut()) {
    for stream in listener.incoming() {
        if stop.load(Ordering::Acquire) {
            return;
        }
        let Ok(stream) = stream else {
            continue;
        };
        // A client that connects but never writes must not wedge the listener.
        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        let _ = reader.read_line(&mut line);
        if line.trim() == "activate" {
            activate();
        }
    }
}

pub fn spawn_backend_for_tests(_prefs: &Preferences) {}

#[allow(dead_code)]
pub fn wake_arc(ctx: egui::Context) -> Arc<dyn Fn() + Send + Sync> {
    Arc::new(move || ctx.request_repaint())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn instance_listener_blocks_and_stops_on_drop() {
        let dir = std::env::temp_dir().join(format!("niruamp-inst-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("instance.sock");
        let _ = fs::remove_file(&path);
        let listener = UnixListener::bind(&path).unwrap();
        let mut guard = InstanceGuard {
            path: path.clone(),
            listener: Some(listener),
            stop: Arc::new(AtomicBool::new(false)),
        };
        let (tx, rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let listener = guard.listener.take().unwrap();
        let stop = guard.stop.clone();
        let handle = thread::spawn(move || {
            serve_instance(listener, &stop, || tx.send(()).unwrap());
            done_tx.send(()).unwrap();
        });
        UnixStream::connect(&path)
            .unwrap()
            .write_all(b"activate\n")
            .unwrap();
        rx.recv_timeout(Duration::from_secs(3)).expect("activation");
        // A silent client does not count as an activation.
        drop(UnixStream::connect(&path).unwrap());
        assert!(rx.recv_timeout(Duration::from_millis(200)).is_err());
        drop(guard);
        done_rx
            .recv_timeout(Duration::from_secs(3))
            .expect("listener thread exits on drop");
        handle.join().unwrap();
        assert!(!path.exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
