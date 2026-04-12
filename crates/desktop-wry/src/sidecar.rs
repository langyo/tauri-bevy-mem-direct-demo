use flume::{Receiver, Sender};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

use shared::protocol::{IpcEnvelope, IpcPayload, ToDesktop, ToRenderer};

const IPC_PREFIX: &str = "__IPC__";

pub struct SidecarHandle {
    child: Arc<tokio::sync::Mutex<Option<Child>>>,
}

impl SidecarHandle {
    pub async fn kill(self) -> Result<(), String> {
        let mut guard = self.child.lock().await;
        if let Some(mut child) = guard.take() {
            child
                .kill()
                .await
                .map_err(|e| format!("Failed to kill sidecar: {}", e))?;
        }
        Ok(())
    }
}

pub async fn start_sidecar(
    cmd_rx: Receiver<ToRenderer>,
    log_tx: Sender<String>,
    ipc_tx: Sender<ToDesktop>,
) -> Result<SidecarHandle, Box<dyn std::error::Error + Send + Sync>> {
    let renderer_path = std::env::current_exe()?
        .parent()
        .ok_or("Cannot determine executable directory")?
        .join("binaries")
        .join(if cfg!(windows) {
            "renderer.exe"
        } else {
            "renderer"
        });

    tracing::info!("Resolved renderer path: {}", renderer_path.display());

    let mut child = Command::new(&renderer_path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let pid = child.id();
    tracing::info!("Bevy sidecar started, pid={:?}", pid);

    let mut stdin = child.stdin.take().expect("Failed to take sidecar stdin");
    let stderr = child.stderr.take().expect("Failed to take sidecar stderr");

    let child = Arc::new(tokio::sync::Mutex::new(Some(child)));
    let handle = SidecarHandle {
        child: child.clone(),
    };

    tokio::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let trimmed = line.trim().to_string();
            if let Some(ipc_json) = trimmed.strip_prefix(IPC_PREFIX) {
                match IpcEnvelope::decode_line(ipc_json) {
                    Ok(envelope) => match envelope.payload {
                        IpcPayload::ToDesktop(msg) => {
                            let _ = ipc_tx.send(msg);
                        }
                        IpcPayload::ToRenderer(_) => {}
                    },
                    Err(e) => {
                        tracing::warn!("Failed to parse IPC message: {} — {}", e, ipc_json);
                    }
                }
            } else {
                tracing::info!("[renderer] {}", trimmed);
                let _ = log_tx.send(trimmed);
            }
        }
    });

    let (stdin_tx, stdin_rx) = flume::bounded::<String>(256);

    tokio::spawn(async move {
        while let Ok(line) = stdin_rx.recv_async().await {
            if stdin.write_all(line.as_bytes()).await.is_err() {
                break;
            }
            if stdin.write_all(b"\n").await.is_err() {
                break;
            }
            if stdin.flush().await.is_err() {
                break;
            }
        }
    });

    tokio::spawn(async move {
        loop {
            match cmd_rx.recv_async().await {
                Ok(msg) => {
                    let envelope = IpcEnvelope::to_renderer(msg);
                    let line = envelope.encode_line();
                    if stdin_tx.send(line).is_err() {
                        break;
                    }
                }
                Err(_) => {
                    break;
                }
            }
        }

        let mut child_guard = child.lock().await;
        if let Some(mut c) = child_guard.take() {
            match c.wait().await {
                Ok(status) => tracing::info!("Bevy sidecar exited with status: {}", status),
                Err(e) => tracing::error!("Bevy sidecar wait error: {}", e),
            }
        }
    });

    Ok(handle)
}
