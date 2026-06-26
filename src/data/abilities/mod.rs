//! Ability data. Each ability is a self-contained module — its BSN scene(s),
//! its tuning consts, and a `static AbilityDef` describing it — and the game
//! references abilities purely through `&'static AbilityDef` (no enum). The slot
//! system, the level-up draft, and character starters all key off these defs.
//!
//! An ability is an `invoked` state-machine shell (Ready → Invoking → Cooldown)
//! whose Invoking phase runs a `repeater` volley; the spawned projectile/zone is
//! itself a small state chart. See any module here for the pattern.

use bevy::prelude::*;
use bevy::scene::prelude::{bsn, Scene};
use diesel_avian3d::gauge::prelude::ModifierSet;
use diesel_avian3d::prelude::*;
use diesel_avian3d::DirectionOffset;

pub mod fireball;
pub mod firebolt;
pub mod firestorm;
pub mod frost_shard;
pub mod ice_storm;
pub mod magic_missile;
pub mod orbiting_blade;

// ---------------------------------------------------------------------------
// AbilityDef — the data the rest of the game references
// ---------------------------------------------------------------------------

/// Which per-ability stats an ability exposes for level-up rank-ups. `Damage` is
/// always rankable; the rest depend on what the ability's scene actually carries
/// (only AoE abilities have `AreaBase`, only projectile abilities have
/// `ProjectileSpeedBase`, only `invoked` abilities have `CooldownBase`).
pub struct AbilityStats {
    pub cooldown: bool,
    pub area: bool,
    pub projectile_speed: bool,
}

/// A playable ability as pure data: a stable id, a display name, a factory for a
/// fresh BSN scene, and its rankable stats. Each lives as a `static` in its
/// module; the game holds `&'static AbilityDef` everywhere.
pub struct AbilityDef {
    pub id: &'static str,
    pub name: &'static str,
    pub scene: fn() -> Box<dyn Scene>,
    pub stats: AbilityStats,
}

impl AbilityDef {
    /// Identity by id — each def is a unique static, so id equality is identity.
    pub fn same(&self, other: &AbilityDef) -> bool {
        self.id == other.id
    }
}

/// Every ability in the game, in menu/draft order.
pub const ALL: [&AbilityDef; 7] = [
    &magic_missile::DEF,
    &firebolt::DEF,
    &frost_shard::DEF,
    &fireball::DEF,
    &orbiting_blade::DEF,
    &ice_storm::DEF,
    &firestorm::DEF,
];

// ---------------------------------------------------------------------------
// Shared scene helpers
// ---------------------------------------------------------------------------

/// Single-component scene inserting `StateComponent(value)` (the canonical
/// `template(…)` form, since `StateComponent`/its payloads aren't `Default`).
pub(crate) fn state<T: Component + Clone>(value: T) -> impl Scene {
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
pub(crate) fn ability_base(
    cooldown: f32,
    projectile_speed: Option<f32>,
    area: Option<f32>,
) -> ModifierSet {
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

/// Firing leaf: spawn a projectile template at the invoker (at enemy mid-height),
/// aimed at whatever the invoker is targeting.
pub(crate) fn configure_projectile_spawn(template_id: &'static str) -> impl Scene {
    bsn! {
        SpawnConfig::invoker_offset_target(
            template_id,
            Vec3Offset::Fixed(DirectionOffset::new(Dir3::Y, 0.0)),
            TargetGenerator::at_invoker_target(),
        )
    }
}

/// Firing leaf that spawns a template at the aimed position (not the invoker).
pub(crate) fn configure_zone_spawn(template_id: &'static str) -> impl Scene {
    bsn! { SpawnConfig::target(template_id) }
}

/// Firing leaf that spawns a template at the spawner's own root position.
pub(crate) fn configure_root_spawn(template_id: &'static str) -> impl Scene {
    bsn! { SpawnConfig::root(template_id) }
}

// ---------------------------------------------------------------------------
// Runtime components authored in ability scenes (queried by `crate::ability`)
// ---------------------------------------------------------------------------

/// Marks a projectile that steers toward its target entity each frame. Projectiles
/// launch out to the side and curve in at `turn_rate` radians/sec, so the homing
/// is visible (and each ability gives it a distinct feel).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Homing {
    pub turn_rate: f32,
}

/// A blade that circles its invoker. The orbit system advances `angle`.
#[derive(Component, Default, Clone)]
pub struct Orbiter {
    pub angle: f32,
}

/// Despawns its entity once the timer finishes. Used by transient AoE bursts.
#[derive(Component)]
pub struct Lifetime(pub Timer);

impl Lifetime {
    pub(crate) fn secs(secs: f32) -> Self {
        Self(Timer::from_seconds(secs, TimerMode::Once))
    }
}

// ---------------------------------------------------------------------------
// Cached visual handles for hot-spawned projectiles
// ---------------------------------------------------------------------------

#[derive(Resource)]
pub struct ProjectileAssets {
    pub missile_mesh: Handle<Mesh>,
    pub missile_material: Handle<StandardMaterial>,
    pub firebolt_mesh: Handle<Mesh>,
    pub firebolt_material: Handle<StandardMaterial>,
    pub frost_mesh: Handle<Mesh>,
    pub frost_material: Handle<StandardMaterial>,
    pub explosion_mesh: Handle<Mesh>,
    pub explosion_material: Handle<StandardMaterial>,
    pub pulse_mesh: Handle<Mesh>,
    pub pulse_material: Handle<StandardMaterial>,
    pub storm_mesh: Handle<Mesh>,
    pub storm_material: Handle<StandardMaterial>,
    pub blade_mesh: Handle<Mesh>,
    pub blade_material: Handle<StandardMaterial>,
    pub meteor_mesh: Handle<Mesh>,
    pub meteor_material: Handle<StandardMaterial>,
}

/// Build the cached projectile/AoE visual handles once at startup.
pub fn setup_projectile_assets(
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
        explosion_mesh: meshes.add(Sphere::new(fireball::EXPLOSION_RADIUS)),
        explosion_material: materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.5, 0.1, 0.35),
            emissive: LinearRgba::new(6.0, 2.0, 0.0, 1.0),
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        pulse_mesh: meshes.add(Sphere::new(ice_storm::STORM_RADIUS)),
        pulse_material: materials.add(StandardMaterial {
            base_color: Color::srgba(0.5, 0.85, 1.0, 0.3),
            emissive: LinearRgba::new(1.0, 3.0, 5.0, 1.0),
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        storm_mesh: meshes.add(Cylinder::new(ice_storm::STORM_RADIUS, 0.1)),
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
        meteor_mesh: meshes.add(Sphere::new(firestorm::METEOR_RADIUS)),
        meteor_material: materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.35, 0.05, 0.4),
            emissive: LinearRgba::new(8.0, 2.5, 0.0, 1.0),
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
    });
}

/// Register every ability's spawned projectile/zone templates with the diesel
/// runtime registry. Each module registers its own.
pub fn register_projectiles(mut registry: ResMut<TemplateRegistry>) {
    magic_missile::register_templates(&mut registry);
    firebolt::register_templates(&mut registry);
    frost_shard::register_templates(&mut registry);
    fireball::register_templates(&mut registry);
    ice_storm::register_templates(&mut registry);
    firestorm::register_templates(&mut registry);
    // orbiting_blade spawns nothing — it *is* the persistent entity.
}
