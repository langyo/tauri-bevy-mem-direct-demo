use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::State,
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use shared::event::{NamedEvent, FRAME_EVENT_NAME};
use shared::proto::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, RendererFpsParams, SensorSnapshotParams};
use shared::protocol::{MoveDirection, ToDesktop, ToRenderer};
use shared::shm::{DualControl, FrameHeader, ShmHandle, FRAME_HEADER_SIZE};
use crate::cubes::{AppState, SidecarCommand};
use std::sync::Arc;
use std::sync::atomic::Ordering;

pub type FrameReceiver = flume::Receiver<Vec<u8>>;

struct ShmAddr(usize, usize);

pub fn spawn_frame_reader(shm: Arc<std::sync::Mutex<ShmHandle>>) -> FrameReceiver {
    let (tx, rx) = flume::bounded(2);

    let event = NamedEvent::open(FRAME_EVENT_NAME).expect("Failed to open frame event");

    let shm_addr = {
        let guard = shm.lock().unwrap();
        ShmAddr(guard.as_ptr() as usize, guard.size())
    };

    std::thread::Builder::new()
        .name("frame-reader".into())
        .spawn(move || {
            let shm_slice = unsafe { std::slice::from_raw_parts(shm_addr.0 as *const u8, shm_addr.1) };
            let ctrl = DualControl::as_bytes(shm_slice);
            let mut last_seq = 0u64;

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
                let data_len = header.data_len.load(Ordering::Acquire) as usize;

                if seq == 0 || w == 0 || h == 0 || data_len == 0 || seq <= last_seq {
                    continue;
                }
                last_seq = seq;

                let end = (FRAME_HEADER_SIZE + data_len).min(buf.len());
                let mut out = Vec::with_capacity(16 + data_len);
                out.extend_from_slice(&(seq as u32).to_le_bytes());
                out.extend_from_slice(&w.to_le_bytes());
                out.extend_from_slice(&h.to_le_bytes());
                out.extend_from_slice(&(data_len as u32).to_le_bytes());
                if end > FRAME_HEADER_SIZE {
                    out.extend_from_slice(&buf[FRAME_HEADER_SIZE..end]);
                }

                if tx.send(out).is_err() {
                    break;
                }
            }
        })
        .expect("Failed to spawn frame reader thread");

    rx
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

pub async fn frame_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_frame_socket(socket, state.shm))
}

async fn handle_frame_socket(mut socket: WebSocket, shm: Arc<std::sync::Mutex<ShmHandle>>) {
    tracing::info!("frame.ws client connected");
    let frame_rx = spawn_frame_reader(shm);
    let mut sent_count: u64 = 0;

    while let Ok(data) = frame_rx.recv_async().await {
        if socket.send(Message::Binary(data.into())).await.is_err() {
            tracing::info!(sent_count, "frame.ws client disconnected");
            return;
        }
        sent_count += 1;
        if sent_count == 1 || sent_count % 300 == 0 {
            tracing::info!(sent_count, "frame.ws sent frames");
        }
    }

    tracing::info!(sent_count, "frame.ws sender loop ended");
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let snapshot = state.data_source.read_all().await;
    let snapshot_params = SensorSnapshotParams { cubes: snapshot };
    let notification = JsonRpcNotification {
        jsonrpc: "2.0".into(),
        method: "sensor.snapshot".into(),
        params: serde_json::to_value(&snapshot_params).unwrap_or_default(),
    };
    let snapshot_json = serde_json::to_string(&notification).unwrap();

    let (mut sender, mut receiver) = socket.split();
    if sender.send(Message::Text(snapshot_json.into())).await.is_err() {
        return;
    }

    let mut tick = tokio::time::interval(std::time::Duration::from_secs(1));
    let data_source = state.data_source;
    let cube_ids: Vec<String> = state.cubes.iter().map(|c| c.id.clone()).collect();
    let sidecar_tx = state.sidecar_tx;
    let log_rx = state.log_rx;
    let ipc_rx = state.ipc_rx;
    let resolution_tx = state.resolution_tx;

    loop {
        tokio::select! {
            _ = tick.tick() => {
                for cube_id in &cube_ids {
                    if let Some(sensor_data) = data_source.read_sensor(cube_id).await {
                        let update_params = shared::proto::SensorUpdateParams {
                            cube_id: sensor_data.id,
                            temperature: sensor_data.temperature,
                            humidity: sensor_data.humidity,
                            timestamp: sensor_data.timestamp,
                        };
                        let notif = JsonRpcNotification {
                            jsonrpc: "2.0".into(),
                            method: "sensor.update".into(),
                            params: serde_json::to_value(&update_params).unwrap_or_default(),
                        };
                        let json = serde_json::to_string(&notif).unwrap();
                        if sender.send(Message::Text(json.into())).await.is_err() {
                            return;
                        }
                    }
                }
            }
            log_line = log_rx.recv_async() => {
                match log_line {
                    Ok(line) => {
                        let notif = JsonRpcNotification {
                            jsonrpc: "2.0".into(),
                            method: "renderer.log".into(),
                            params: serde_json::json!({"line": line}),
                        };
                        let json = serde_json::to_string(&notif).unwrap();
                        if sender.send(Message::Text(json.into())).await.is_err() {
                            return;
                        }
                    }
                    Err(_) => {}
                }
            }
            ipc_msg = ipc_rx.recv_async() => {
                match ipc_msg {
                    Ok(ToDesktop::FpsReport { fps, frame_count }) => {
                        let notif = JsonRpcNotification {
                            jsonrpc: "2.0".into(),
                            method: "renderer.fps".into(),
                            params: serde_json::to_value(&RendererFpsParams {
                                fps,
                                frame_count,
                            }).unwrap_or_default(),
                        };
                        let json = serde_json::to_string(&notif).unwrap();
                        if sender.send(Message::Text(json.into())).await.is_err() {
                            return;
                        }
                    }
                    Ok(_) => {}
                    Err(_) => {}
                }
            }
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(req) = serde_json::from_str::<JsonRpcRequest>(&text) {
                            handle_rpc_request(&mut sender, &sidecar_tx, &resolution_tx, req).await;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        return;
                    }
                    _ => {}
                }
            }
        }
    }
}

