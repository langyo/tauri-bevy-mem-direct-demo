# Bevy Host + wry Overlay Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Merge Bevy 3D renderer and wry 2D HTML panel into a single process — Bevy owns the window, wry renders HTML as a transparent child overlay.

**Architecture:** Bevy 0.15 is the main app (single window, single process). A Bevy plugin (`WryOverlayPlugin`) accesses the `winit::Window` via `NonSend<WinitWindows>` and creates a wry `WebView` with `build_as_child()` + `with_transparent(true)`. The HTML panel (tairitsu/hikari WASM) loads from axum `ServeDir`. Sensor data, renderer.log, and input commands flow through a single WebSocket (`/ws`). The entire sidecar process, stdout frame pipe, `FrameBuffer`, and `/frame.ws` are removed.

**Tech Stack:** Bevy 0.15, wry 0.49 (with `transparent` feature), winit 0.30, axum 0.8, flume

---

## Key API Reference

### Bevy 0.15 — Getting winit::Window

```rust
use bevy::prelude::*;
use bevy::winit::WinitWindows;

fn my_system(
    primary: Query<Entity, With<PrimaryWindow>>,
    winit_windows: NonSend<WinitWindows>,
) {
    let entity = primary.single();
    let Some(wrapper) = winit_windows.get_window(entity) else { return };
    let winit: &winit::window::Window = &**wrapper;
    // Use winit for wry::WebViewBuilder::build_as_child(winit)
}
```

**Important:** Windows are created in `PreUpdate`, so use `Update` schedule for first-time wry creation. Use `Option<WebView>` resource as guard to create only once.

### wry 0.49 — Transparent Child WebView

```rust
use wry::{WebViewBuilder, Rect};

let webview = WebViewBuilder::new()
    .with_url("http://127.0.0.1:18742/index.html")
    .with_transparent(true)
    .with_bounds(Rect {
        position: (0, 0).into(),
        size: (1280, 800).into(),
    })
    .build_as_child(&winit_window)?;
```

**Important:** `build_as_child` does NOT auto-resize. Must call `webview.set_bounds()` on window resize events.

**Feature flag required:** `wry = { version = "0.49", features = ["transparent"] }`

---

## Crate Changes Overview

| Crate | Before | After |
|---|---|---|
| `renderer` | Standalone binary (sidecar process) | Becomes a **library** (`[lib]`) providing Bevy plugins |
| `desktop` | winit+wry window + sidecar spawner + axum | Bevy app + axum server (no sidecar) |
| `server` | `/frame.ws` + `FrameBuffer` dep | Remove `/frame.ws`, remove `FrameBuffer` param |
| `shared` | `frame.rs` (FrameBuffer) | Remove `frame.rs`, remove `mod frame` |
| `panel` | Unchanged | Unchanged |

---

## Task 1: Convert renderer from binary to library

**Files:**
- Modify: `crates/renderer/Cargo.toml`
- Modify: `crates/renderer/src/main.rs` → rename to `crates/renderer/src/lib.rs`
- Create: `crates/renderer/src/lib.rs` (plugin exports)
- Modify: `Cargo.toml` (workspace)

**Step 1: Convert Cargo.toml**

Change `crates/renderer/Cargo.toml`:
- Remove `[[bin]]` section (if any)
- Add `[lib]` if needed
- Keep same dependencies

**Step 2: Create lib.rs**

Create `crates/renderer/src/lib.rs` that re-exports everything:

```rust
mod camera;
mod frame_stream;
mod ipc;
mod picking;
mod scene;

pub use camera::*;
pub use scene::*;
```

**Step 3: Move main.rs logic to an app builder function**

Create `crates/renderer/src/app.rs`:

```rust
use bevy::prelude::*;

pub fn renderer_app() -> App {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.1, 0.1, 0.15)))
        .insert_resource(WinitSettings {
            focused_mode: UpdateMode::Continuous,
            unfocused_mode: UpdateMode::Continuous,
        })
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, (
            scene::spawn_cubes,
            scene::spawn_ground,
            scene::spawn_lighting,
            camera::spawn_camera,
        ))
        .add_systems(Update, (
            camera::camera_movement_system,
            picking::pick_system,
        ));
    app
}
```

Note: `frame_stream` systems (capture image, readback, send_frame) are REMOVED — no more sidecar frame pipe.

Note: `ipc::ipc_read_system` is REMOVED — no more stdin IPC from sidecar. Input will come via WebSocket through a Bevy resource/event instead.

