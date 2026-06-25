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


use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::scene::prelude::{bsn, Scene};
use diesel_avian3d::gauge::prelude::ModifierSet;
use diesel_avian3d::prelude::*;
use diesel_avian3d::DirectionOffset;

use crate::damage::{DamageEffect, HitEffect};
use crate::enemy::Enemy;
use crate::layers::{Layer, TeamFilter};
use crate::player::Player;
use crate::states::PlayingState;

const MAGIC_MISSILE_PROJECTILE: &str = "abilities/magic_missile";
const MISSILE_SPEED: f32 = 18.0;
const MAGIC_MISSILE_COOLDOWN: f32 = 0.8;

const FIREBOLT_PROJECTILE: &str = "abilities/firebolt";
const FIREBOLT_SPEED: f32 = 13.0;
const FIREBOLT_COOLDOWN: f32 = 1.1;

const FROST_SHARD_PROJECTILE: &str = "abilities/frost_shard";
const FROST_SHARD_SPEED: f32 = 22.0;
const FROST_SHARD_COOLDOWN: f32 = 0.5;

const FIREBALL_PROJECTILE: &str = "abilities/fireball_projectile";
const FIREBALL_EXPLOSION: &str = "abilities/fireball_explosion";
const FIREBALL_SPEED: f32 = 12.0;
const FIREBALL_COOLDOWN: f32 = 1.3;
const EXPLOSION_RADIUS: f32 = 3.0;

const ICE_STORM_ZONE: &str = "abilities/ice_storm_zone";
const FROST_PULSE: &str = "abilities/frost_pulse";
const ICE_STORM_COOLDOWN: f32 = 4.0;
const STORM_RADIUS: f32 = 3.2;
const STORM_PULSE_COUNT: &str = "8";

const BLADE_ORBIT_RADIUS: f32 = 2.2;
const BLADE_ORBIT_SPEED: f32 = 3.5;

/// Number of ability slots a character runs at once.
pub const SLOT_COUNT: usize = 3;

/// Despawns its entity once the timer finishes. Used by transient AoE bursts.
#[derive(Component)]
pub struct Lifetime(Timer);

impl Lifetime {
    fn secs(secs: f32) -> Self {
        Self(Timer::from_seconds(secs, TimerMode::Once))
    }
}

/// A blade that circles its invoker. The orbit system advances `angle`.
#[derive(Component, Default, Clone)]
pub struct Orbiter {
    angle: f32,
}

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
    explosion_mesh: Handle<Mesh>,
    explosion_material: Handle<StandardMaterial>,
    pulse_mesh: Handle<Mesh>,
    pulse_material: Handle<StandardMaterial>,
    storm_mesh: Handle<Mesh>,
    storm_material: Handle<StandardMaterial>,
    blade_mesh: Handle<Mesh>,
    blade_material: Handle<StandardMaterial>,
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
    Fireball,
    OrbitingBlade,
    IceStorm,
}

impl AbilityId {
    pub const ALL: [AbilityId; 6] = [
        AbilityId::MagicMissile,
        AbilityId::Firebolt,
        AbilityId::FrostShard,
        AbilityId::Fireball,
        AbilityId::OrbitingBlade,
        AbilityId::IceStorm,
    ];

