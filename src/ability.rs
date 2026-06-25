//! Abilities: the diesel/gearbox showcase.
//!
//! An ability is an `invoked` state-machine shell (Ready → Invoking → Cooldown)
//! whose Invoking phase runs a `repeater` volley that spawns projectiles. The
//! projectile is itself a small state chart (Flying → Hit → Done) with its
//! `DamageEffect` authored as a `SubEffectOf` the Hit state — so a collision
//! drives the whole hit/damage flow declaratively.
//!
//! Auto-fire (VS-style) is just spamming `StartInvoke` at the player's abilities
//! every frame; each ability's Cooldown state rate-limits itself.

use std::time::Duration;

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::scene::prelude::{bsn, Scene};
use diesel_avian3d::prelude::*;
use diesel_avian3d::DirectionOffset;

use crate::damage::{DamageEffect, HitEffect};
use crate::enemy::Enemy;
use crate::layers::{Layer, TeamFilter};
use crate::player::Player;
use crate::states::PlayingState;

const MAGIC_MISSILE_PROJECTILE: &str = "abilities/magic_missile";
const MISSILE_SPEED: f32 = 18.0;
const MAGIC_MISSILE_COOLDOWN: Duration = Duration::from_millis(800);

const FIREBOLT_PROJECTILE: &str = "abilities/firebolt";
const FIREBOLT_SPEED: f32 = 13.0;
const FIREBOLT_COOLDOWN: Duration = Duration::from_millis(1100);

const FROST_SHARD_PROJECTILE: &str = "abilities/frost_shard";
const FROST_SHARD_SPEED: f32 = 22.0;
const FROST_SHARD_COOLDOWN: Duration = Duration::from_millis(500);

/// Number of ability slots a character runs at once.
pub const SLOT_COUNT: usize = 3;

// ---------------------------------------------------------------------------
// Cached visual handles for hot-spawned projectiles
// ---------------------------------------------------------------------------

#[derive(Resource)]
struct ProjectileAssets {
    missile_mesh: Handle<Mesh>,
    missile_material: Handle<StandardMaterial>,
    firebolt_mesh: Handle<Mesh>,
    firebolt_material: Handle<StandardMaterial>,
    frost_mesh: Handle<Mesh>,
    frost_material: Handle<StandardMaterial>,
}

/// Marks a projectile that steers toward its target entity each frame.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Homing;

// ---------------------------------------------------------------------------
// Ability identity + slots
// ---------------------------------------------------------------------------

/// Every ability in the game. Used to reference abilities from slots, the
/// level-up draft, and character starters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AbilityId {
    MagicMissile,
    Firebolt,
    FrostShard,
}

impl AbilityId {
    pub const ALL: [AbilityId; 3] = [
        AbilityId::MagicMissile,
        AbilityId::Firebolt,
        AbilityId::FrostShard,
    ];

    pub fn name(self) -> &'static str {
        match self {
            AbilityId::MagicMissile => "Magic Missile",
            AbilityId::Firebolt => "Firebolt",
            AbilityId::FrostShard => "Frost Shard",
        }
    }

    /// Build a fresh ability scene for this id.
    pub fn scene(self) -> Box<dyn Scene> {
        match self {
            AbilityId::MagicMissile => Box::new(magic_missile()),
            AbilityId::Firebolt => Box::new(firebolt()),
            AbilityId::FrostShard => Box::new(frost_shard()),
        }
    }
}

/// The player's equipped abilities. Filled left-to-right by the starter and the
/// level-up draft; the equip system spawns one ability entity per filled slot.
#[derive(Component, Default)]
pub struct AbilitySlots {
    slots: [Option<AbilityId>; SLOT_COUNT],
}

impl AbilitySlots {
    pub fn with_starter(starter: AbilityId) -> Self {
        let mut slots = [None; SLOT_COUNT];
        slots[0] = Some(starter);
        Self { slots }
    }

    pub fn contains(&self, id: AbilityId) -> bool {
        self.slots.iter().any(|s| *s == Some(id))
    }

