use async_trait::async_trait;
use rand::Rng;
use shared::cube::{CubeSensorData, SensorHistoryEntry};
use std::collections::HashMap;
use std::sync::Mutex;

#[async_trait]
pub trait DataSource: Send + Sync + 'static {
    async fn read_sensor(&self, cube_id: &str) -> Option<CubeSensorData>;
    async fn read_all(&self) -> Vec<CubeSensorData>;
}

struct CubeState {
    label: String,
    base_temp: f64,
    base_humidity: f64,
    history: Vec<SensorHistoryEntry>,
}

pub struct MockDataSource {
    cubes: Mutex<HashMap<String, CubeState>>,
}

impl Default for MockDataSource {
    fn default() -> Self {
        Self::new()
    }
}

impl MockDataSource {
    pub fn new() -> Self {
        let mut cubes = HashMap::new();
        let defs = [
            ("cube_0", "节点A", 25.0, 57.5),
            ("cube_1", "节点B", 30.0, 50.0),
            ("cube_2", "节点C", 23.0, 65.0),
            ("cube_3", "节点D", 27.0, 50.0),
            ("cube_4", "节点E", 21.0, 70.0),
            ("cube_5", "节点F", 29.0, 42.5),
        ];
        for (id, label, base_temp, base_humidity) in defs {
            cubes.insert(
                id.to_string(),
                CubeState {
                    label: label.to_string(),
                    base_temp,
                    base_humidity,
                    history: Vec::new(),
                },
            );
        }
        Self {
            cubes: Mutex::new(cubes),
        }
    }
}

#[async_trait]
impl DataSource for MockDataSource {
    async fn read_sensor(&self, cube_id: &str) -> Option<CubeSensorData> {
        let mut cubes = self.cubes.lock().unwrap();
        let state = cubes.get_mut(cube_id)?;
        let mut rng = rand::rng();
        let now = chrono::Utc::now().timestamp();
        let time_offset = (now % 60) as f64 * 0.05;
        let temperature = state.base_temp + time_offset + rng.random_range(-1.5..1.5);
        let humidity = state.base_humidity + time_offset * 0.8 + rng.random_range(-3.0..3.0);
        let entry = SensorHistoryEntry {
            timestamp: now,
            temperature,
            humidity,
        };
        state.history.push(entry);
        if state.history.len() > 30 {
            state.history.remove(0);
        }
        Some(CubeSensorData {
            id: cube_id.to_string(),
            label: state.label.clone(),
            temperature,
            humidity,
            timestamp: now,
            history: state.history.clone(),
        })
    }

    async fn read_all(&self) -> Vec<CubeSensorData> {
        let ids = ["cube_0", "cube_1", "cube_2", "cube_3", "cube_4", "cube_5"];
        let mut results = Vec::new();
        for id in ids {
            if let Some(data) = self.read_sensor(id).await {
                results.push(data);
            }
        }
        results
    }
}