**Step 4: Remove frame_stream.rs and ipc.rs**

Delete `crates/renderer/src/frame_stream.rs` and `crates/renderer/src/ipc.rs`.
Remove `mod frame_stream;` and `mod ipc;` from lib.rs.

**Step 5: Update camera_movement_system**

Remove dependency on `ipc::IpcChannel`. Camera movement will be driven by Bevy keyboard input resource or events from the wry overlay plugin.

For now, keep WASD keyboard input working directly via Bevy's `KeyCode` input:

```rust
pub fn camera_movement_system(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut query: Query<&mut Transform, With<MainCamera>>,
) {
    let speed = 5.0 * time.delta_secs();
    for mut transform in query.iter_mut() {
        if keyboard.pressed(KeyCode::KeyW) { transform.translation.z -= speed; }
        if keyboard.pressed(KeyCode::KeyS) { transform.translation.z += speed; }
        if keyboard.pressed(KeyCode::KeyA) { transform.translation.x -= speed; }
        if keyboard.pressed(KeyCode::KeyD) { transform.translation.x += speed; }
        transform.translation.y = 15.0;
        transform.translation.x = transform.translation.x.clamp(-10.0, 10.0);
        transform.translation.z = transform.translation.z.clamp(-10.0, 10.0);
    }
}
```

**Step 6: Update picking.rs**

Remove dependency on `ipc::IpcChannel`. Picking will be event-driven from the wry overlay. For now, stub it out — it will be wired up in a later task.

**Step 7: Verify it compiles**

Run: `cargo check -p renderer`
Expected: Compiles successfully (no main fn needed for lib)

**Step 8: Commit**

```
git add crates/renderer/
git commit -m "refactor: convert renderer from binary to library, remove frame_stream and ipc"
```

---

## Task 2: Create WryOverlayPlugin

**Files:**
- Create: `crates/renderer/src/wry_overlay.rs`
- Modify: `crates/renderer/src/lib.rs`
- Modify: `crates/renderer/Cargo.toml` (add wry dep with transparent feature)

**Step 1: Add wry dependency to renderer**

In `crates/renderer/Cargo.toml`:
```toml
wry = { workspace = true }
```

**Step 2: Create wry_overlay.rs**

```rust
use bevy::prelude::*;
use bevy::winit::WinitWindows;
use wry::{WebView, WebViewBuilder, Rect};

pub struct WryOverlayPlugin {
    pub url: String,
    pub port: u16,
}

impl Plugin for WryOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(WryWebViewHandle(None))
            .insert_resource(WryOverlayConfig {
                url: format!("http://127.0.0.1:{}/index.html", self.port),
            })
            .add_systems(Update, init_webview)
            .add_systems(Update, resize_webview);
    }
}

#[derive(Resource)]
pub struct WryWebViewHandle(pub Option<WebView>);

#[derive(Resource)]
struct WryOverlayConfig {
    url: String,
}

fn init_webview(
    primary: Query<Entity, With<PrimaryWindow>>,
    winit_windows: NonSend<WinitWindows>,
    mut handle: ResMut<WryWebViewHandle>,
    config: Res<WryOverlayConfig>,
    windows: Query<&bevy::window::Window>,
) {
    if handle.0.is_some() {
        return;
    }
    let entity = primary.single();
    let Some(wrapper) = winit_windows.get_window(entity) else {
        return;
    };
    let winit: &winit::window::Window = &**wrapper;

    let bevy_window = windows.get(entity).unwrap();
    let size = bevy_window.physical_size();

    let webview = WebViewBuilder::new()
        .with_url(&config.url)
        .expect("Invalid URL")
        .with_transparent(true)
        .with_devtools(true)
        .with_bounds(Rect {
            position: (0, 0).into(),
            size: (size.width, size.height).into(),
        })
        .build_as_child(winit)
        .expect("Failed to build wry webview");

    handle.0 = Some(webview);
    info!("Wry overlay webview created");
}

fn resize_webview(
    mut handle: ResMut<WryWebViewHandle>,
    primary: Query<Entity, With<PrimaryWindow>>,
    mut resize_events: EventReader<bevy::window::WindowResized>,
) {
    let Some(webview) = handle.0.as_ref() else {
        return;
    };
    for event in resize_events.read() {
        if primary.single() == event.window {
            let _ = webview.set_bounds(Rect {
                position: (0, 0).into(),
                size: (event.width, event.height).into(),
            });
        }
    }
}
```

