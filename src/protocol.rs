//! JSON-RPC 2.0 client machinery for NPLAY's versioned Unix-socket protocol.
use crate::model::Snapshot;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub const PROTOCOL_VERSION: u32 = 1;
pub const CLIENT_NAME: &str = "niruamp";
pub const CLIENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const MAX_RESPONSE_LINE: usize = 32 * 1024 * 1024;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct RpcError {
    #[serde(default)]
    pub code: i64,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub data: Value,
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)?;
        if let Some(detail) = self.data.get("detail").and_then(Value::as_str) {
            if !detail.is_empty() {
                write!(f, " ({detail})")?;
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum ClientError {
    Io(String),
    Protocol(String),
    Backend(RpcError),
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::Io(s) => write!(f, "{s}"),
            ClientError::Protocol(s) => write!(f, "{s}"),
            ClientError::Backend(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ClientError {}

#[derive(Debug, Clone)]
pub enum Incoming {
    Response {
        id: u64,
        result: Result<Value, RpcError>,
    },
    Notification {
        method: String,
        params: Value,
    },
    /// A pushed visualization frame (protocol 1.1 `visualization.subscribe`):
    /// one 0–255 value per bar. Not a JSON-RPC message.
    Vis(Vec<u8>),
    Disconnected(String),
}

/// Classify one line from NPLAY. Visualization frames (`{"vis":…}`) are routed
/// before JSON-RPC handling; anything unparseable is ignored.
pub fn parse_line(text: &str) -> Option<Incoming> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let value: Value = serde_json::from_str(text).ok()?;
    if let Some(vis) = value.get("vis") {
        return decode_vis(vis, value.get("enc").and_then(Value::as_str)).map(Incoming::Vis);
    }
    if let Some(method) = value.get("method").and_then(Value::as_str) {
        return Some(Incoming::Notification {
            method: method.to_string(),
            params: value.get("params").cloned().unwrap_or(Value::Null),
        });
    }
    let id = value.get("id").and_then(Value::as_u64)?;
    let result = if let Some(err) = value.get("error") {
        Err(
            serde_json::from_value::<RpcError>(err.clone()).unwrap_or_else(|_| RpcError {
                code: -1,
                message: "backend returned an invalid error".into(),
                data: Value::Null,
            }),
        )
    } else {
        Ok(value.get("result").cloned().unwrap_or(Value::Null))
    };
    Some(Incoming::Response { id, result })
}

fn decode_vis(vis: &Value, enc: Option<&str>) -> Option<Vec<u8>> {
    const MAX_BARS: usize = 256;
    match vis {
        Value::String(text) if enc == Some("b64") || enc.is_none() => {
            use base64::Engine;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(text.as_bytes())
                .ok()?;
            (bytes.len() <= MAX_BARS).then_some(bytes)
        }
        Value::Array(items) if items.len() <= MAX_BARS => items
            .iter()
            .map(|v| v.as_u64().map(|n| n.min(255) as u8))
            .collect(),
        _ => None,
    }
}

type Wake = Arc<dyn Fn() + Send + Sync>;

pub struct Client {
    tx: Sender<Vec<u8>>,
    rx: Receiver<Incoming>,
    next_id: AtomicU64,
    path: PathBuf,
    wake: Arc<Mutex<Option<Wake>>>,
    /// Set by the reader when it has woken the UI; cleared by the UI before it
    /// drains. Any number of messages between two frames cost one wake.
    wake_pending: Arc<AtomicBool>,
    /// Id of the one outstanding quiet request (0 = none).
    quiet_id: Arc<AtomicU64>,
    closed: Arc<AtomicBool>,
}

impl Client {
    pub fn connect(path: &Path) -> Result<(Self, Snapshot), ClientError> {
        let mut stream = UnixStream::connect(path)
            .map_err(|e| ClientError::Io(format!("cannot connect to {}: {e}", path.display())))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(15)))
            .map_err(|e| ClientError::Io(e.to_string()))?;
        stream
            .set_write_timeout(Some(Duration::from_secs(15)))
            .map_err(|e| ClientError::Io(e.to_string()))?;
        let hello = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "client": {"name": CLIENT_NAME, "version": CLIENT_VERSION},
                "protocol_versions": [PROTOCOL_VERSION]
            }
        });
        let mut line =
            serde_json::to_vec(&hello).map_err(|e| ClientError::Protocol(e.to_string()))?;
        line.push(b'\n');
        stream
            .write_all(&line)
            .map_err(|e| ClientError::Io(format!("initialize write failed: {e}")))?;
        let mut reader = BufReader::new(stream);
        let mut buf = String::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        let snapshot: Snapshot = loop {
            if std::time::Instant::now() > deadline {
                return Err(ClientError::Protocol("backend handshake timed out".into()));
            }
            buf.clear();
            reader
                .read_line(&mut buf)
                .map_err(|e| ClientError::Io(format!("initialize read failed: {e}")))?;
            if buf.is_empty() {
                return Err(ClientError::Protocol(
                    "backend closed during handshake".into(),
                ));
            }
            if buf.len() > MAX_RESPONSE_LINE {
                return Err(ClientError::Protocol(
                    "backend sent an oversized frame".into(),
                ));
            }
            let value: Value = serde_json::from_str(buf.trim())
                .map_err(|e| ClientError::Protocol(format!("invalid backend JSON: {e}")))?;
            if value.get("id").and_then(Value::as_u64) != Some(1) {
                continue;
            }
            if let Some(err) = value.get("error") {
                let err: RpcError =
                    serde_json::from_value(err.clone()).unwrap_or_else(|_| RpcError {
                        code: -1,
                        message: "backend rejected handshake".into(),
                        data: Value::Null,
                    });
                return Err(ClientError::Backend(err));
            }
            let result = value
                .get("result")
                .cloned()
                .ok_or_else(|| ClientError::Protocol("handshake response has no result".into()))?;
            let snap: Snapshot = serde_json::from_value(result)
                .map_err(|e| ClientError::Protocol(format!("handshake snapshot mismatch: {e}")))?;
            if snap.protocol_version != PROTOCOL_VERSION {
                return Err(ClientError::Protocol(format!(
                    "backend protocol {} is not supported (need {})",
                    snap.protocol_version, PROTOCOL_VERSION
                )));
            }
            break snap;
        };

        let read_stream = reader.into_inner();
        read_stream
            .set_read_timeout(None)
            .map_err(|e| ClientError::Io(e.to_string()))?;
        let write_stream = read_stream
            .try_clone()
            .map_err(|e| ClientError::Io(e.to_string()))?;
        let (tx, job_rx) = mpsc::channel::<Vec<u8>>();
        let (event_tx, rx) = mpsc::channel::<Incoming>();
        let wake: Arc<Mutex<Option<Wake>>> = Arc::new(Mutex::new(None));
        let closed = Arc::new(AtomicBool::new(false));
        let wake_pending = Arc::new(AtomicBool::new(false));
        let quiet_id = Arc::new(AtomicU64::new(0));

        {
            let event_tx = event_tx.clone();
            let closed = closed.clone();
            let wake = wake.clone();
            let pending = wake_pending.clone();
            let quiet = quiet_id.clone();
            thread::Builder::new()
                .name("niruamp-rpc-read".into())
                .spawn(move || read_loop(read_stream, event_tx, closed, wake, pending, quiet))
                .map_err(|e| ClientError::Io(e.to_string()))?;
        }
        {
            let event_tx = event_tx.clone();
            let closed = closed.clone();
            let wake = wake.clone();
            thread::Builder::new()
                .name("niruamp-rpc-write".into())
                .spawn(move || write_loop(write_stream, job_rx, event_tx, closed, wake))
                .map_err(|e| ClientError::Io(e.to_string()))?;
        }
        Ok((
            Self {
                tx,
                rx,
                next_id: AtomicU64::new(2),
                path: path.to_path_buf(),
                wake,
                wake_pending,
                quiet_id,
                closed,
            },
            snapshot,
        ))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn set_wake(&self, wake: Wake) {
        if let Ok(mut slot) = self.wake.lock() {
            *slot = Some(wake);
        }
    }

    /// Called by the UI before it drains messages: the next message wakes it again.
    pub fn rearm_wake(&self) {
        self.wake_pending.store(false, Ordering::Release);
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Relaxed)
    }

    pub fn send(&self, method: &str, params: Value) -> Result<u64, ClientError> {
        self.send_with(method, params, false)
    }

    /// Send a request whose response must not wake the UI: the 1.0
    /// `visualization.data` poll. The frame that sends the next poll picks
    /// the answer up, so each poll period costs one frame instead of two.
    pub fn send_quiet(&self, method: &str, params: Value) -> Result<u64, ClientError> {
        self.send_with(method, params, true)
    }

    fn send_with(&self, method: &str, params: Value, quiet: bool) -> Result<u64, ClientError> {
        if self.is_closed() {
            return Err(ClientError::Io("backend connection is closed".into()));
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        if quiet {
            self.quiet_id.store(id, Ordering::Release);
        }
        let value = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let mut line =
            serde_json::to_vec(&value).map_err(|e| ClientError::Protocol(e.to_string()))?;
        line.push(b'\n');
        self.tx
            .send(line)
            .map_err(|_| ClientError::Io("backend writer stopped".into()))?;
        Ok(id)
    }

    pub fn try_recv(&self) -> Option<Incoming> {
        match self.rx.try_recv() {
            Ok(msg) => Some(msg),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Incoming::Disconnected(
                "backend event channel closed".into(),
            )),
        }
    }

    pub fn recv_timeout(&self, timeout: Duration) -> Option<Incoming> {
        self.rx.recv_timeout(timeout).ok()
    }
}

