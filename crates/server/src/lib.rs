pub mod cubes;
pub mod data;
pub mod signaling;
pub mod ws;

use crate::cubes::AppState;
use crate::data::DataSource;
use crate::signaling::SignalingBridge;
use axum::extract::Request;
use axum::http::{HeaderName, HeaderValue};
use axum::middleware::{self, Next};
use axum::routing::get;
use axum::Router;
use flume::Sender;
use std::sync::Arc;
use tower_http::services::ServeDir;

use shared::protocol::ToDesktop;

pub type LogSender = flume::Sender<String>;
pub type LogReceiver = flume::Receiver<String>;
pub type IpcSender = flume::Sender<ToDesktop>;
pub type IpcReceiver = flume::Receiver<ToDesktop>;

async fn coop_coep_headers(request: Request, next: Next) -> axum::response::Response {
    let mut resp = next.run(request).await;
    let headers = resp.headers_mut();
    headers.insert(
        HeaderName::from_static("cross-origin-opener-policy"),
        HeaderValue::from_static("same-origin"),
    );
    headers.insert(
        HeaderName::from_static("cross-origin-embedder-policy"),
        HeaderValue::from_static("require-corp"),
    );
    resp
}

pub fn create_router(
    data_source: Arc<dyn DataSource>,
    sidecar_tx: Sender<cubes::SidecarCommand>,
    log_rx: LogReceiver,
    ipc_rx: IpcReceiver,
    shm: cubes::ShmRef,
    multiplier_tx: flume::Sender<(u32, u32)>,
    dist_dir: std::path::PathBuf,
) -> Router {
    let (bridge, _handles) = SignalingBridge::new();
    let state = AppState {
        data_source,
        cubes: shared::cube::default_cubes(),
        signaling: bridge,
        sidecar_tx,
        log_rx,
        ipc_rx,
        shm,
        resolution_tx: multiplier_tx,
    };

    Router::new()
        .route("/ws", get(ws::ws_handler))
        .route("/frame.ws", get(ws::frame_ws_handler))
        .route("/api/cubes", get(cubes::cubes_handler))
        .nest("/signaling", signaling::router())
        .fallback_service(ServeDir::new(dist_dir))
        .layer(middleware::from_fn(coop_coep_headers))
        .with_state(state)
}
