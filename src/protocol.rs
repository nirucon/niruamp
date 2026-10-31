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
    Disconnected(String),
}

type Wake = Arc<dyn Fn() + Send + Sync>;

pub struct Client {
    tx: Sender<Vec<u8>>,
    rx: Receiver<Incoming>,
    next_id: AtomicU64,
    path: PathBuf,
    wake: Arc<Mutex<Option<Wake>>>,
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

        {
            let event_tx = event_tx.clone();
            let closed = closed.clone();
            let wake = wake.clone();
            thread::Builder::new()
                .name("niruamp-rpc-read".into())
                .spawn(move || read_loop(read_stream, event_tx, closed, wake))
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

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Relaxed)
    }

    pub fn send(&self, method: &str, params: Value) -> Result<u64, ClientError> {
        if self.is_closed() {
            return Err(ClientError::Io("backend connection is closed".into()));
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
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

fn read_loop(
    stream: UnixStream,
    tx: Sender<Incoming>,
    closed: Arc<AtomicBool>,
    wake: Arc<Mutex<Option<Wake>>>,
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
                let text = buf.trim();
                if text.is_empty() {
                    continue;
                }
                let value: Value = match serde_json::from_str(text) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                if let Some(method) = value.get("method").and_then(Value::as_str) {
                    let _ = tx.send(Incoming::Notification {
                        method: method.to_string(),
                        params: value.get("params").cloned().unwrap_or(Value::Null),
                    });
                    wake_shared(&wake);
                    continue;
                }
                let Some(id) = value.get("id").and_then(Value::as_u64) else {
                    continue;
                };
                let result = if let Some(err) = value.get("error") {
                    Err(
                        serde_json::from_value::<RpcError>(err.clone()).unwrap_or_else(|_| {
                            RpcError {
                                code: -1,
                                message: "backend returned an invalid error".into(),
                                data: Value::Null,
                            }
                        }),
                    )
                } else {
                    Ok(value.get("result").cloned().unwrap_or(Value::Null))
                };
                let _ = tx.send(Incoming::Response { id, result });
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
