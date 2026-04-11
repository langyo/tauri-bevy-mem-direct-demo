use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: serde_json::Value,
    pub id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    pub id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorUpdateParams {
    pub cube_id: String,
    pub temperature: f64,
    pub humidity: f64,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorSnapshotParams {
    pub cubes: Vec<crate::cube::CubeSensorData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PickParams {
    pub screen_x: f32,
    pub screen_y: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PickResult {
    pub cube_id: Option<String>,
    pub screen_x: f32,
    pub screen_y: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveParams {
    pub direction: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RendererFpsParams {
    pub fps: f64,
    pub frame_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderResolutionParams {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayReadyParams {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayReadyResult {
    pub posted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MoveDirection {
    Forward,
    Backward,
    Left,
    Right,
}

impl MoveDirection {
    pub fn from_direction_str(s: &str) -> Option<Self> {
        match s {
            "forward" => Some(Self::Forward),
            "backward" => Some(Self::Backward),
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonRpcMessage {
    Request(JsonRpcRequest),
    Notification(JsonRpcNotification),
    Response(JsonRpcResponse),
}