    pub fn name(self) -> &'static str {
        match self {
            AbilityId::MagicMissile => "Magic Missile",
            AbilityId::Firebolt => "Firebolt",
            AbilityId::FrostShard => "Frost Shard",
            AbilityId::Fireball => "Fireball",
            AbilityId::OrbitingBlade => "Orbiting Blade",
            AbilityId::IceStorm => "Ice Storm",
        }
    }

    /// Build a fresh ability scene for this id.
    pub fn scene(self) -> Box<dyn Scene> {
        match self {
            AbilityId::MagicMissile => Box::new(magic_missile()),
            AbilityId::Firebolt => Box::new(firebolt()),
            AbilityId::FrostShard => Box::new(frost_shard()),
            AbilityId::Fireball => Box::new(fireball()),
            AbilityId::OrbitingBlade => Box::new(orbiting_blade()),
            AbilityId::IceStorm => Box::new(ice_storm()),
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

/// Per-ability base attributes. Each stat is split into a plain `*Base` (the
/// per-ability rank-up target — a level-up applies a gauge instant to it) and a
/// derived *effective* value that folds in the matching player global; the
/// ability's effects (and spawned projectiles) read the derived one as
/// `"...@ability"`, resolved against the ability root's `@invoker` (the player):
/// `Cooldown = CooldownBase × CooldownMult`, `Area = AreaBase × Area`,
/// `ProjectileSpeed = ProjectileSpeedBase × ProjectileSpeed`.
fn ability_base(cooldown: f32, projectile_speed: Option<f32>, area: Option<f32>) -> ModifierSet {
    let mut set = ModifierSet::new();
    set.add("CooldownBase", cooldown);
    set.add_expr("Cooldown", "CooldownBase * CooldownMult@invoker");
    if let Some(speed) = projectile_speed {
        set.add("ProjectileSpeedBase", speed);
        set.add_expr("ProjectileSpeed", "ProjectileSpeedBase * ProjectileSpeed@invoker");
    }
    if let Some(radius) = area {
        set.add("AreaBase", radius);
        set.add_expr("Area", "AreaBase * Area@invoker");
    }
    set
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
    invoked_with(
        "Magic Missile",
        MAGIC_MISSILE_COOLDOWN,
        ability_base(MAGIC_MISSILE_COOLDOWN, Some(MISSILE_SPEED), None),
        |root| {
            repeater(
                root,
                "ProjectileCount@invoker",
                "0.12 / AttackSpeed@invoker",
                configure_projectile_spawn(MAGIC_MISSILE_PROJECTILE),
            )
        },
    )
}

/// The projectile: Flying → Hit → Done. Flies straight (gravity off), dies on
/// contact (arcane damage scaled by the invoker's `Damage`) or after 2s.
fn magic_missile_projectile() -> impl Scene {
    bsn! {
        #Root
            Name::new("MagicMissile")
            LinearProjectileEffect { speed: MISSILE_SPEED, horizontal: true }
            template(|_| Ok(bevy_gauge::attributes! { "Speed" => "ProjectileSpeed@ability" }))
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
                    DamageEffect::arcane("Damage@invoker * Damage@ability")),
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
    invoked_with(
        "Firebolt",
        FIREBOLT_COOLDOWN,
        ability_base(FIREBOLT_COOLDOWN, Some(FIREBOLT_SPEED), None),
        |root| {
            repeater(
                root,
                "ProjectileCount@invoker",
                "0.12 / AttackSpeed@invoker",
                configure_projectile_spawn(FIREBOLT_PROJECTILE),
            )
        },
    )
}

/// Bigger, slower homing bolt dealing fire damage scaled by the invoker's
/// `Damage` (which, for the Sentinel, scales with MaxHealth).
fn firebolt_projectile() -> impl Scene {
    bsn! {
        #Root
            Name::new("Firebolt")
            LinearProjectileEffect { speed: FIREBOLT_SPEED, horizontal: true }
            template(|_| Ok(bevy_gauge::attributes! { "Speed" => "ProjectileSpeed@ability" }))
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
                    DamageEffect::fire("Damage@invoker * Damage@ability")),
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
    invoked_with(
        "Frost Shard",
        FROST_SHARD_COOLDOWN,
        ability_base(FROST_SHARD_COOLDOWN, Some(FROST_SHARD_SPEED), None),
        |root| {
            repeater(
                root,
                "ProjectileCount@invoker",
                "0.12 / AttackSpeed@invoker",
                configure_projectile_spawn(FROST_SHARD_PROJECTILE),
            )
        },
    )
}

/// Small, fast homing shard dealing cold damage.
fn frost_shard_projectile() -> impl Scene {
    bsn! {
        #Root
            Name::new("FrostShard")
            LinearProjectileEffect { speed: FROST_SHARD_SPEED, horizontal: true }
            template(|_| Ok(bevy_gauge::attributes! { "Speed" => "ProjectileSpeed@ability" }))
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
                    DamageEffect::cold("Damage@invoker * Damage@ability")),
            ] Transitions [
                (Target(#Done) AlwaysEdge)
            ],
            #Done state(DelayedDespawn::now()),
        ]
    }
}

// ---------------------------------------------------------------------------
// Fireball — projectile that bursts into an AoE explosion on impact
// ---------------------------------------------------------------------------

/// Firing leaf that spawns a template at the aimed position (not the invoker).
fn configure_zone_spawn(template_id: &'static str) -> impl Scene {
    bsn! { SpawnConfig::target(template_id) }
}

/// Firing leaf that spawns a template at the spawner's own root position.
fn configure_root_spawn(template_id: &'static str) -> impl Scene {
    bsn! { SpawnConfig::root(template_id) }
}

pub fn fireball() -> impl Scene {
    invoked_with(
        "Fireball",
        FIREBALL_COOLDOWN,
        ability_base(FIREBALL_COOLDOWN, Some(FIREBALL_SPEED), Some(EXPLOSION_RADIUS)),
        |root| {
            repeater(
                root,
                "ProjectileCount@invoker",
                "0.12 / AttackSpeed@invoker",
                configure_projectile_spawn(FIREBALL_PROJECTILE),
            )
        },
    )
}

/// Slow fire projectile; on hit it spawns an explosion at the impact point and
/// deals no direct damage (the explosion does the work).
fn fireball_projectile() -> impl Scene {
    bsn! {
        #Root
            Name::new("Fireball")
            LinearProjectileEffect { speed: FIREBALL_SPEED, horizontal: true }
            template(|_| Ok(bevy_gauge::attributes! { "Speed" => "ProjectileSpeed@ability" }))
            Homing
            TeamFilter::Enemies
            CollisionLayers::new([Layer::Projectile], [Layer::Character])
            Collider::sphere(0.3)
            Visibility::Inherited
            template(|ctx| Ok(Mesh3d(ctx.resource::<ProjectileAssets>().firebolt_mesh.clone())))
            template(|ctx| Ok(MeshMaterial3d(ctx.resource::<ProjectileAssets>().firebolt_material.clone())))
            StateMachine InitialState(#Flying)
        Substates [
            #Flying Transitions [
                (Target(#Hit) MessageEdge::<CollidedEntity>::default()),
                (Target(#Done) AlwaysEdge Delay::from_secs_f32(3.0)),
            ],
            #Hit Substates [
                (SubEffectOf(#Hit) InvokedBy(#Root)
                    Name::new("SpawnExplosion")
                    SpawnConfig::passed(FIREBALL_EXPLOSION)),
            ] Transitions [
                (Target(#Done) AlwaysEdge)
            ],
            #Done state(DelayedDespawn::now()),
        ]
    }
}

/// One-shot AoE: on entry, gathers every entity in radius and burns it, then
/// fades out after a short lifetime.
fn fireball_explosion() -> impl Scene {
    bsn! {
        #Root
            Name::new("Explosion")
            TeamFilter::Enemies
            Visibility::Inherited
            template(|_| Ok(Lifetime::secs(0.35)))
            template(|ctx| Ok(Mesh3d(ctx.resource::<ProjectileAssets>().explosion_mesh.clone())))
            template(|ctx| Ok(MeshMaterial3d(ctx.resource::<ProjectileAssets>().explosion_material.clone())))
            StateMachine InitialState(#Active)
        Substates [
            #Active GoOffConfig::default()
            Substates [
                #AoE SubEffectOf(#Active) InvokedBy(#Root)
                    TargetMutator::root_gathering(AvianGatherer::AllEntitiesInRadius(EXPLOSION_RADIUS))
                    // Gauge-drive the gather radius: the gatherer's single field
                    // resolves against `"TargetMutator.gatherer"`, aliased here to
                    // the spell's `Area` so an upgrade scales every explosion.
                    template(|_| Ok(bevy_gauge::attributes! { "TargetMutator.gatherer" => "Area@ability" }))
                Substates [
                    (SubEffectOf(#AoE) InvokedBy(#Root)
                        Name::new("Burn")
                        HitEffect
                        DamageEffect::fire("Damage@invoker * Damage@ability * 1.5")),
                ],
            ],
        ]
    }
}

// ---------------------------------------------------------------------------
// Ice Storm — a placed zone that pulses cold AoE for a few seconds
// ---------------------------------------------------------------------------

pub fn ice_storm() -> impl Scene {
    invoked_with(
        "Ice Storm",
        ICE_STORM_COOLDOWN,
        ability_base(ICE_STORM_COOLDOWN, None, Some(STORM_RADIUS)),
        |root| {
            repeater(
                root,
                "1",
                "0.1 / AttackSpeed@invoker",
                configure_zone_spawn(ICE_STORM_ZONE),
            )
        },
    )
}

/// The zone: a repeater spawns a frost pulse at its position every tick, then it
/// despawns when the volley is done.
fn ice_storm_zone() -> impl Scene {
    bsn! {
        #Root
            Name::new("IceStorm")
            TeamFilter::Enemies
            Visibility::Inherited
            template(|ctx| Ok(Mesh3d(ctx.resource::<ProjectileAssets>().storm_mesh.clone())))
            template(|ctx| Ok(MeshMaterial3d(ctx.resource::<ProjectileAssets>().storm_material.clone())))
            StateMachine InitialState(#Pulsing)
        Substates [
            #Pulsing Transitions [
                (Target(#Done) MessageEdge::<Done>::default())
            ] Substates [
                #Inner repeater(
                    #Root, STORM_PULSE_COUNT, "0.5 / AttackSpeed@invoker",
                    configure_root_spawn(FROST_PULSE),
                ),
            ],
            #Done state(DelayedDespawn::now()),
        ]
    }
}

/// A single cold AoE tick (like the explosion, but chilling).
fn frost_pulse() -> impl Scene {
    bsn! {
        #Root
            Name::new("FrostPulse")
            TeamFilter::Enemies
            Visibility::Inherited
            template(|_| Ok(Lifetime::secs(0.3)))
            template(|ctx| Ok(Mesh3d(ctx.resource::<ProjectileAssets>().pulse_mesh.clone())))
            template(|ctx| Ok(MeshMaterial3d(ctx.resource::<ProjectileAssets>().pulse_material.clone())))
            StateMachine InitialState(#Active)
        Substates [
            #Active GoOffConfig::default()
            Substates [
                #AoE SubEffectOf(#Active) InvokedBy(#Root)
                    TargetMutator::root_gathering(AvianGatherer::AllEntitiesInRadius(STORM_RADIUS))
                    template(|_| Ok(bevy_gauge::attributes! { "TargetMutator.gatherer" => "Area@ability" }))
                Substates [
                    (SubEffectOf(#AoE) InvokedBy(#Root)
                        Name::new("Chill")
                        HitEffect
                        DamageEffect::cold("Damage@invoker * Damage@ability * 0.6")),
                ],
            ],
        ]
    }
}

// ---------------------------------------------------------------------------
// Orbiting Blade — a sustained ability: the ability entity *is* the orbiter
// ---------------------------------------------------------------------------

/// A persistent blade circling the player. Its `#Active` state self-transitions
/// on `CollidedEntity`, re-firing its damage each time it sweeps an enemy.
pub fn orbiting_blade() -> impl Scene {
    bsn! {
        #Root
            Name::new("OrbitingBlade")
            // `Ability` so `@ability` resolves to this entity — its own `Damage`
            // multiplier makes the blade rank-uppable like the spawned abilities.
            Ability
            template(|_| Ok(bevy_gauge::attributes! { "Damage" => 1.0 }))
            Orbiter
            TeamFilter::Enemies
            Sensor
            CollisionEventsEnabled
            template(|_| Ok(RigidBody::Kinematic))
            Collider::sphere(0.35)
            CollisionLayers::new([Layer::Projectile], [Layer::Character])
            Visibility::Inherited
            template(|ctx| Ok(Mesh3d(ctx.resource::<ProjectileAssets>().blade_mesh.clone())))
            template(|ctx| Ok(MeshMaterial3d(ctx.resource::<ProjectileAssets>().blade_material.clone())))
            Transform::default()
            StateMachine InitialState(#Active)
        Substates [
            #Active Transitions [
                (Target(#Active) MessageEdge::<CollidedEntity>::default())
            ] Substates [
                (SubEffectOf(#Active) InvokedBy(#Root)
                    Name::new("Slash")
                    HitEffect
                    DamageEffect::physical("Damage@invoker * Damage@ability")),
            ],
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
            .add_systems(Update, (equip_abilities, tick_lifetimes))
            .add_systems(
                Update,
                (update_aim, auto_invoke, home_missiles, orbit_blades)
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
        explosion_mesh: meshes.add(Sphere::new(EXPLOSION_RADIUS)),
        explosion_material: materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.5, 0.1, 0.35),
            emissive: LinearRgba::new(6.0, 2.0, 0.0, 1.0),
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        pulse_mesh: meshes.add(Sphere::new(STORM_RADIUS)),
        pulse_material: materials.add(StandardMaterial {
            base_color: Color::srgba(0.5, 0.85, 1.0, 0.3),
            emissive: LinearRgba::new(1.0, 3.0, 5.0, 1.0),
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        storm_mesh: meshes.add(Cylinder::new(STORM_RADIUS, 0.1)),
        storm_material: materials.add(StandardMaterial {
            base_color: Color::srgba(0.4, 0.8, 1.0, 0.25),
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        blade_mesh: meshes.add(Cuboid::new(0.5, 0.15, 0.15)),
        blade_material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.85, 0.9, 1.0),
            emissive: LinearRgba::new(2.0, 2.0, 3.0, 1.0),
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
    registry.register(FIREBALL_PROJECTILE, || Box::new(fireball_projectile()));
    registry.register(FIREBALL_EXPLOSION, || Box::new(fireball_explosion()));
    registry.register(ICE_STORM_ZONE, || Box::new(ice_storm_zone()));
    registry.register(FROST_PULSE, || Box::new(frost_pulse()));
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

/// Circle each orbiting blade around the player on the XZ plane.
fn orbit_blades(
    time: Res<Time>,
    player: Query<&GlobalTransform, With<Player>>,
    mut blades: Query<(&mut Orbiter, &mut Transform)>,
) {
    let Ok(player_tf) = player.single() else {
        return;
    };
    let center = player_tf.translation();
    for (mut orbiter, mut transform) in &mut blades {
        orbiter.angle += BLADE_ORBIT_SPEED * time.delta_secs();
        transform.translation = center
            + Vec3::new(
                orbiter.angle.cos() * BLADE_ORBIT_RADIUS,
                0.6,
                orbiter.angle.sin() * BLADE_ORBIT_RADIUS,
            );
    }
}

/// Despawn entities whose lifetime has elapsed (transient AoE bursts).
fn tick_lifetimes(
    time: Res<Time>,
    mut lifetimes: Query<(Entity, &mut Lifetime)>,
    mut commands: Commands,
) {
    for (entity, mut lifetime) in &mut lifetimes {
        if lifetime.0.tick(time.delta()).just_finished() {
            if let Ok(mut ec) = commands.get_entity(entity) {
                ec.try_despawn();
            }
        }
    }
}
