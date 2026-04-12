use server::data::MockDataSource;
use shared::event::{NamedEvent, FRAME_EVENT_NAME};
use shared::protocol::ToRenderer;
use shared::shm::{DualControl, FrameHeader, ShmHandle, SHM_MAX_SIZE, SHM_NAME};
use std::process::{Child, Command};
use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::sidecar;

pub struct RuntimeContext {
    platform: &'static str,
    url: String,
    rt: tokio::runtime::Runtime,
    server_task: Option<tokio::task::JoinHandle<()>>,
    sidecar_handle: Option<sidecar::SidecarHandle>,
}

impl RuntimeContext {
    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn shutdown(mut self) {
        let platform = self.platform;
        self.rt.block_on(async move {
            if let Some(server_task) = self.server_task.take() {
                server_task.abort();
            }

            if let Some(sidecar_handle) = self.sidecar_handle.take() {
                if let Err(error) = sidecar_handle.kill().await {
                    tracing::warn!(platform, %error, "failed to kill Bevy sidecar");
                }
            }
        });
    }
}

fn find_dist_dir() -> std::path::PathBuf {
    for candidate in ["dist", "../dist"] {
        let p = std::path::PathBuf::from(candidate);
        if p.join("index.html").exists() {
            return p;
        }
    }
    panic!("dist/ directory not found. Run `just build` first.");
}

fn spawn_shm_probe(shm: Arc<std::sync::Mutex<ShmHandle>>, label: &'static str) {
    std::thread::Builder::new()
        .name(format!("{label}-shm-probe"))
        .spawn(move || {
            let event = loop {
                match NamedEvent::open(FRAME_EVENT_NAME) {
                    Ok(ev) => break ev,
                    Err(_) => std::thread::sleep(std::time::Duration::from_millis(100)),
                }
            };
            let (shm_ptr, shm_size) = {
                let guard = shm.lock().unwrap();
                (guard.as_ptr() as usize, guard.size())
            };
            let shm_slice = unsafe { std::slice::from_raw_parts(shm_ptr as *const u8, shm_size) };
            let ctrl = DualControl::as_bytes(shm_slice);
            let mut last_seq: u64 = 0;
            let mut reported: u64 = 0;

            loop {
                if !event.wait(100) {
                    continue;
                }
                let ready_idx = ctrl.ready_index.load(Ordering::Acquire);
                let buf = ctrl.buffer_slice(shm_slice, ready_idx);
                let header = FrameHeader::from_buffer(buf);
                let seq = header.seq.load(Ordering::Acquire);
                let w = header.width.load(Ordering::Acquire);
                let h = header.height.load(Ordering::Acquire);
                let len = header.data_len.load(Ordering::Acquire);
                if seq == 0 || seq <= last_seq || w == 0 || h == 0 || len == 0 {
                    continue;
                }
                last_seq = seq;
                reported += 1;
                if reported == 1 || reported % 300 == 0 {
                    tracing::info!(
                        label,
                        seq,
                        w,
                        h,
                        len,
                        reported,
                        "cef runtime observed shm frame"
                    );
                }
            }
        })
        .expect("Failed to spawn shm probe thread");
}

fn maybe_launch_cef_host(platform: &'static str, port: u16) -> Option<Child> {
    let Some(base_cmd) = std::env::var("DEMO_CEF_HOST_CMD").ok() else {
        tracing::error!(
            platform,
            "DEMO_CEF_HOST_CMD is not set; demo-panel-cef requires a real CEF host executable"
        );
        return None;
    };

    let full_cmd = format!(
        "{} --url http://127.0.0.1:{} --shm-name {} --frame-event {}",
        base_cmd, port, SHM_NAME, FRAME_EVENT_NAME
    );

    let child = if cfg!(windows) {
        Command::new("powershell")
            .args(["-NoProfile", "-Command", &full_cmd])
            .spawn()
    } else {
        Command::new("sh").args(["-lc", &full_cmd]).spawn()
    };

    match child {
        Ok(c) => {
            tracing::info!(platform, cmd = %full_cmd, pid = ?c.id(), "started CEF host process");
            Some(c)
        }
        Err(e) => {
            tracing::error!(platform, cmd = %full_cmd, error = %e, "failed to start CEF host process");
            None
        }
    }
}

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();
}

