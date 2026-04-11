use crate::ipc::IpcChannel;
use crate::scene::CubeId;
use bevy::prelude::*;
use shared::protocol::ToDesktop;

pub fn pick_system(
    mut ipc: ResMut<IpcChannel>,
    cameras: Query<(&Camera, &GlobalTransform)>,
    cubes: Query<(Entity, &CubeId, &Transform)>,
) {
    let picks = ipc.drain_picks();
    if picks.is_empty() {
        return;
    }

    let (camera, camera_transform) = match cameras.get_single() {
        Ok(pair) => pair,
        Err(_) => return,
    };

    for (request_id, screen_x, screen_y) in picks {
        let Ok(ray) = camera.viewport_to_world(camera_transform, Vec2::new(screen_x, screen_y))
        else {
            continue;
        };
        let ray_origin = ray.origin;
        let ray_dir = ray.direction;
        let mut closest: Option<(f32, String)> = None;

        for (_entity, cube_id, transform) in cubes.iter() {
            let half = 0.5;
            let center = transform.translation;
            let min = center - Vec3::splat(half);
            let max = center + Vec3::splat(half);

            if let Some(t) = ray_aabb_intersection(ray_origin, *ray_dir, min, max) {
                if closest.is_none() || t < closest.as_ref().unwrap().0 {
                    closest = Some((t, cube_id.0.clone()));
                }
            }
        }

        let hit_cube_id = closest.map(|(_, id)| id);
        ipc.send_reply(
            request_id,
            ToDesktop::PickResult {
                request_id,
                cube_id: hit_cube_id,
                screen_x,
                screen_y,
            },
        );
    }
}

fn ray_aabb_intersection(ray_origin: Vec3, ray_dir: Vec3, min: Vec3, max: Vec3) -> Option<f32> {
    let inv_dir = Vec3::new(1.0 / ray_dir.x, 1.0 / ray_dir.y, 1.0 / ray_dir.z);

    let t1 = (min - ray_origin) * inv_dir;
    let t2 = (max - ray_origin) * inv_dir;

    let t_min_vec = t1.min(t2);
    let t_max_vec = t1.max(t2);

    let t_min = t_min_vec.x.max(t_min_vec.y).max(t_min_vec.z);
    let t_max = t_max_vec.x.min(t_max_vec.y).min(t_max_vec.z);

    if t_max < 0.0 || t_min > t_max {
        return None;
    }

    Some(if t_min > 0.0 { t_min } else { t_max })
}
