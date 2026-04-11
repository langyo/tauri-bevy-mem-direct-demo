mod sidecar;

use server::data::MockDataSource;
use shared::protocol::ToRenderer;
use shared::shm::{ShmHandle, SHM_MAX_SIZE, SHM_NAME};
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::{Window, WindowId};
use wry::dpi::{LogicalPosition, LogicalSize};
use wry::{Rect, WebViewBuilder};
use serde::Deserialize;

#[derive(Deserialize)]
struct IpcMessage {
    resize: Option<ResizeMsg>,
}

#[derive(Deserialize)]
struct ResizeMsg {
    width: u32,
    height: u32,
    #[serde(default = "default_dpr")]
    dpr: f64,
}

fn default_dpr() -> f64 {
    1.0
}

struct RenderState {
    css_size: (u32, u32),
    dpr: f64,
    override_resolution: Option<(u32, u32)>,
}

impl RenderState {
    fn render_resolution(&self) -> (u32, u32) {
        if let Some((w, h)) = self.override_resolution {
            return (w.max(1), h.max(1));
        }
        let w = (self.css_size.0 as f64 * self.dpr).round() as u32;
        let h = (self.css_size.1 as f64 * self.dpr).round() as u32;
        (w.max(1), h.max(1))
    }
}

struct App {
    _window: Option<Window>,
    _webview: Option<wry::WebView>,
    sidecar: Option<sidecar::SidecarHandle>,
    sidecar_cmd_tx: Option<flume::Sender<ToRenderer>>,
    render_state: Arc<std::sync::Mutex<RenderState>>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self._window.is_some() {
            return;
        }

        let window = event_loop
            .create_window(
                winit::window::WindowAttributes::default()
                    .with_title("Demo")
                    .with_inner_size(winit::dpi::LogicalSize::new(1280u32, 800u32))
                    .with_resizable(true)
                    .with_transparent(true),
            )
            .expect("Failed to create window");

        if let Some(ref tx) = self.sidecar_cmd_tx {
            let size = window.inner_size();
            let scale = window.scale_factor();
            let css_w = (size.width as f64 / scale).round() as u32;
            let css_h = (size.height as f64 / scale).round() as u32;
            let _ = tx.send(ToRenderer::SetResolution {
                width: css_w,
                height: css_h,
            });
        }

        let init_script = r#"
(function() {
    function reportCanvasSize() {
        var c = document.getElementById('bevy-canvas');
        if (!c || !window.ipc) return;
        var w = Math.round(c.clientWidth);
        var h = Math.round(c.clientHeight);
        if (w > 0 && h > 0) {
            var dpr = window.devicePixelRatio || 1;
            window.ipc.postMessage(JSON.stringify({resize:{width:w,height:h,dpr:dpr}}));
        }
    }

    function observeCanvas() {
        var c = document.getElementById('bevy-canvas');
        if (!c) { setTimeout(observeCanvas, 100); return; }
        var ro = new ResizeObserver(function() { reportCanvasSize(); });
        ro.observe(c);
        reportCanvasSize();
    }

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', observeCanvas);
    } else {
        observeCanvas();
    }
})();
"#;

        let ipc_sidecar_tx = self.sidecar_cmd_tx.clone();
        let ipc_render_state = self.render_state.clone();

        let webview = WebViewBuilder::new()
            .with_url("http://127.0.0.1:18742/index.html")
            .with_devtools(true)
            .with_transparent(true)
            .with_initialization_script(init_script)
            .with_ipc_handler(move |req| {
                if let Ok(msg) = serde_json::from_str::<IpcMessage>(req.body()) {
                    if let Some(resize) = msg.resize {
                        {
                            let mut st = ipc_render_state.lock().unwrap();
                            st.css_size = (resize.width, resize.height);
                            st.dpr = resize.dpr;
                        }
                        let (rw, rh) = ipc_render_state.lock().unwrap().render_resolution();
                        if let Some(ref tx) = ipc_sidecar_tx {
                            let _ = tx.send(ToRenderer::SetResolution { width: rw, height: rh });
                        }
                    }
                }
            })
            .build(&window)
            .expect("Failed to build webview");
        event_loop.set_control_flow(ControlFlow::Wait);

        self._window = Some(window);
        self._webview = Some(webview);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::Wait);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match &event {
            WindowEvent::Resized(size) => {
                if let (Some(ref win), Some(ref wv)) = (&self._window, &self._webview)
                {
                    let scale = win.scale_factor();
                    let logical = size.to_logical::<u32>(scale);
                    let _ = wv.set_bounds(Rect {
                        position: LogicalPosition::new(0, 0).into(),
                        size: LogicalSize::new(logical.width, logical.height).into(),
                    });
                }
            }
            WindowEvent::CloseRequested => {
                if let Some(handle) = self.sidecar.take() {
                    std::thread::spawn(move || {
                        let rt =
                            tokio::runtime::Runtime::new().expect("Failed to create temp runtime");
                        let _ = rt.block_on(handle.kill());
                    });
                }
                event_loop.exit();
            }
            _ => {}
        }
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

#[cfg(target_os = "windows")]
mod win_job {
    use std::ffi::c_void;