pub fn start_runtime(platform: &'static str) -> Result<RuntimeContext, String> {
    init_tracing();

    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| format!("Failed to create tokio runtime: {e}"))?;

    let data_source = Arc::new(MockDataSource::new()) as Arc<dyn server::data::DataSource>;
    let dist_dir = find_dist_dir();
    let port: u16 = 18742;

    let (sidecar_cmd_tx, sidecar_cmd_rx) = flume::bounded(32);
    let (log_tx, log_rx) = flume::bounded(256);
    let (ipc_tx, ipc_rx) = flume::bounded(64);
    let (resolution_tx, resolution_rx) = flume::bounded(8);

    let sidecar_handle = rt
        .block_on(sidecar::start_sidecar(sidecar_cmd_rx, log_tx, ipc_tx))
        .map_err(|e| format!("Failed to start Bevy sidecar: {e}"))?;

    let shm: Arc<std::sync::Mutex<ShmHandle>> = {
        let mut handle = None;
        for _ in 0..100 {
            match ShmHandle::open(SHM_NAME, SHM_MAX_SIZE) {
                Ok(h) => {
                    handle = Some(h);
                    break;
                }
                Err(_) => std::thread::sleep(std::time::Duration::from_millis(100)),
            }
        }
        Arc::new(std::sync::Mutex::new(handle.ok_or_else(|| {
            "Failed to open shared memory after waiting 10s for renderer".to_string()
        })?))
    };

    spawn_shm_probe(shm.clone(), platform);

    let sidecar_cmd_tx_for_listener = sidecar_cmd_tx.clone();
    std::thread::Builder::new()
        .name(format!("{platform}-resolution-listener"))
        .spawn(move || {
            while let Ok((w, h)) = resolution_rx.recv() {
                let (rw, rh) = if w == 0 || h == 0 {
                    (1280, 800)
                } else {
                    (w, h)
                };
                tracing::info!(platform, w, h, rw, rh, "resolution-listener forwarding");
                let _ = sidecar_cmd_tx_for_listener.send(ToRenderer::SetResolution {
                    width: rw,
                    height: rh,
                });
            }
        })
        .expect("Failed to spawn resolution listener thread");

    let server_task = rt.block_on(async move {
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| format!("Failed to bind axum server: {e}"))?;
        tracing::info!(platform, %addr, "cef runtime axum listening");

        let server_task = tokio::spawn(async move {
            axum::serve(
                listener,
                server::create_router(
                    data_source,
                    sidecar_cmd_tx,
                    log_rx,
                    ipc_rx,
                    shm,
                    resolution_tx,
                    dist_dir,
                ),
            )
            .await
            .expect("Axum server error");
        });

        Ok::<_, String>(server_task)
    })?;

    Ok(RuntimeContext {
        platform,
        url: format!("http://127.0.0.1:{port}"),
        rt,
        server_task: Some(server_task),
        sidecar_handle: Some(sidecar_handle),
    })
}

pub fn run_external_host(platform: &'static str) {
    let runtime = match start_runtime(platform) {
        Ok(runtime) => runtime,
        Err(error) => panic!("{error}"),
    };

    let mut cef_child = maybe_launch_cef_host(platform, 18742);
    if cef_child.is_none() {
        runtime.shutdown();
        return;
    }
    tracing::info!(
        platform,
        "cef host bridge ready: set DEMO_CEF_HOST_CMD to wire real CEF host"
    );

    runtime.rt.block_on(async {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!(platform, "ctrl-c received, shutting down");
    });

    if let Some(mut child) = cef_child.take() {
        let _ = child.kill();
    }

    runtime.shutdown();
}