    pub fn is_full(&self) -> bool {
        self.slots.iter().all(Option::is_some)
    }

    /// Equip into the first empty slot. Returns false if full or already equipped.
    pub fn equip(&mut self, id: AbilityId) -> bool {
        if self.contains(id) {
            return false;
        }
        if let Some(slot) = self.slots.iter_mut().find(|s| s.is_none()) {
            *slot = Some(id);
            true
        } else {
            false
        }
    }

    pub fn equipped(&self) -> impl Iterator<Item = AbilityId> + '_ {
        self.slots.iter().filter_map(|s| *s)
    }
}

/// Tags a spawned ability entity with the slot id it fulfills, so the equip
/// system can tell which slots are already live.
#[derive(Component, Clone, Copy)]
pub struct SlotAbility(pub AbilityId);

// ---------------------------------------------------------------------------
// Scene helpers
// ---------------------------------------------------------------------------

/// Single-component scene inserting `StateComponent(value)` (the canonical
/// `template(…)` form, since `StateComponent`/its payloads aren't `Default`).
fn state<T: Component + Clone>(value: T) -> impl Scene {
    let sc = StateComponent(value);
    bsn! { template(move |_| Ok(sc.clone())) }
}

/// Firing leaf: spawn a projectile template at the invoker (a touch above the
/// ground, at enemy mid-height), aimed at whatever the invoker is targeting.
fn configure_projectile_spawn(template_id: &'static str) -> impl Scene {
    bsn! {
        SpawnConfig::invoker_offset_target(
            template_id,
            Vec3Offset::Fixed(DirectionOffset::new(Dir3::Y, 0.0)),
            TargetGenerator::at_invoker_target(),
        )
    }
}

// ---------------------------------------------------------------------------
// Magic Missile
// ---------------------------------------------------------------------------

/// Ready → Invoking (a `ProjectileCount`-long volley) → Cooldown.
pub fn magic_missile() -> impl Scene {
    invoked::<Vec3, _, _>("Magic Missile", MAGIC_MISSILE_COOLDOWN, |root| {
        repeater::<Vec3>(
            root,
            "ProjectileCount@invoker",
            0.12,
            configure_projectile_spawn(MAGIC_MISSILE_PROJECTILE),
        )
    })
}

