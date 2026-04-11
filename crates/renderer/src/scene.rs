use bevy::prelude::*;
use shared::cube::{default_cubes, CubeMetadata};

#[derive(Component)]
pub struct CubeId(pub String);

#[derive(Component)]
pub struct OrbitCube {
    pub angle_offset: f32,
    pub radius: f32,
    pub speed: f32,
    pub y: f32,
}

pub fn spawn_cubes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let cube_mesh = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let cubes = default_cubes();
    let count = cubes.len() as f32;
    let radius = 4.0;

    for (i, CubeMetadata { id, color, .. }) in cubes.into_iter().enumerate() {
        let angle = (i as f32 / count) * std::f32::consts::TAU;
        let x = radius * angle.cos();
        let z = radius * angle.sin();

        let emissive_color = LinearRgba::new(color[0], color[1], color[2], 1.0);
        let material = materials.add(StandardMaterial {
            base_color: Color::srgba(color[0], color[1], color[2], color[3]),
            emissive: emissive_color,
            ..default()
        });
        commands.spawn((
            Mesh3d(cube_mesh.clone()),
            MeshMaterial3d(material),
            Transform::from_xyz(x, 0.5, z),
            CubeId(id),
            OrbitCube {
                angle_offset: angle,
                radius,
                speed: 0.3,
                y: 0.5,
            },
        ));
    }
}

pub fn orbit_system(time: Res<Time>, mut query: Query<(&OrbitCube, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (orbit, mut transform) in query.iter_mut() {
        let angle = orbit.angle_offset + t * orbit.speed;
        transform.translation.x = orbit.radius * angle.cos();
        transform.translation.z = orbit.radius * angle.sin();
        transform.translation.y = orbit.y;
        transform.rotation = Quat::from_rotation_y(angle);
    }
}

pub fn spawn_ground(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let size = 20.0;
    let ground_mesh = meshes.add(Cuboid::new(size, 0.05, size));
    let ground_material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.25, 0.25, 0.28, 0.5),
        emissive: LinearRgba::new(0.15, 0.15, 0.17, 1.0),
        ..default()
    });
    commands.spawn((
        Mesh3d(ground_mesh),
        MeshMaterial3d(ground_material),
        Transform::from_xyz(0.0, -0.025, 0.0),
    ));
}

pub fn spawn_lighting(mut _commands: Commands) {
    let _ = &mut _commands;
}
