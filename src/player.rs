//! The player: a fresh kinematic, interpolation-smoothed top-down controller.
//!
//! Movement is integer-simple: sample WASD each frame into [`MoveInput`], then
//! in `FixedUpdate` set the kinematic body's `LinearVelocity` to
//! `direction * MoveSpeed`. Avian's transform interpolation (enabled app-wide in
//! `main`) smooths the fixed-step motion across render frames, so the character
//! glides regardless of the physics tick rate.

use avian3d::prelude::*;
use bevy::prelude::*;
use diesel_avian3d::gauge::prelude::AttributeInitializer;
use diesel_avian3d::prelude::*;

use crate::ability::AbilitySlots;
use crate::attributes::{Health, MoveSpeed, PickupRadius};
use crate::characters::SelectedCharacter;
use crate::layers::{Layer, Team};
use crate::meta::MetaProgress;
use crate::states::AppState;

/// Marker for the player-controlled character.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Player;

/// This frame's desired move direction on the ground plane (normalized, or zero).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct MoveInput(pub Vec3);

/// Camera placement relative to the player: 24 units up, pulled back along +Z by
/// `24 * tan(20°)` so the view tilts ~20° off straight-down.
pub const CAMERA_OFFSET: Vec3 = Vec3::new(0.0, 24.0, 8.74);

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        // Player lives for the duration of a Playing session; respawned each
        // time we (re-)enter Playing (e.g. after a game-over restart).
        app.add_systems(OnEnter(AppState::Playing), spawn_player)
            .add_systems(Update, read_move_input)
            .add_systems(FixedUpdate, apply_movement)
            .add_systems(PostUpdate, camera_follow.after(TransformSystems::Propagate));
    }
}

fn spawn_player(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    selected: Res<SelectedCharacter>,
    meta: Res<MetaProgress>,
) {
    // Character baseline + scaling, then permanent metaprogression bonuses.
    let mut stats = selected.0.mod_set();
    meta.apply_to(&mut stats);

    commands.spawn((
        Name::new("Player"),
        Player,
        Team::player(),
        MoveInput::default(),
        InvokerTarget::position(Vec3::ZERO),
        Health::default(),
        MoveSpeed::default(),
        PickupRadius::default(),
        AbilitySlots::with_starter(selected.0.starter_ability()),
        AttributeInitializer::new(stats),
        // Visuals.
        Mesh3d(meshes.add(Capsule3d::new(0.4, 0.8))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.35, 0.7, 1.0),
            ..default()
        })),
        Transform::from_xyz(0.0, 0.7, 0.0),
        // Physics: kinematic body driven by velocity, smoothed by interpolation.
        (
            RigidBody::Kinematic,
            Collider::capsule(0.4, 0.8),
            CollisionLayers::new([Layer::Character], LayerMask::ALL),
            TransformInterpolation,
        ),
    ));
}

/// Sample WASD into a normalized ground-plane direction. Camera looks down -Y,
/// so screen-up (W) is -Z and screen-right (D) is +X.
fn read_move_input(keys: Res<ButtonInput<KeyCode>>, mut q: Query<&mut MoveInput, With<Player>>) {
    let mut dir = Vec3::ZERO;
    if keys.pressed(KeyCode::KeyW) {
        dir.z -= 1.0;
    }
    if keys.pressed(KeyCode::KeyS) {
        dir.z += 1.0;
    }
    if keys.pressed(KeyCode::KeyA) {
        dir.x -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) {
        dir.x += 1.0;
    }
    for mut input in &mut q {
        input.0 = dir.normalize_or_zero();
    }
}

fn apply_movement(mut q: Query<(&MoveInput, &MoveSpeed, &mut LinearVelocity), With<Player>>) {
    for (input, speed, mut velocity) in &mut q {
        let planar = input.0 * speed.value;
        velocity.0 = Vec3::new(planar.x, 0.0, planar.z);
    }
}

/// Keep the top-down camera centered over the player with a fixed height offset.
fn camera_follow(
    player: Query<&GlobalTransform, With<Player>>,
    mut camera: Query<&mut Transform, (With<Camera3d>, Without<Player>)>,
) {
    let Ok(player) = player.single() else {
        return;
    };
    let target = player.translation();
    for mut cam in &mut camera {
        // Keep the rotation set at spawn; translating by a constant offset
        // preserves the 20° tilt relative to the followed point.
        cam.translation = target + CAMERA_OFFSET;
    }
}
