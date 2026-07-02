//! Pickups: orbs dropped by dying enemies that drift to the player and apply an
//! effect on contact.
//!
//! Two kinds:
//! - **Health orb**: a *consumable*. Instantly restores health on contact.
//! - **Buff orb**: attaches a *timed buff* (see [`crate::data::buffs`]), a diesel
//!   sustained-modifier scene that boosts a stat for a few seconds then removes
//!   itself.
//!
//! Collection reuses the player's `PickupRadius` attribute: orbs within it home
//! in, and on contact apply and despawn.

use bevy::prelude::*;
use bevy::scene::prelude::{CommandsSceneExt, Scene};
use rand::Rng;

use crate::attributes::{Died, Health, PickupRadius};
use crate::data::buffs;
use crate::enemy::Enemy;
use crate::player::Player;
use crate::states::PlayingState;

/// Chance a dying enemy drops anything at all.
const DROP_CHANCE: f64 = 0.30;
/// Of drops, the share that are health orbs (the rest are buffs).
const HEAL_SHARE: f64 = 0.55;
/// Health restored by a health orb.
const HEAL_AMOUNT: f32 = 25.0;
/// Contact distance at which an orb is collected.
const COLLECT_RADIUS: f32 = 0.8;
/// How fast an orb flies toward the player once within pickup range.
const ATTRACT_SPEED: f32 = 16.0;
/// Resting height of an orb above the ground.
const ORB_HEIGHT: f32 = 0.5;

/// What a pickup does when collected.
#[derive(Clone, Copy)]
pub enum PickupKind {
    /// Restore this much health to the collector.
    Heal(f32),
    /// Spawn a timed-buff scene attached to the collector (passed as the invoker).
    Buff(fn(Entity) -> Box<dyn Scene>),
}

/// Marks a world orb and carries what it grants on contact.
#[derive(Component, Clone, Copy)]
pub struct Pickup(pub PickupKind);

/// Cached orb visuals so each drop clones rather than allocating.
#[derive(Resource)]
struct PickupAssets {
    heal_mesh: Handle<Mesh>,
    heal_material: Handle<StandardMaterial>,
    buff_mesh: Handle<Mesh>,
    buff_material: Handle<StandardMaterial>,
}

pub struct PickupPlugin;

impl Plugin for PickupPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_pickup_assets)
            .add_observer(drop_on_death)
            .add_systems(
                Update,
                attract_and_collect.run_if(in_state(PlayingState::Running)),
            );
    }
}

fn setup_pickup_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(PickupAssets {
        heal_mesh: meshes.add(Sphere::new(0.25)),
        heal_material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.3, 1.0, 0.4),
            emissive: LinearRgba::new(0.5, 4.0, 1.0, 1.0),
            ..default()
        }),
        buff_mesh: meshes.add(Sphere::new(0.25)),
        buff_material: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.85, 0.2),
            emissive: LinearRgba::new(5.0, 3.5, 0.5, 1.0),
            ..default()
        }),
    });
}

/// Roll the drop table when an enemy dies and spawn an orb at its position.
fn drop_on_death(
    died: On<Died>,
    q_enemy: Query<&GlobalTransform, With<Enemy>>,
    assets: Res<PickupAssets>,
    mut commands: Commands,
) {
    let Ok(transform) = q_enemy.get(died.entity) else {
        return;
    };
    let mut rng = rand::rng();
    if !rng.random_bool(DROP_CHANCE) {
        return;
    }

    let (kind, mesh, material) = if rng.random_bool(HEAL_SHARE) {
        (
            PickupKind::Heal(HEAL_AMOUNT),
            assets.heal_mesh.clone(),
            assets.heal_material.clone(),
        )
    } else {
        // Pick a buff flavor; both are timed sustained-modifier scenes.
        let factory: fn(Entity) -> Box<dyn Scene> =
            if rng.random_bool(0.5) { buffs::rage } else { buffs::haste };
        (
            PickupKind::Buff(factory),
            assets.buff_mesh.clone(),
            assets.buff_material.clone(),
        )
    };

    let mut pos = transform.translation();
    pos.y = ORB_HEIGHT;
    commands.spawn((
        Name::new("Pickup"),
        Pickup(kind),
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_translation(pos),
        Visibility::Inherited,
    ));
}

/// Draw orbs within the player's pickup radius toward them, and apply + despawn
/// any that reach contact range.
fn attract_and_collect(
    time: Res<Time>,
    mut player: Query<(Entity, &GlobalTransform, &PickupRadius, &mut Health), With<Player>>,
    mut orbs: Query<(Entity, &mut Transform, &Pickup)>,
    mut commands: Commands,
) {
    let Ok((player_entity, player_tf, radius, mut health)) = player.single_mut() else {
        return;
    };
    let player_pos = player_tf.translation();
    let dt = time.delta_secs();

    for (orb_entity, mut transform, pickup) in &mut orbs {
        let to_player = player_pos - transform.translation;
        let dist = to_player.length();

        if dist <= COLLECT_RADIUS {
            match pickup.0 {
                PickupKind::Heal(amount) => {
                    health.current = (health.current + amount).min(health.max);
                }
                PickupKind::Buff(factory) => {
                    commands.spawn_scene(factory(player_entity));
                }
            }
            if let Ok(mut ec) = commands.get_entity(orb_entity) {
                ec.try_despawn();
            }
        } else if dist <= radius.value {
            let step = (ATTRACT_SPEED * dt).min(dist);
            transform.translation += to_player.normalize_or_zero() * step;
        }
    }
}