**Step 3: Export from lib.rs**

Add to `crates/renderer/src/lib.rs`:
```rust
mod wry_overlay;
pub use wry_overlay::WryOverlayPlugin;
```

**Step 4: Verify compilation**

Run: `cargo check -p renderer`
Expected: Compiles successfully

**Step 5: Commit**

```
git add crates/renderer/
git commit -m "feat: add WryOverlayPlugin for transparent child webview on Bevy window"
```

---

## Task 3: Update workspace Cargo.toml — add wry transparent feature

**Files:**
- Modify: `Cargo.toml` (workspace root)

**Step 1: Update wry dependency**

Change:
```toml
wry = "0.49"
```
To:
```toml
wry = { version = "0.49", features = ["transparent"] }
```

**Step 2: Verify**

Run: `cargo check -p renderer`
Expected: Compiles

**Step 3: Commit**

```
git add Cargo.toml
git commit -m "chore: enable wry transparent feature in workspace"
```

---

## Task 4: Rewrite desktop crate — Bevy app + axum server, no sidecar

**Files:**
- Rewrite: `crates/desktop/src/main.rs`
- Delete: `crates/desktop/src/sidecar.rs`
- Modify: `crates/desktop/Cargo.toml`

**Step 1: Update Cargo.toml**

`crates/desktop/Cargo.toml`:
```toml
[package]
name = "demo-panel"
version = "0.2.0"
edition.workspace = true

[[bin]]
name = "demo-panel"
path = "src/main.rs"

[dependencies]
renderer = { path = "../renderer" }
server = { path = "../server" }
shared = { path = "../shared" }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
axum = { workspace = true }
bevy = { workspace = true }
```

Remove: `winit`, `wry`, `flume` deps (Bevy provides winit; renderer provides wry).

**Step 2: Rewrite main.rs**

```rust
use server::data::MockDataSource;
use std::sync::Arc;

fn find_dist_dir() -> std::path::PathBuf {
    for candidate in ["dist", "../dist"] {
        let p = std::path::PathBuf::from(candidate);
        if p.join("index.html").exists() {
            return p;
        }
    }
    panic!("dist/ directory not found. Run `just build` first.");
}

fn main() {
    tracing_subscriber::fmt::init();

    let port: u16 = 18742;
    let dist_dir = find_dist_dir();
    let data_source = Arc::new(MockDataSource::new()) as Arc<dyn server::data::DataSource>;
    let (sidecar_cmd_tx, _sidecar_cmd_rx) = flume::bounded(32);
    let (_log_tx, log_rx) = flume::bounded(256);

    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    rt.block_on(async {
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .expect("Failed to bind axum server");
        tracing::info!("Axum server listening on {}", addr);
        tokio::spawn(async move {
            axum::serve(
                listener,
                server::create_router_no_frame(data_source, sidecar_cmd_tx, log_rx, dist_dir),
            )
            .await
            .expect("Axum server error");
        });
    });

    let mut app = renderer::renderer_app();
    app.add_plugins(renderer::WryOverlayPlugin { port });
    app.run();
}
```

Note: `server::create_router_no_frame` is a new function without `FrameBuffer` param (see Task 5).

**Step 3: Delete sidecar.rs**

Delete `crates/desktop/src/sidecar.rs`.
Remove `mod sidecar;` from main.rs.

**Step 4: Commit**

```
git add crates/desktop/
git commit -m "refactor: rewrite desktop as Bevy app + axum, remove sidecar"
```

---

## Task 5: Simplify server — remove frame.ws and FrameBuffer

**Files:**
- Modify: `crates/server/src/lib.rs`
- Modify: `crates/server/src/ws.rs`
- Modify: `crates/server/Cargo.toml`

**Step 1: Remove FrameBuffer dependency from Cargo.toml**

Remove from `crates/server/Cargo.toml`:
```toml
shared = { path = "../shared" }
```
Add only what's needed (proto types):
```toml
shared = { path = "../shared" }
```
(Keep shared — still need proto types. But no more FrameBuffer import.)

**Step 2: Remove frame.ws handler from ws.rs**

Delete the entire `frame_ws_handler` function and `handle_frame_socket` function from `crates/server/src/ws.rs`.

**Step 3: Create new router function without FrameBuffer**

In `crates/server/src/lib.rs`, add:

