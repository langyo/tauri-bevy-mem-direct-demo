use axum::{
    extract::State,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use flume::Sender;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

use crate::cubes::AppState;

#[derive(Clone)]
pub struct SignalingBridge {
    offer_tx: Sender<(String, Sender<String>)>,
    ice_to_sidecar_tx: Sender<String>,
    ice_from_sidecar_buffer: Arc<std::sync::Mutex<Vec<String>>>,
}

pub struct SignalingBridgeHandles {
    pub offer_rx: flume::Receiver<(String, Sender<String>)>,
    pub ice_to_sidecar_rx: flume::Receiver<String>,
    pub ice_from_sidecar_buffer: Arc<std::sync::Mutex<Vec<String>>>,
}

impl SignalingBridge {
    pub fn new() -> (Self, SignalingBridgeHandles) {
        let (offer_tx, offer_rx) = flume::bounded(4);
        let (ice_to_sidecar_tx, ice_to_sidecar_rx) = flume::bounded(16);
        let ice_from_sidecar_buffer = Arc::new(std::sync::Mutex::new(Vec::new()));

        let bridge = SignalingBridge {
            offer_tx,
            ice_to_sidecar_tx,
            ice_from_sidecar_buffer: ice_from_sidecar_buffer.clone(),
        };

        let handles = SignalingBridgeHandles {
            offer_rx,
            ice_to_sidecar_rx,
            ice_from_sidecar_buffer,
        };

        (bridge, handles)
    }
}

#[derive(Debug, Deserialize)]
pub struct OfferRequest {
    pub sdp: String,
}

#[derive(Debug, Serialize)]
pub struct SdpResponse {
    pub sdp: String,
}

#[derive(Debug, Deserialize)]
pub struct IceRequest {
    pub candidate: String,
}

#[derive(Debug, Serialize)]
pub struct IceResponse {
    pub candidates: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/offer", post(handle_offer))
        .route("/answer", post(handle_answer))
        .route("/ice", post(handle_ice).get(handle_ice_get))
}

async fn handle_offer(
    State(state): State<AppState>,
    Json(req): Json<OfferRequest>,
) -> Response {
    let (reply_tx, reply_rx) = flume::bounded(1);

    if state.signaling.offer_tx.send((req.sdp, reply_tx)).is_err() {
        return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "Signaling channel closed".into(),
            }),
        )
            .into_response();
    }

    match tokio::time::timeout(Duration::from_secs(30), reply_rx.recv_async()).await {
        Ok(Ok(answer_sdp)) => (
            axum::http::StatusCode::OK,
            Json(SdpResponse { sdp: answer_sdp }),
        )
            .into_response(),
        Ok(Err(_)) => (
            axum::http::StatusCode::GATEWAY_TIMEOUT,
            Json(ErrorResponse {
                error: "Sidecar dropped answer channel".into(),
            }),
        )
            .into_response(),
        Err(_) => (
            axum::http::StatusCode::GATEWAY_TIMEOUT,
            Json(ErrorResponse {
                error: "Timed out waiting for WebRTC answer from sidecar".into(),
            }),
        )
            .into_response(),
    }
}

async fn handle_answer() -> Response {
    (
        axum::http::StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: "WebRTC answers are returned in the offer response".into(),
        }),
    )
        .into_response()
}

async fn handle_ice(
    State(state): State<AppState>,
    Json(req): Json<IceRequest>,
) -> Response {
    let _ = state.signaling.ice_to_sidecar_tx.send(req.candidate);

    let mut guard = state.signaling.ice_from_sidecar_buffer.lock().unwrap();
    let candidates = std::mem::take(&mut *guard);

    (axum::http::StatusCode::OK, Json(IceResponse { candidates })).into_response()
}

async fn handle_ice_get(State(state): State<AppState>) -> Response {
    let mut guard = state.signaling.ice_from_sidecar_buffer.lock().unwrap();
    let candidates = std::mem::take(&mut *guard);

    (axum::http::StatusCode::OK, Json(IceResponse { candidates })).into_response()
}
