//! Frost Shard — a fast, low-cooldown cold projectile.

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::scene::prelude::{bsn, Scene};
use diesel_avian3d::prelude::*;

use super::{ability_base, configure_projectile_spawn, state, AbilityDef, AbilityStats, Homing, ProjectileAssets};
use crate::damage::{DamageEffect, HitEffect};
use crate::layers::{Layer, TeamFilter};

const PROJECTILE: &str = "abilities/frost_shard";
const SPEED: f32 = 22.0;
const COOLDOWN: f32 = 0.5;

pub static DEF: AbilityDef = AbilityDef {
    id: "frost_shard",
    name: "Frost Shard",
    scene,
    stats: AbilityStats { cooldown: true, area: false, projectile_speed: true },
};

/// Ready → Invoking (a `ProjectileCount`-long volley) → Cooldown.
pub fn scene() -> Box<dyn Scene> {
    Box::new(invoked_with(
        "Frost Shard",
        COOLDOWN,
        ability_base(COOLDOWN, Some(SPEED), None),
        |root| {
            repeater(
                root,
                "ProjectileCount@invoker",
                "0.12 / AttackSpeed@invoker",
                configure_projectile_spawn(PROJECTILE),
            )
        },
    ))
}

pub(crate) fn register_templates(registry: &mut TemplateRegistry) {
    registry.register(PROJECTILE, || Box::new(projectile()));
}

/// Small, fast homing shard dealing cold damage.
fn projectile() -> impl Scene {
    bsn! {
        #Root
            Name::new("FrostShard")
            LinearProjectileEffect { speed: SPEED, horizontal: true }
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