fn wake_shared(wake: &Arc<Mutex<Option<Wake>>>) {
    if let Ok(slot) = wake.lock() {
        if let Some(wake) = slot.as_ref() {
            wake();
        }
    }
}

fn wake_coalesced(wake: &Arc<Mutex<Option<Wake>>>, pending: &AtomicBool) {
    if !pending.swap(true, Ordering::AcqRel) {
        wake_shared(wake);
    }
}

fn read_loop(
    stream: UnixStream,
    tx: Sender<Incoming>,
    closed: Arc<AtomicBool>,
    wake: Arc<Mutex<Option<Wake>>>,
    pending: Arc<AtomicBool>,
    quiet: Arc<AtomicU64>,
) {
    let mut reader = BufReader::new(stream);
    let mut buf = String::new();
    loop {
        buf.clear();
        match reader.read_line(&mut buf) {
            Ok(0) => {
                closed.store(true, Ordering::Relaxed);
                let _ = tx.send(Incoming::Disconnected(
                    "NPLAY service closed the connection".into(),
                ));
                wake_shared(&wake);
                return;
            }
            Ok(_) => {
                if buf.len() > MAX_RESPONSE_LINE {
                    closed.store(true, Ordering::Relaxed);
                    let _ = tx.send(Incoming::Disconnected(
                        "NPLAY service sent an oversized frame".into(),
                    ));
                    wake_shared(&wake);
                    return;
                }
                if let Some(msg) = parse_line(&buf) {
                    let silent = matches!(&msg, Incoming::Response { id, .. }
                        if *id == quiet.load(Ordering::Acquire));
                    let _ = tx.send(msg);
                    if !silent {
                        wake_coalesced(&wake, &pending);
                    }
                }
            }
            Err(e) => {
                closed.store(true, Ordering::Relaxed);
                let _ = tx.send(Incoming::Disconnected(format!(
                    "NPLAY service read failed: {e}"
                )));
                wake_shared(&wake);
                return;
            }
        }
    }
}