async fn handle_rpc_request(
    sender: &mut (impl SinkExt<Message, Error = axum::Error> + Unpin),
    sidecar_tx: &flume::Sender<SidecarCommand>,
    resolution_tx: &flume::Sender<(u32, u32)>,
    req: JsonRpcRequest,
) {
    let response = match req.method.as_str() {
        "input.pick" => {
            let params: shared::proto::PickParams = match serde_json::from_value(req.params) {
                Ok(p) => p,
                Err(e) => {
                    let err = JsonRpcResponse {
                        jsonrpc: "2.0".into(),
                        result: None,
                        error: Some(shared::proto::JsonRpcError {
                            code: -32602,
                            message: format!("Invalid params: {}", e),
                            data: None,
                        }),
                        id: req.id,
                    };
                    let json = serde_json::to_string(&err).unwrap();
                    let _ = sender.send(Message::Text(json.into())).await;
                    return;
                }
            };
            let cmd = ToRenderer::PickRay {
                screen_x: params.screen_x,
                screen_y: params.screen_y,
            };
            let _ = sidecar_tx.send(cmd);
            JsonRpcResponse {
                jsonrpc: "2.0".into(),
                result: Some(serde_json::json!({"status": "pending"})),
                error: None,
                id: req.id,
            }
        }
        "input.move" => {
            let params: shared::proto::MoveParams = match serde_json::from_value(req.params) {
                Ok(p) => p,
                Err(e) => {
                    let err = JsonRpcResponse {
                        jsonrpc: "2.0".into(),
                        result: None,
                        error: Some(shared::proto::JsonRpcError {
                            code: -32602,
                            message: format!("Invalid params: {}", e),
                            data: None,
                        }),
                        id: req.id,
                    };
                    let json = serde_json::to_string(&err).unwrap();
                    let _ = sender.send(Message::Text(json.into())).await;
                    return;
                }
            };
            let direction = match params.direction.as_str() {
                "forward" => MoveDirection::Forward,
                "backward" => MoveDirection::Backward,
                "left" => MoveDirection::Left,
                "right" => MoveDirection::Right,
                _ => {
                    let err = JsonRpcResponse {
                        jsonrpc: "2.0".into(),
                        result: None,
                        error: Some(shared::proto::JsonRpcError {
                            code: -32602,
                            message: format!("Invalid direction: {}", params.direction),
                            data: None,
                        }),
                        id: req.id,
                    };
                    let json = serde_json::to_string(&err).unwrap();
                    let _ = sender.send(Message::Text(json.into())).await;
                    return;
                }
            };
            let cmd = ToRenderer::MoveCamera { direction };
            let _ = sidecar_tx.send(cmd);
            JsonRpcResponse {
                jsonrpc: "2.0".into(),
                result: Some(serde_json::json!({"status": "ok"})),
                error: None,
                id: req.id,
            }
        }
        "display.renderResolution" => {
            let w = req.params.get("width").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let h = req.params.get("height").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            tracing::info!(w, h, "RPC display.renderResolution received");
            if w == 0 && h == 0 {
                let _ = resolution_tx.send((0, 0));
            } else if w > 0 && h > 0 && w <= 3840 && h <= 2160 {
                let _ = resolution_tx.send((w, h));
            }
            JsonRpcResponse {
                jsonrpc: "2.0".into(),
                result: Some(serde_json::json!({"status": "ok"})),
                error: None,
                id: req.id,
            }
        }
        _ => {
            JsonRpcResponse {
                jsonrpc: "2.0".into(),
                result: None,
                error: Some(shared::proto::JsonRpcError {
                    code: -32601,
                    message: "Method not found".into(),
                    data: None,
                }),
                id: req.id,
            }
        }
    };
    let json = serde_json::to_string(&response).unwrap();
    let _ = sender.send(Message::Text(json.into())).await;
}
