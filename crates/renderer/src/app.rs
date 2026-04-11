use bevy::prelude::*;
use bevy::winit::WinitSettings;

use crate::camera;
use crate::picking;
use crate::scene;

pub fn renderer_app() -> App {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.1, 0.1, 0.15)))
        .insert_resource(WinitSettings {
            focused_mode: bevy::winit::UpdateMode::Continuous,
            unfocused_mode: bevy::winit::UpdateMode::Continuous,
        })
        .add_plugins(DefaultPlugins)
        .add_systems(
            Startup,
            (
                scene::spawn_cubes,
                scene::spawn_ground,
                scene::spawn_lighting,
                camera::spawn_camera,
            ),
        )
        .add_systems(
            Update,
            (camera::camera_movement_system, picking::pick_system),
        );
    app
}