```rust
pub fn create_router_no_frame(
    data_source: Arc<dyn DataSource>,
    sidecar_tx: Sender<cubes::SidecarCommand>,
    log_rx: LogReceiver,
    dist_dir: std::path::PathBuf,
) -> Router {
    let (bridge, _handles) = SignalingBridge::new();
    let state = AppState {
        data_source,
        cubes: shared::cube::default_cubes(),
        signaling: bridge,
        sidecar_tx,
        log_rx,
    };

    Router::new()
        .route("/ws", get(ws::ws_handler))
        .route("/api/cubes", get(cubes::cubes_handler))
        .nest("/signaling", signaling::router())
        .fallback_service(ServeDir::new(dist_dir))
        .with_state(state)
}
```

Keep the old `create_router` function for now (or remove it — YAGNI, remove it).

**Step 4: Remove FrameBuffer from AppState**

Remove `frame_buffer: Arc<shared::frame::FrameBuffer>` from `AppState` in `crates/server/src/cubes.rs`.

**Step 5: Commit**

```
git add crates/server/
git commit -m "refactor: remove frame.ws handler and FrameBuffer from server"
```

---

## Task 6: Clean up shared crate — remove FrameBuffer

**Files:**
- Modify: `crates/shared/src/lib.rs`
- Delete: `crates/shared/src/frame.rs`

**Step 1: Remove frame module**

In `crates/shared/src/lib.rs`, remove `pub mod frame;`.

**Step 2: Delete frame.rs**

Delete `crates/shared/src/frame.rs`.

**Step 3: Remove tokio dependency from shared (if no longer needed)**

Check if `tokio` is still used elsewhere in shared. If only `frame.rs` used `tokio::sync::Notify`, remove tokio from `crates/shared/Cargo.toml`.

**Step 4: Verify**

Run: `cargo check`
Expected: Compiles

**Step 5: Commit**

```
git add crates/shared/
git commit -m "refactor: remove FrameBuffer from shared crate"
```

---

## Task 7: Update desktop Cargo.toml — add flume back

The desktop crate needs `flume` for creating the channels passed to server. Check if it's already there.

If missing, add to `crates/desktop/Cargo.toml`:
```toml
flume = { workspace = true }
```

**Step 1: Verify compilation**

Run: `cargo check -p demo-panel`
Expected: Compiles

**Step 2: Commit (if changes needed)**

---

## Task 8: Update justfile for new build flow

**Files:**
- Modify: `justfile`

**Step 1: Remove sidecar build step**

Remove any `just` recipes that build the renderer as a sidecar binary and copy it.
The renderer is now a library, built as part of `cargo build`.

**Step 2: Simplify dev recipe**

```just
set shell := ["powershell", "-NoProfile", "-Command"]

_python := if os() == "windows" { "python" } else { "python3" }

build:
    {{ _python }} scripts/build_panel.py

dev: build
    cargo run --package demo-panel
```

**Step 3: Commit**

```
git add justfile
git commit -m "chore: simplify justfile, remove sidecar build steps"
```

---

## Task 9: End-to-end smoke test

**Step 1: Build panel WASM**

Run: `just build`
Expected: `dist/` populated with WASM and JS

**Step 2: Run the app**

Run: `just dev`
Expected:
- Bevy window opens with 3D scene (6 cubes, ground, lighting)
- wry transparent overlay on top
- HTML panel loads from `http://127.0.0.1:18742/index.html`
- Background transparent → 3D scene visible behind HTML elements
- WASD keys move camera
- Sensor data updates via WebSocket

**Step 3: Verify no sidecar process**

Check Task Manager — should only see one `demo-panel.exe` process (no separate renderer).

**Step 4: Commit any fixes**

```
git add -A
git commit -m "fix: address smoke test issues"
```

---

## Task 10: Clean up diagnostic logs

**Files:**
- Modify: various (remove `eprintln!`, `tracing::info!` diagnostic prints)

Remove all `[frame-js]`, `[frame-ws]`, `[frame]` diagnostic logging that was left from the MVP.

**Step 1: Commit**

```
git commit -m "chore: remove diagnostic logging from MVP"
```

---

## Post-Plan: Future Work (NOT in this plan)

- Wire up input.pick / input.move from wry overlay → Bevy via evaluate_script or events
- Wire up renderer.log from Bevy → wry overlay via evaluate_script
- Dynamic resolution: detect canvas size → resize Bevy camera
- WASM panel still uses `use_ws("ws://localhost:18742/ws")` — verify it still connects
