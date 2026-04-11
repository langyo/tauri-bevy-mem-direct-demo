use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcEnvelope {
    pub id: u64,
    pub payload: IpcPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum IpcPayload {
    ToRenderer(ToRenderer),
    ToDesktop(ToDesktop),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ToRenderer {
    MoveCamera { direction: MoveDirection },
    PickRay { screen_x: f32, screen_y: f32 },
    SetResolution { width: u32, height: u32 },
    Ping { timestamp: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ToDesktop {
    PickResult {
        request_id: u64,
        cube_id: Option<String>,
        screen_x: f32,
        screen_y: f32,
    },
    Pong {
        timestamp: u64,
    },
    FrameDimensions {
        width: u32,
        height: u32,
    },
    Log {
        level: LogLevel,
        message: String,
    },
    Error {
        request_id: u64,
        code: i64,
        message: String,
    },
    FpsReport {
        fps: f64,
        frame_count: u64,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MoveDirection {
    Forward,
    Backward,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
    Debug,
}

impl IpcEnvelope {
    pub fn to_renderer(payload: ToRenderer) -> Self {
        Self {
            id: 0,
            payload: IpcPayload::ToRenderer(payload),
        }
    }

    pub fn to_renderer_with_id(id: u64, payload: ToRenderer) -> Self {
        Self {
            id,
            payload: IpcPayload::ToRenderer(payload),
        }
    }

    pub fn to_desktop(payload: ToDesktop) -> Self {
        Self {
            id: 0,
            payload: IpcPayload::ToDesktop(payload),
        }
    }

    pub fn to_desktop_reply(request_id: u64, payload: ToDesktop) -> Self {
        Self {
            id: request_id,
            payload: IpcPayload::ToDesktop(payload),
        }
    }

    pub fn encode_line(&self) -> String {
        serde_json::to_string(self).expect("failed to serialize IpcEnvelope")
    }

    pub fn decode_line(line: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(line)
    }
}
