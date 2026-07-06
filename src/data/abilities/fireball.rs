//! Fireball: a slow projectile that bursts into an AoE explosion on impact. The
//! projectile deals no direct damage; the explosion gathers everything in radius.

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::scene::prelude::{bsn, Scene};
use diesel_avian3d::prelude::*;
use bevy_diesel::gauge::prelude::*;
use bevy_diesel::gearbox::prelude::*;

use super::{ability_base, configure_projectile_spawn, AbilityDef, AbilityStats, Homing, Lifetime, ProjectileAssets};
use crate::damage::{DamageEffect, HitEffect};
use crate::layers::{Layer, TeamFilter};

const PROJECTILE: &str = "abilities/fireball_projectile";
const EXPLOSION: &str = "abilities/fireball_explosion";
const SPEED: f32 = 12.0;
const COOLDOWN: f32 = 1.3;
/// Base explosion radius (also the explosion mesh size, see `ProjectileAssets`).
pub(crate) const EXPLOSION_RADIUS: f32 = 3.0;

pub static DEF: AbilityDef = AbilityDef {
    id: "fireball",
    name: "Fireball",
    base,
    region,
    root_extras: super::no_root_extras,
    stats: AbilityStats { cooldown: true, area: true, projectile_speed: true },
};

fn base() -> bevy_diesel::gauge::prelude::ModifierSet {
    ability_base(COOLDOWN, Some(SPEED), Some(EXPLOSION_RADIUS))
}

fn region(root: bevy::ecs::template::EntityTemplate) -> Box<dyn Scene> {
    Box::new(crate::data::items::machine::invoked_region(root, COOLDOWN, |root| {
        repeater(
            root,
            "ProjectileCount@invoker",
            "0.12 / AttackSpeed@invoker",
            configure_projectile_spawn("abilities/fireball_projectile"),
        )
    }))
}

pub(crate) fn register_templates(registry: &mut TemplateRegistry) {
    registry.register(PROJECTILE, || Box::new(projectile()));
    registry.register(EXPLOSION, || Box::new(explosion()));
}

/// Slow fire projectile; on hit it spawns an explosion at the impact point and
/// deals no direct damage.
fn projectile() -> impl Scene {
    bsn! {
        #Root
            Name::new("Fireball")
            LinearProjectileEffect { speed: SPEED, horizontal: true }
            template(|_| Ok(attributes! { "Speed" => "ProjectileSpeed@ability" }))
            Homing { turn_rate: 5.0 }
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
                    SpawnConfig::passed(EXPLOSION)),
            ] Transitions [
                (Target(#Done) AlwaysEdge)
            ],
            #Done GoOffConfig::root() DespawnEffect,
        ]
    }
}

/// One-shot AoE: on entry, gathers every entity in radius and burns it, then
/// fades after a short lifetime.
fn explosion() -> impl Scene {
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
                    // resolves against `"TargetMutator.gatherer"`, aliased to the
                    // spell's `Area` so an upgrade scales every explosion.
                    template(|_| Ok(attributes! { "TargetMutator.gatherer" => "Area@ability" }))
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
