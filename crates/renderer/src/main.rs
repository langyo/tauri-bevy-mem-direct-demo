use bevy::app::ScheduleRunnerPlugin;
use bevy::log::LogPlugin;
use bevy::prelude::*;
use std::time::Duration;

mod camera;
mod frame_stream;
mod ipc;
mod picking;
mod scene;

fn apply_resolution(
    mut ipc: ResMut<ipc::IpcChannel>,
    mut res: ResMut<frame_stream::CaptureResolution>,
    mut dirty: ResMut<frame_stream::ResolutionDirty>,
) {
    if let Some((request_id, width, height)) = ipc.drain_resolution() {
        res.width = width;
        res.height = height;
        dirty.0 = true;
        eprintln!(
            "[renderer] resolution set to {}x{} (id={})",
            width, height, request_id
        );
    }
}

fn main() {
    eprintln!("[renderer] starting (headless, no window)");
    run_bevy();
    eprintln!("[renderer] bevy loop ended");
}

fn run_bevy() {
    let window_plugin = WindowPlugin {
        primary_window: None,
        exit_condition: bevy::window::ExitCondition::DontExit,
        close_when_requested: false,
    };

    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgba(0.0, 0.0, 0.0, 0.0)))
        .insert_resource(ipc::IpcChannel::new())
        .insert_resource(frame_stream::ResolutionDirty::default())
        .insert_resource(frame_stream::CaptureResolution::new(1280, 800))
        .insert_resource(frame_stream::FpsTracker::default())
        .add_plugins(
            DefaultPlugins
                .build()
                .disable::<bevy::winit::WinitPlugin>()
                .disable::<LogPlugin>()
                .set(window_plugin),
        )
        .add_plugins(ScheduleRunnerPlugin::run_loop(Duration::ZERO))
        .add_systems(
            Startup,
            (
                scene::spawn_cubes,
                scene::spawn_ground,
                scene::spawn_lighting,
                camera::spawn_camera,
            ),
        )
        .add_systems(PostStartup, frame_stream::setup_capture_image)
        .add_systems(
            Update,
            (
                ipc::ipc_read_system,
                apply_resolution,
                frame_stream::rebuild_capture_image,
                scene::orbit_system,
                camera::camera_movement_system,
                picking::pick_system,
            )
                .chain(),
        )
        .add_systems(Update, frame_stream::fps_report_system)
        .add_observer(frame_stream::on_readback_complete);

    app.run();
}
