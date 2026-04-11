use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CubeMetadata {
    pub id: String,
    pub label: String,
    pub position: [f32; 3],
    pub color: [f32; 4],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CubeSensorData {
    pub id: String,
    pub label: String,
    pub temperature: f64,
    pub humidity: f64,
    pub timestamp: i64,
    pub history: Vec<SensorHistoryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorHistoryEntry {
    pub timestamp: i64,
    pub temperature: f64,
    pub humidity: f64,
}

pub fn default_cubes() -> Vec<CubeMetadata> {
    vec![
        CubeMetadata {
            id: "cube_0".into(),
            label: "节点A".into(),
            position: [-3.0, 0.5, -3.0],
            color: hex_to_rgba(0x17, 0x59, 0xA8),
        },
        CubeMetadata {
            id: "cube_1".into(),
            label: "节点B".into(),
            position: [0.0, 0.5, -3.0],
            color: hex_to_rgba(0xFF, 0x46, 0x1F),
        },
        CubeMetadata {
            id: "cube_2".into(),
            label: "节点C".into(),
            position: [3.0, 0.5, -3.0],
            color: hex_to_rgba(0xFF, 0xB6, 0x1E),
        },
        CubeMetadata {
            id: "cube_3".into(),
            label: "节点D".into(),
            position: [-3.0, 0.5, 3.0],
            color: hex_to_rgba(0x0E, 0xB8, 0x3A),
        },
        CubeMetadata {
            id: "cube_4".into(),
            label: "节点E".into(),
            position: [0.0, 0.5, 3.0],
            color: hex_to_rgba(0xFF, 0xF1, 0x43),
        },
        CubeMetadata {
            id: "cube_5".into(),
            label: "节点F".into(),
            position: [3.0, 0.5, 3.0],
            color: hex_to_rgba(0x06, 0x52, 0x79),
        },
    ]
}

fn hex_to_rgba(r: u8, g: u8, b: u8) -> [f32; 4] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
}
