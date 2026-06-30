//! Orbiting Blade: a sustained ability where the equipped entity *is* the orbiter.
//! Its `#Active` state self-transitions on `CollidedEntity`, re-firing its damage
//! each time it sweeps an enemy. The `crate::ability::orbit_blades` system moves it.

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::scene::prelude::{bsn, Scene};
use diesel_avian3d::prelude::*;

use super::{AbilityDef, AbilityStats, Orbiter, ProjectileAssets};
use crate::damage::{DamageEffect, HitEffect};
use crate::layers::{Layer, TeamFilter};

pub static DEF: AbilityDef = AbilityDef {
    id: "orbiting_blade",
    name: "Orbiting Blade",
    base,
    region,
    root_extras,
    // Sustained: no cooldown/area/projectile-speed; only `Damage` ranks up.
    stats: AbilityStats { cooldown: false, area: false, projectile_speed: false },
};

/// No extra base attributes. The item builder seeds the `Damage` multiplier
/// (1.0) every ability shares, the blade's only rank-up.
fn base() -> diesel_avian3d::gauge::prelude::ModifierSet {
    diesel_avian3d::gauge::prelude::ModifierSet::new()
}

/// The blade is a *persistent* weapon: its visuals, collider, and `Orbiter` live
/// on the item root (the `crate::ability::orbit_blades` system moves it). Parked
/// (`Visibility::Hidden` + `ColliderDisabled` from the item builder) while stored,
/// revealed by `on_equipped` while in the `Equipped` zone.
fn root_extras() -> Box<dyn Scene> {
    Box::new(bsn! {
        Orbiter
        TeamFilter::Enemies
        Sensor
        CollisionEventsEnabled
        template(|_| Ok(RigidBody::Kinematic))
        Collider::sphere(0.35)
        CollisionLayers::new([Layer::Projectile], [Layer::Character])
        template(|ctx| Ok(Mesh3d(ctx.resource::<ProjectileAssets>().blade_mesh.clone())))
        template(|ctx| Ok(MeshMaterial3d(ctx.resource::<ProjectileAssets>().blade_material.clone())))
        Transform::default()
    })
}

/// The blade's behaviour region, merged onto the item's `Equipped` state: a single
/// `Active` state that self-transitions on `CollidedEntity`, re-firing its slash
/// each sweep. Active only while equipped; benched, it neither orbits nor hits.
fn region(root: bevy::ecs::template::EntityTemplate) -> Box<dyn Scene> {
    Box::new(bsn! {
        InitialState(#Active)
        Substates [
            #Active Name::new("Active") Transitions [
                (Target(#Active) MessageEdge::<CollidedEntity>::default())
            ] Substates [
                (SubEffectOf(#Active) InvokedBy(root)
                    Name::new("Slash")
                    HitEffect
                    DamageEffect::physical("Damage@invoker * Damage@ability")),
            ],
        ]
    })
}