/// The projectile: Flying → Hit → Done. Flies straight (gravity off), dies on
/// contact (arcane damage scaled by the invoker's `Damage`) or after 2s.
fn magic_missile_projectile() -> impl Scene {
    bsn! {
        #Root
            Name::new("MagicMissile")
            LinearProjectileEffect { speed: MISSILE_SPEED, horizontal: true }
            Homing
            TeamFilter::Enemies
            CollisionLayers::new([Layer::Projectile], [Layer::Character])
            Collider::sphere(0.15)
            Visibility::Inherited
            template(|ctx| Ok(Mesh3d(ctx.resource::<ProjectileAssets>().missile_mesh.clone())))
            template(|ctx| Ok(MeshMaterial3d(ctx.resource::<ProjectileAssets>().missile_material.clone())))
            StateMachine InitialState(#Flying)
        Substates [
            #Flying Transitions [
                (Target(#Hit) MessageEdge::<CollidedEntity>::default()),
                (Target(#Done) AlwaysEdge Delay::from_secs_f32(2.0)),
            ],
            #Hit Substates [
                (SubEffectOf(#Hit) InvokedBy(#Root)
                    Name::new("DealDamage")
                    HitEffect
                    DamageEffect::arcane("Damage@invoker")),
            ] Transitions [
                (Target(#Done) AlwaysEdge)
            ],
            #Done state(DelayedDespawn::now()),
        ]
    }
}

// ---------------------------------------------------------------------------
// Firebolt — slower, hard-hitting fire projectile (Sentinel's starter)
// ---------------------------------------------------------------------------

/// Ready → Invoking (single bolt) → Cooldown.
pub fn firebolt() -> impl Scene {
    invoked::<Vec3, _, _>("Firebolt", FIREBOLT_COOLDOWN, |root| {
        repeater::<Vec3>(
            root,
            "ProjectileCount@invoker",
            0.12,
            configure_projectile_spawn(FIREBOLT_PROJECTILE),
        )
    })
}

/// Bigger, slower homing bolt dealing fire damage scaled by the invoker's
/// `Damage` (which, for the Sentinel, scales with MaxHealth).
fn firebolt_projectile() -> impl Scene {
    bsn! {
        #Root
            Name::new("Firebolt")
            LinearProjectileEffect { speed: FIREBOLT_SPEED, horizontal: true }
            Homing
            TeamFilter::Enemies
            CollisionLayers::new([Layer::Projectile], [Layer::Character])
            Collider::sphere(0.35)
            Visibility::Inherited
            template(|ctx| Ok(Mesh3d(ctx.resource::<ProjectileAssets>().firebolt_mesh.clone())))
            template(|ctx| Ok(MeshMaterial3d(ctx.resource::<ProjectileAssets>().firebolt_material.clone())))
            StateMachine InitialState(#Flying)
        Substates [
            #Flying Transitions [
                (Target(#Hit) MessageEdge::<CollidedEntity>::default()),
                (Target(#Done) AlwaysEdge Delay::from_secs_f32(2.5)),
            ],
            #Hit Substates [
                (SubEffectOf(#Hit) InvokedBy(#Root)
                    Name::new("DealDamage")
                    HitEffect
                    DamageEffect::fire("Damage@invoker")),
            ] Transitions [
                (Target(#Done) AlwaysEdge)
            ],
            #Done state(DelayedDespawn::now()),
        ]
    }
}

// ---------------------------------------------------------------------------
// Frost Shard — fast, low-cooldown cold projectile
// ---------------------------------------------------------------------------

/// Ready → Invoking (single shard) → Cooldown.
pub fn frost_shard() -> impl Scene {
    invoked::<Vec3, _, _>("Frost Shard", FROST_SHARD_COOLDOWN, |root| {
        repeater::<Vec3>(
            root,
            "ProjectileCount@invoker",
            0.12,
            configure_projectile_spawn(FROST_SHARD_PROJECTILE),
        )
    })
}

/// Small, fast homing shard dealing cold damage.
fn frost_shard_projectile() -> impl Scene {
    bsn! {
        #Root
            Name::new("FrostShard")
            LinearProjectileEffect { speed: FROST_SHARD_SPEED, horizontal: true }
            Homing
            TeamFilter::Enemies
            CollisionLayers::new([Layer::Projectile], [Layer::Character])
            Collider::sphere(0.12)
            Visibility::Inherited
            template(|ctx| Ok(Mesh3d(ctx.resource::<ProjectileAssets>().frost_mesh.clone())))
            template(|ctx| Ok(MeshMaterial3d(ctx.resource::<ProjectileAssets>().frost_material.clone())))
            StateMachine InitialState(#Flying)
        Substates [
            #Flying Transitions [
                (Target(#Hit) MessageEdge::<CollidedEntity>::default()),
                (Target(#Done) AlwaysEdge Delay::from_secs_f32(1.5)),
            ],
            #Hit Substates [
                (SubEffectOf(#Hit) InvokedBy(#Root)
                    Name::new("DealDamage")
                    HitEffect
                    DamageEffect::cold("Damage@invoker")),
            ] Transitions [
                (Target(#Done) AlwaysEdge)
            ],
            #Done state(DelayedDespawn::now()),
        ]
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct AbilityPlugin;

impl Plugin for AbilityPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TemplateRegistry>()
            .add_systems(Startup, (setup_projectile_assets, register_projectiles))
            .add_systems(Update, equip_abilities)
            .add_systems(
                Update,
                (update_aim, auto_invoke, home_missiles)
                    .run_if(in_state(PlayingState::Running)),
            );
    }
}

fn setup_projectile_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(ProjectileAssets {
        missile_mesh: meshes.add(Sphere::new(0.15)),
        missile_material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.6, 0.4, 1.0),
            emissive: LinearRgba::new(2.0, 1.0, 5.0, 1.0),
            ..default()
        }),
        firebolt_mesh: meshes.add(Sphere::new(0.35)),
        firebolt_material: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.4, 0.1),
            emissive: LinearRgba::new(6.0, 2.0, 0.0, 1.0),
            ..default()
        }),
        frost_mesh: meshes.add(Sphere::new(0.12)),
        frost_material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.5, 0.85, 1.0),
            emissive: LinearRgba::new(1.0, 3.0, 5.0, 1.0),
            ..default()
        }),
    });
}

fn register_projectiles(mut registry: ResMut<TemplateRegistry>) {
    registry.register(MAGIC_MISSILE_PROJECTILE, || {
        Box::new(magic_missile_projectile())
    });
    registry.register(FIREBOLT_PROJECTILE, || Box::new(firebolt_projectile()));
    registry.register(FROST_SHARD_PROJECTILE, || {
        Box::new(frost_shard_projectile())
    });
}

/// Spawn an ability entity for every filled slot that isn't live yet. Runs when
/// `AbilitySlots` changes (player spawn, level-up draft), so equipping an ability
/// brings it online without disturbing the others' cooldowns.
fn equip_abilities(
    q_player: Query<(Entity, &AbilitySlots, Option<&Invokes>), (With<Player>, Changed<AbilitySlots>)>,
    q_slot: Query<&SlotAbility>,
    mut commands: Commands,
) {
    for (player, slots, invokes) in &q_player {
        let live: Vec<AbilityId> = invokes
            .into_iter()
            .flat_map(|inv| inv.into_iter())
            .filter_map(|&e| q_slot.get(e).ok().map(|s| s.0))
            .collect();
        for id in slots.equipped() {
            if !live.contains(&id) {
                commands
                    .spawn_scene(id.scene())
                    .insert((InvokedBy(player), SlotAbility(id)));
            }
        }
    }
}

/// Aim the player at the nearest enemy (entity + position) so auto-fired
/// abilities target it and homing projectiles can track it.
fn update_aim(
    mut player: Query<(&GlobalTransform, &mut InvokerTarget), With<Player>>,
    enemies: Query<(Entity, &GlobalTransform), With<Enemy>>,
) {
    let Ok((player_tf, mut target)) = player.single_mut() else {
        return;
    };
    let origin = player_tf.translation();
    let nearest = enemies.iter().min_by(|a, b| {
        let da = a.1.translation().distance_squared(origin);
        let db = b.1.translation().distance_squared(origin);
        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
    });
    if let Some((entity, enemy_tf)) = nearest {
        target.entity = Some(entity);
        target.position = enemy_tf.translation();
    }
}

/// Steer homing projectiles toward their target's current position (XZ plane).
fn home_missiles(
    mut missiles: Query<(&GlobalTransform, &mut LinearProjectile, &AbilityTarget), With<Homing>>,
    targets: Query<&GlobalTransform>,
) {
    for (tf, mut proj, target) in &mut missiles {
        let Some(target_entity) = target.entity else {
            continue;
        };
        let Ok(target_tf) = targets.get(target_entity) else {
            continue;
        };
        let mut delta = target_tf.translation() - tf.translation();
        delta.y = 0.0;
        let dir = delta.normalize_or_zero();
        if dir != Vec3::ZERO {
            proj.direction = dir;
        }
    }
}

/// Auto-fire: try to invoke every ability the player owns each frame. Cooldown
/// states gate the actual firing.
fn auto_invoke(
    player: Query<(&Invokes, &InvokerTarget), With<Player>>,
    mut writer: MessageWriter<StartInvoke>,
) {
    let Ok((invokes, target)) = player.single() else {
        return;
    };
    let aim = AbilityTarget::position(target.position);
    for &ability in invokes.into_iter() {
        writer.write(StartInvoke::new(ability, aim));
    }
}
