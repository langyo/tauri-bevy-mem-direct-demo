use crate::data::DataSource;
use crate::signaling::SignalingBridge;
use axum::{extract::State, response::Json};
use flume::Sender;
use shared::cube::CubeMetadata;
use shared::protocol::ToDesktop;
use shared::protocol::ToRenderer;
use shared::shm::ShmHandle;
use std::sync::Arc;

pub type SidecarCommand = ToRenderer;
pub type ShmRef = Arc<std::sync::Mutex<ShmHandle>>;

#[derive(Clone)]
pub struct AppState {
    pub data_source: Arc<dyn DataSource>,
    pub cubes: Vec<CubeMetadata>,
    pub signaling: SignalingBridge,
    pub sidecar_tx: Sender<SidecarCommand>,
    pub log_rx: flume::Receiver<String>,
    pub ipc_rx: flume::Receiver<ToDesktop>,
    pub shm: ShmRef,
    pub resolution_tx: flume::Sender<(u32, u32)>,
}

pub async fn cubes_handler(State(state): State<AppState>) -> Json<Vec<CubeMetadata>> {
    Json(state.cubes)
}