    #[repr(C)]
    struct JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
        basic_limit_information: JOBOBJECT_BASIC_LIMIT_INFORMATION,
        io_info: JOBOBJECT_IO_RATE_CONTROL_INFORMATION,
        process_memory_limit: usize,
        job_memory_limit: usize,
        peak_process_memory_used: usize,
        peak_job_memory_used: usize,
    }

    #[repr(C)]
    struct JOBOBJECT_BASIC_LIMIT_INFORMATION {
        per_process_user_time_limit: i64,
        per_job_user_time_limit: i64,
        limit_flags: u32,
        minimum_working_set_size: usize,
        maximum_working_set_size: usize,
        active_process_limit: u32,
        affinity: usize,
        priority_class: u32,
        scheduling_class: u32,
    }

    #[repr(C)]
    #[allow(dead_code)]
    struct JOBOBJECT_IO_RATE_CONTROL_INFORMATION {
        max_iops: i64,
        max_bandwidth: i64,
        reservation_iops: i64,
        reservation_bandwidth: i64,
        volume: *const u16,
        base_io_size: u32,
        control_flags: u32,
    }

    extern "system" {
        fn CreateJobObjectW(lpJobAttributes: *const c_void, lpName: *const u16) -> *mut c_void;
        fn SetInformationJobObject(
            hJob: *const c_void,
            JobObjectInformationClass: u32,
            lpJobObjectInfo: *const c_void,
            cbJobObjectInfoLength: u32,
        ) -> i32;
        fn AssignProcessToJobObject(hJob: *const c_void, hProcess: *const c_void) -> i32;
        fn GetCurrentProcess() -> *mut c_void;
    }

    const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x00002000;
    const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION: u32 = 9;

    pub fn create_job_object() -> *mut c_void {
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return std::ptr::null_mut();
            }

            let mut info = std::mem::zeroed::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>();
            info.basic_limit_information.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

            let result = SetInformationJobObject(
                job,
                JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
                &info as *const _ as *const c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );

            if result == 0 {
                return std::ptr::null_mut();
            }

            let process = GetCurrentProcess();
            AssignProcessToJobObject(job, process);

            job
        }
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    #[cfg(target_os = "windows")]
    {
        let job = win_job::create_job_object();
        if job.is_null() {
            tracing::warn!("Failed to create job object, sidecar may not be killed on exit");
        }
    }

    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

    let data_source = Arc::new(MockDataSource::new()) as Arc<dyn server::data::DataSource>;
    let port: u16 = 18742;
    let dist_dir = find_dist_dir();

    let (sidecar_cmd_tx, sidecar_cmd_rx) = flume::bounded(32);
    let (log_tx, log_rx) = flume::bounded(256);
    let (ipc_tx, ipc_rx) = flume::bounded(64);
    let (resolution_tx, resolution_rx) = flume::bounded(8);
    let sidecar_cmd_tx_for_app = sidecar_cmd_tx.clone();
    let sidecar_cmd_tx_for_listener = sidecar_cmd_tx.clone();

    let sidecar_handle = rt
        .block_on(sidecar::start_sidecar(sidecar_cmd_rx, log_tx, ipc_tx))
        .expect("Failed to start Bevy sidecar");

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
        Arc::new(std::sync::Mutex::new(
            handle.expect("Failed to open shared memory after waiting 10s for renderer"),
        ))
    };

    rt.block_on(async {
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .expect("Failed to bind axum server");
        tracing::info!("Axum server listening on {}", addr);
        let shm_for_router = shm.clone();
        tokio::spawn(async move {
            axum::serve(
                listener,
                server::create_router(
                    data_source,
                    sidecar_cmd_tx,
                    log_rx,
                    ipc_rx,
                    shm_for_router,
                    resolution_tx,
                    dist_dir,
                ),
            )
            .await
            .expect("Axum server error");
        });
    });

    let render_state = Arc::new(std::sync::Mutex::new(RenderState {
        css_size: (1280, 800),
        dpr: 1.0,
        override_resolution: None,
    }));
    let render_state_for_listener = render_state.clone();

    std::thread::Builder::new()
        .name("resolution-listener".into())
        .spawn(move || {
            while let Ok((w, h)) = resolution_rx.recv() {
                tracing::info!(w, h, "resolution-listener received");
                {
                    let mut st = render_state_for_listener.lock().unwrap();
                    st.override_resolution = if w == 0 && h == 0 { None } else { Some((w, h)) };
                }
                let (rw, rh) = render_state_for_listener.lock().unwrap().render_resolution();
                tracing::info!(rw, rh, "resolution-listener sending SetResolution");
                let _ = sidecar_cmd_tx_for_listener.send(ToRenderer::SetResolution { width: rw, height: rh });
            }
        })
        .expect("Failed to spawn resolution listener thread");

    let event_loop = winit::event_loop::EventLoop::new().expect("Failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    event_loop
        .run_app(&mut App {
            _window: None,
            _webview: None,
            sidecar: Some(sidecar_handle),
            sidecar_cmd_tx: Some(sidecar_cmd_tx_for_app),
            render_state,
        })
        .expect("Event loop error");
}
