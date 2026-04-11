use std::collections::HashMap;
use std::rc::Rc;

use serde::{Deserialize, Serialize};
use tairitsu_hooks::{use_effect, use_ref, use_signal};
use tairitsu_vdom::Platform;
use tairitsu_web::WitPlatform;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SensorUpdateParams {
    cube_id: String,
    temperature: f64,
    humidity: f64,
    timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SensorSnapshotEntry {
    id: String,
    label: String,
    temperature: f64,
    humidity: f64,
    timestamp: i64,
    #[serde(default)]
    history: Vec<SensorHistoryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorHistoryEntry {
    pub timestamp: i64,
    pub temperature: f64,
    pub humidity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SensorSnapshotParams {
    cubes: Vec<SensorSnapshotEntry>,
}

#[derive(Debug, Clone)]
pub struct CubeSensorData {
    pub id: String,
    pub label: String,
    pub temperature: f64,
    pub humidity: f64,
    pub timestamp: i64,
    pub history: Vec<SensorHistoryEntry>,
}

impl Default for CubeSensorData {
    fn default() -> Self {
        Self {
            id: String::new(),
            label: String::new(),
            temperature: 0.0,
            humidity: 0.0,
            timestamp: 0,
            history: Vec::new(),
        }
    }
}

impl From<SensorSnapshotEntry> for CubeSensorData {
    fn from(e: SensorSnapshotEntry) -> Self {
        CubeSensorData {
            id: e.id,
            label: e.label,
            temperature: e.temperature,
            humidity: e.humidity,
            timestamp: e.timestamp,
            history: e
                .history
                .into_iter()
                .map(|h| SensorHistoryEntry {
                    timestamp: h.timestamp,
                    temperature: h.temperature,
                    humidity: h.humidity,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RendererFpsParams {
    fps: f64,
    frame_count: u64,
}

pub struct UseWsHandle {
    pub data: tairitsu_hooks::ReactiveSignal<HashMap<String, CubeSensorData>>,
    pub connected: tairitsu_hooks::ReactiveSignal<bool>,
    pub renderer_fps: tairitsu_hooks::ReactiveSignal<f64>,
    pub send_pick: Rc<dyn Fn(f32, f32)>,
    pub send_move: Rc<dyn Fn(String)>,
    pub send_render_resolution: Rc<dyn Fn(u32, u32)>,
}

pub fn use_ws(url: &str) -> UseWsHandle {
    let data = use_signal(HashMap::<String, CubeSensorData>::new);
    let connected = use_signal(|| false);
    let renderer_fps = use_signal(|| 0.0f64);
    let ws_handle = use_ref(0u64);
    let next_id = use_signal(|| 0u64);

    let url_owned = url.to_string();
    let data_clone = data.clone();
    let connected_clone = connected.clone();
    let fps_clone = renderer_fps.clone();

    let ws_for_effect = ws_handle.clone();

    use_effect(move || {
        let url = url_owned.clone();
        let data = data_clone.clone();
        let connected = connected_clone.clone();
        let renderer_fps = fps_clone.clone();

        let on_open: Option<Box<dyn FnOnce(u64)>> = {
            let connected = connected.clone();
            Some(Box::new(move |_handle: u64| {
                connected.set(true);
            }))
        };

        let on_message: Option<Box<dyn FnMut(u64, String)>> = {
            let data = data.clone();
            let renderer_fps = renderer_fps.clone();
            Some(Box::new(move |_handle: u64, text: String| {
                let v: serde_json::Value = match serde_json::from_str(&text) {
                    Ok(v) => v,
                    Err(_) => return,
                };
                let method = v.get("method").and_then(|m| m.as_str()).unwrap_or("");
                let params = v.get("params");

                match method {
                    "sensor.snapshot" => {
                        if let Some(params) = params {
                            if let Ok(snapshot) =
                                serde_json::from_value::<SensorSnapshotParams>(params.clone())
                            {
                                let mut map = data.write();
                                for entry in snapshot.cubes {
                                    let id = entry.id.clone();
                                    map.insert(id, CubeSensorData::from(entry));
                                }
                            }
                        }
                    }
                    "sensor.update" => {
                        if let Some(params) = params {
                            if let Ok(update) =
                                serde_json::from_value::<SensorUpdateParams>(params.clone())
                            {
                                let mut map = data.write();
                                let entry = map.entry(update.cube_id.clone()).or_default();
                                entry.temperature = update.temperature;
                                entry.humidity = update.humidity;
                                entry.timestamp = update.timestamp;
                                entry.history.push(SensorHistoryEntry {
                                    timestamp: update.timestamp,
                                    temperature: update.temperature,
                                    humidity: update.humidity,
                                });
                                if entry.history.len() > 60 {
                                    entry.history.remove(0);
                                }
                            }
                        }
                    }
                    "renderer.log" => {
                        if let Some(params) = params {
                            if let Some(line) = params.get("line").and_then(|l| l.as_str()) {
                                WitPlatform::console_log(line);
                            }
                        }
                    }
                    "renderer.fps" => {
                        if let Some(params) = params {
                            if let Ok(fps_data) =
                                serde_json::from_value::<RendererFpsParams>(params.clone())
                            {
                                renderer_fps.set(fps_data.fps);
                                if let Ok(platform) = WitPlatform::new() {
                                    if let Some(el) =
                                        platform.get_element_by_id("renderer-fps-display")
                                    {
                                        let text = if fps_data.fps > 0.0 {
                                            format!("Bevy: {:.0}", fps_data.fps)
                                        } else {
                                            "Bevy: --".into()
                                        };
                                        platform.set_inner_html(&el, text);
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }))
        };

        let on_error: Option<Box<dyn FnOnce(u64)>> = {
            let connected = connected.clone();
            Some(Box::new(move |_handle: u64| {
                connected.set(false);
            }))
        };

        let on_close: Option<Box<dyn FnOnce(u64, bool, u16, String)>> = {
            let connected = connected.clone();
            Some(Box::new(
                move |_handle: u64, _was_clean: bool, _code: u16, _reason: String| {
                    connected.set(false);
                },
            ))
        };

        if let Ok(handle) = WitPlatform::ws_new(&url, on_open, on_message, on_error, on_close) {
            *ws_for_effect.current_mut() = handle;
        }
    });

    let next_id_pick = next_id.clone();
    let ws_for_pick = ws_handle.clone();

    let send_pick: Rc<dyn Fn(f32, f32)> = Rc::new(move |screen_x: f32, screen_y: f32| {
        let id_val = {
            let mut n = next_id_pick.write();
            *n += 1;
            *n
        };
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "input.pick",
            "params": { "screen_x": screen_x, "screen_y": screen_y },
            "id": id_val
        });
        let msg_str = serde_json::to_string(&msg).unwrap_or_default();
        let handle = *ws_for_pick.current();
        if handle != 0 {
            let _ = WitPlatform::ws_send(handle, &msg_str);
        }
    });

    let next_id_move = next_id.clone();
    let ws_for_move = ws_handle.clone();

    let send_move: Rc<dyn Fn(String)> = Rc::new(move |direction: String| {
        let id_val = {
            let mut n = next_id_move.write();
            *n += 1;
            *n
        };
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "input.move",
            "params": { "direction": direction },
            "id": id_val
        });
        let msg_str = serde_json::to_string(&msg).unwrap_or_default();
        let handle = *ws_for_move.current();
        if handle != 0 {
            let _ = WitPlatform::ws_send(handle, &msg_str);
        }
    });

    let next_id_res = next_id.clone();
    let ws_for_res = ws_handle.clone();

    let send_render_resolution: Rc<dyn Fn(u32, u32)> = Rc::new(move |width: u32, height: u32| {
        WitPlatform::console_log(&format!(
            "send_render_resolution called: {}x{}",
            width, height
        ));
        let id_val = {
            let mut n = next_id_res.write();
            *n += 1;
            *n
        };
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "display.renderResolution",
            "params": { "width": width, "height": height },
            "id": id_val
        });
        let msg_str = serde_json::to_string(&msg).unwrap_or_default();
        WitPlatform::console_log(&format!("sending WS msg: {}", msg_str));
        let handle = *ws_for_res.current();
        if handle != 0 {
            let _ = WitPlatform::ws_send(handle, &msg_str);
        }
    });

    UseWsHandle {
        data,
        connected,
        renderer_fps,
        send_pick,
        send_move,
        send_render_resolution,
    }
}
