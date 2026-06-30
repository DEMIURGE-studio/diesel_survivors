//! The play field: a static ground plane and lighting. A bare stage for the
//! gameplay systems.

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::layers::Layer;

const HALF_EXTENT: f32 = 40.0;

pub struct ArenaPlugin;

impl Plugin for ArenaPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_arena);
    }
}

fn setup_arena(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Ground: a thin static slab on the Character/Terrain layer.
    commands.spawn((
        Name::new("Ground"),
        Mesh3d(meshes.add(Cuboid::new(HALF_EXTENT * 2.0, 0.2, HALF_EXTENT * 2.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.16, 0.17, 0.20),
            perceptual_roughness: 0.95,
            ..default()
        })),
        Transform::from_xyz(0.0, -0.1, 0.0),
        RigidBody::Static,
        Collider::cuboid(HALF_EXTENT * 2.0, 0.2, HALF_EXTENT * 2.0),
        CollisionLayers::new([Layer::Terrain], LayerMask::ALL),
    ));

    commands.spawn((
        Name::new("SunLight"),
        DirectionalLight {
            illuminance: 8_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(8.0, 20.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}
