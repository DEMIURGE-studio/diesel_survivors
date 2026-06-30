//! Enemies: chasers that home in on the player. The spawner drips them in around
//! the player on a timer. Hit reactions and death rewards layer on once the
//! damage pipeline lands.

use avian3d::prelude::*;
use bevy::prelude::*;
use rand::Rng;

use crate::attributes::{Died, Health, MoveSpeed};
use crate::damage::{DamageEffect, HitEffect};
use crate::layers::{Layer, Team, TeamFilter};
use crate::player::Player;
use crate::stats::enemy_stats;
use crate::states::PlayingState;
use diesel_avian3d::prelude::{AbilityTarget, GoOff, InvokedBy};

const SPAWN_INTERVAL: f32 = 1.5;
const SPAWN_RING_RADIUS: f32 = 18.0;
const MAX_ENEMIES: usize = 60;

const MELEE_RANGE: f32 = 1.1;
const MELEE_DAMAGE: f32 = 5.0;
const MELEE_INTERVAL: f32 = 1.0;

/// Marker for hostile chasers.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Enemy;

/// Contact attack: on its cadence, fires a [`GoOff`] at the player through the
/// shared damage pipeline (team filter, resistances, `DealtDamage`/`TookDamage`,
/// `Hit`/`TookHit`). `effect` is the enemy's own melee `DamageEffect` entity,
/// spawned `InvokedBy` the enemy in [`spawn_enemies`] and despawned with it in
/// [`on_enemy_died`].
#[derive(Component)]
pub struct MeleeAttack {
    effect: Entity,
    timer: Timer,
}

impl MeleeAttack {
    fn new(effect: Entity) -> Self {
        Self {
            effect,
            timer: Timer::from_seconds(MELEE_INTERVAL, TimerMode::Repeating),
        }
    }
}

/// Cached visual handles so each spawn clones rather than allocating.
#[derive(Resource)]
struct EnemyAssets {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

#[derive(Resource)]
struct SpawnTimer(Timer);

pub struct EnemyPlugin;

impl Plugin for EnemyPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SpawnTimer(Timer::from_seconds(
            SPAWN_INTERVAL,
            TimerMode::Repeating,
        )))
        .add_observer(on_enemy_died)
        .add_systems(Startup, setup_enemy_assets)
        .add_systems(
            Update,
            (spawn_enemies, enemy_melee).run_if(in_state(PlayingState::Running)),
        )
        .add_systems(
            FixedUpdate,
            chase_player.run_if(in_state(PlayingState::Running)),
        );
    }
}

fn setup_enemy_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(EnemyAssets {
        mesh: meshes.add(Capsule3d::new(0.3, 0.6)),
        material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.85, 0.3, 0.3),
            ..default()
        }),
    });
}

fn spawn_enemies(
    mut commands: Commands,
    time: Res<Time>,
    mut timer: ResMut<SpawnTimer>,
    assets: Res<EnemyAssets>,
    player: Query<&GlobalTransform, With<Player>>,
    existing: Query<(), With<Enemy>>,
) {
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }
    if existing.iter().count() >= MAX_ENEMIES {
        return;
    }
    let Ok(player) = player.single() else {
        return;
    };

    let mut rng = rand::rng();
    let angle = rng.random_range(0.0..std::f32::consts::TAU);
    let origin = player.translation();
    let pos = Vec3::new(
        origin.x + angle.cos() * SPAWN_RING_RADIUS,
        0.6,
        origin.z + angle.sin() * SPAWN_RING_RADIUS,
    );

    let enemy = commands
        .spawn((
            Name::new("Walker"),
            Enemy,
            Team::enemies(),
            Health::default(),
            MoveSpeed::default(),
            enemy_stats(2.0, 2.5),
            Mesh3d(assets.mesh.clone()),
            MeshMaterial3d(assets.material.clone()),
            Transform::from_translation(pos),
            RigidBody::Kinematic,
            Collider::capsule(0.3, 0.6),
            CollisionLayers::new([Layer::Character], LayerMask::ALL),
            TransformInterpolation,
        ))
        .id();

    // The enemy's melee strike as a diesel effect: a `DamageEffect` leaf invoked
    // by the enemy. `enemy_melee` fires a `GoOff` at it when the enemy is in
    // range, so contact damage flows through the shared pipeline.
    let melee = commands
        .spawn((
            Name::new("WalkerMelee"),
            InvokedBy(enemy),
            TeamFilter::Enemies,
            HitEffect,
            DamageEffect::physical(&MELEE_DAMAGE.to_string()),
        ))
        .id();
    commands.entity(enemy).insert(MeleeAttack::new(melee));
}

/// Despawn an enemy when it dies, along with its melee effect entity. (Drops and
/// XP layer on here later.)
fn on_enemy_died(
    died: On<Died>,
    q_enemy: Query<&MeleeAttack, With<Enemy>>,
    mut commands: Commands,
) {
    let Ok(melee) = q_enemy.get(died.entity) else {
        return;
    };
    if let Ok(mut effect) = commands.get_entity(melee.effect) {
        effect.try_despawn();
    }
    if let Ok(mut entity) = commands.get_entity(died.entity) {
        entity.try_despawn();
    }
}

/// Enemies in range attack the player on their attack cadence, firing their melee
/// `DamageEffect` at the player through the shared damage pipeline.
fn enemy_melee(
    time: Res<Time>,
    player: Query<(Entity, &GlobalTransform), With<Player>>,
    mut enemies: Query<(&GlobalTransform, &mut MeleeAttack), With<Enemy>>,
    mut go_off: MessageWriter<GoOff>,
) {
    let Ok((player_entity, player_tf)) = player.single() else {
        return;
    };
    let player_pos = player_tf.translation();
    for (enemy_tf, mut attack) in &mut enemies {
        let fired = attack.timer.tick(time.delta()).just_finished();
        let in_range = enemy_tf.translation().distance(player_pos) <= MELEE_RANGE;
        if fired && in_range {
            go_off.write(GoOff::new(
                attack.effect,
                AbilityTarget::entity(player_entity, player_pos),
            ));
        }
    }
}

/// Steer each enemy straight at the player on the ground plane.
fn chase_player(
    player: Query<&GlobalTransform, With<Player>>,
    mut enemies: Query<(&GlobalTransform, &MoveSpeed, &mut LinearVelocity), With<Enemy>>,
) {
    let Ok(player) = player.single() else {
        return;
    };
    let target = player.translation();
    for (transform, speed, mut velocity) in &mut enemies {
        let to_player = target - transform.translation();
        let dir = Vec3::new(to_player.x, 0.0, to_player.z).normalize_or_zero();
        velocity.0 = dir * speed.value;
    }
}
