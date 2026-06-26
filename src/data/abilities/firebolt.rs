//! Firebolt — Sentinel's starter. A slow, hard-hitting *straight* fire bolt (no
//! homing — a committed skill-shot) whose `Damage` (for the Sentinel) scales with
//! MaxHealth.

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::scene::prelude::{bsn, Scene};
use diesel_avian3d::prelude::*;

use super::{ability_base, configure_projectile_spawn, state, AbilityDef, AbilityStats, ProjectileAssets};
use crate::damage::{DamageEffect, HitEffect};
use crate::layers::{Layer, TeamFilter};

const PROJECTILE: &str = "abilities/firebolt";
const SPEED: f32 = 13.0;
const COOLDOWN: f32 = 1.1;

pub static DEF: AbilityDef = AbilityDef {
    id: "firebolt",
    name: "Firebolt",
    scene,
    stats: AbilityStats { cooldown: true, area: false, projectile_speed: true },
};

/// Ready → Invoking (a `ProjectileCount`-long volley) → Cooldown.
pub fn scene() -> Box<dyn Scene> {
    Box::new(invoked_with(
        "Firebolt",
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

/// Bigger, slower homing bolt dealing fire damage.
fn projectile() -> impl Scene {
    bsn! {
        #Root
            Name::new("Firebolt")
            LinearProjectileEffect { speed: SPEED, horizontal: true }
            template(|_| Ok(bevy_gauge::attributes! { "Speed" => "ProjectileSpeed@ability" }))
            // No `Homing`: a committed straight shot, aimed where the target was
            // at fire time — the heavy-hitter's tradeoff vs. the homing abilities.
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
