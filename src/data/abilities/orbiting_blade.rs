//! Orbiting Blade — a sustained ability: the equipped entity *is* the orbiter. Its
//! `#Active` state self-transitions on `CollidedEntity`, re-firing its damage each
//! time it sweeps an enemy. The `crate::ability::orbit_blades` system moves it.

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
    scene,
    // Sustained: no cooldown/area/projectile-speed — only `Damage` ranks up.
    stats: AbilityStats { cooldown: false, area: false, projectile_speed: false },
};

pub fn scene() -> Box<dyn Scene> {
    Box::new(blade())
}

fn blade() -> impl Scene {
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