fn write_loop(
    mut stream: UnixStream,
    jobs: Receiver<Vec<u8>>,
    tx: Sender<Incoming>,
    closed: Arc<AtomicBool>,
    wake: Arc<Mutex<Option<Wake>>>,
) {
    while let Ok(frame) = jobs.recv() {
        if let Err(e) = stream.write_all(&frame).and_then(|_| stream.flush()) {
            if !closed.swap(true, Ordering::Relaxed) {
                let _ = tx.send(Incoming::Disconnected(format!(
                    "NPLAY service write failed: {e}"
                )));
                wake_shared(&wake);
            }
            return;
        }
    }
    closed.store(true, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_vis_frames_are_routed_before_json_rpc() {
        match parse_line("{\"vis\":[0,12,255,300]}\n") {
            Some(Incoming::Vis(bars)) => assert_eq!(bars, vec![0, 12, 255, 255]),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn b64_vis_frames_decode() {
        match parse_line("{\"vis\":\"AAz/\",\"enc\":\"b64\"}") {
            Some(Incoming::Vis(bars)) => assert_eq!(bars, vec![0, 12, 255]),
            other => panic!("unexpected {other:?}"),
        }
        // Key order is not guaranteed by JSON.
        assert!(matches!(
            parse_line("{\"enc\":\"b64\",\"vis\":\"AAA=\"}"),
            Some(Incoming::Vis(_))
        ));
        assert!(parse_line("{\"vis\":\"!!\",\"enc\":\"b64\"}").is_none());
    }

    #[test]
    fn responses_and_notifications_still_parse() {
        assert!(matches!(
            parse_line("{\"jsonrpc\":\"2.0\",\"id\":7,\"result\":{\"subscribed\":true}}"),
            Some(Incoming::Response {
                id: 7,
                result: Ok(_)
            })
        ));
        assert!(matches!(
            parse_line(
                "{\"jsonrpc\":\"2.0\",\"id\":8,\"error\":{\"code\":-32601,\"message\":\"no\"}}"
            ),
            Some(Incoming::Response {
                id: 8,
                result: Err(_)
            })
        ));
        assert!(matches!(
            parse_line("{\"jsonrpc\":\"2.0\",\"method\":\"state.changed\",\"params\":{}}"),
            Some(Incoming::Notification { .. })
        ));
        assert!(parse_line("not json").is_none());
        assert!(parse_line("   ").is_none());
    }
}
