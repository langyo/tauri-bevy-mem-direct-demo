use crate::ipc::IpcChannel;
use bevy::prelude::*;
use bevy::render::camera::RenderTarget;
use shared::protocol::MoveDirection;

const MOVE_SPEED: f32 = 5.0;
const BOUND_X: (f32, f32) = (-10.0, 10.0);
const BOUND_Z: (f32, f32) = (-10.0, 10.0);
const CAMERA_HEIGHT: f32 = 15.0;

#[derive(Component)]
pub struct MainCamera;

pub fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Camera {
            target: RenderTarget::Window(Default::default()),
            ..default()
        },
        Transform::from_xyz(0.0, CAMERA_HEIGHT, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
        MainCamera,
    ));
}

pub fn camera_movement_system(
    time: Res<Time>,
    mut ipc: ResMut<IpcChannel>,
    mut query: Query<&mut Transform, With<MainCamera>>,
) {
    let Ok(mut transform) = query.get_single_mut() else {
        return;
    };

    let moves = ipc.drain_moves();
    for direction in moves {
        let delta = time.delta_secs() * MOVE_SPEED;
        let forward = transform.forward();
        let right = transform.right();

        match direction {
            MoveDirection::Forward => {
                transform.translation.x -= forward.x * delta;
                transform.translation.z -= forward.z * delta;
            }
            MoveDirection::Backward => {
                transform.translation.x += forward.x * delta;
                transform.translation.z += forward.z * delta;
            }
            MoveDirection::Left => {
                transform.translation.x -= right.x * delta;
                transform.translation.z -= right.z * delta;
            }
            MoveDirection::Right => {
                transform.translation.x += right.x * delta;
                transform.translation.z += right.z * delta;
            }
        }
    }

    transform.translation.x = transform.translation.x.clamp(BOUND_X.0, BOUND_X.1);
    transform.translation.z = transform.translation.z.clamp(BOUND_Z.0, BOUND_Z.1);
    transform.translation.y = CAMERA_HEIGHT;
}
