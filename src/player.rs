//! The player: a kinematic, interpolation-smoothed top-down controller. The
//! entity is authored as a per-character BSN scene (see
//! [`crate::data::characters::player_scene`]); this module owns the marker, the
//! input/movement systems, and the camera.
//!
//! Movement samples WASD each frame into [`MoveInput`], then in `FixedUpdate`
//! sets the kinematic body's `LinearVelocity` to `direction * MoveSpeed`. Avian's
//! transform interpolation smooths the fixed-step motion across render frames, so
//! the character glides regardless of the physics tick rate.

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::scene::prelude::CommandsSceneExt;

use crate::attributes::MoveSpeed;
use crate::data::characters::{player_scene, SelectedCharacter};
use crate::states::AppState;

/// Marker for the player-controlled character.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Player;

/// This frame's desired move direction on the ground plane (normalized, or zero).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct MoveInput(pub Vec3);

/// Camera placement relative to the player: 24 units up, pulled back along +Z by
/// `24 * tan(20 deg)` so the view tilts ~20 deg off straight-down.
pub const CAMERA_OFFSET: Vec3 = Vec3::new(0.0, 24.0, 8.74);

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        // Player lives for the duration of a Playing session; respawned each
        // time Playing is (re-)entered (e.g. after a game-over restart).
        app.add_systems(OnEnter(AppState::Playing), spawn_player)
            .add_systems(Update, read_move_input)
            .add_systems(FixedUpdate, apply_movement)
            .add_systems(PostUpdate, camera_follow.after(TransformSystems::Propagate));
    }
}

fn spawn_player(mut commands: Commands, selected: Res<SelectedCharacter>) {
    commands.spawn_scene(player_scene(selected.0));
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
        // preserves the 20 deg tilt relative to the followed point.
        cam.translation = target + CAMERA_OFFSET;
    }
}
