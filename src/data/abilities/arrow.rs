//! Arrow: the Bow weapon's ability. A straight physical bolt volley (count scales
//! with `ProjectileCount`). Hit damage is drawn from the bow's own `Damage.base`
//! via `@item`, so a stronger bow makes Arrow pierce harder with no change to the
//! ability. Same cross-entity source as [`slice`], on a projectile.

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::scene::prelude::{bsn, Scene};
use diesel_avian3d::prelude::*;

use super::{ability_base, configure_projectile_spawn, state, AbilityDef, AbilityStats, ProjectileAssets};
use crate::damage::{DamageEffect, HitEffect};
use crate::layers::{Layer, TeamFilter};

pub(crate) const PROJECTILE: &str = "abilities/arrow";
const SPEED: f32 = 24.0;
const COOLDOWN: f32 = 0.9;

pub static DEF: AbilityDef = AbilityDef {
    id: "arrow",
    name: "Arrow",
    base,
    region,
    root_extras: super::no_root_extras,
    stats: AbilityStats { cooldown: true, area: false, projectile_speed: true },
};

fn base() -> diesel_avian3d::gauge::prelude::ModifierSet {
    ability_base(COOLDOWN, Some(SPEED), None)
}

/// Ready -> Invoking (a `ProjectileCount`-long volley) -> Cooldown.
fn region(root: bevy::ecs::template::EntityTemplate) -> Box<dyn Scene> {
    Box::new(crate::data::items::machine::invoked_region(root, COOLDOWN, |root| {
        repeater(
            root,
            "ProjectileCount@invoker",
            "0.12 / AttackSpeed@invoker",
            configure_projectile_spawn(PROJECTILE),
        )
    }))
}

pub(crate) fn register_templates(registry: &mut TemplateRegistry) {
    registry.register(PROJECTILE, || Box::new(projectile()));
}

/// The bolt: Flying -> Hit -> Done. Flies straight (gravity off), dies on contact
/// (physical damage from the bow's `@item`) or after 2s.
fn projectile() -> impl Scene {
    bsn! {
        #Root
            Name::new("Arrow")
            LinearProjectileEffect { speed: SPEED, horizontal: true }
            template(|_| Ok(bevy_gauge::attributes! { "Speed" => "ProjectileSpeed@ability" }))
            TeamFilter::Enemies
            CollisionLayers::new([Layer::Projectile], [Layer::Character])
            Collider::sphere(0.12)
            Visibility::Inherited
            template(|ctx| Ok(Mesh3d(ctx.resource::<ProjectileAssets>().firebolt_mesh.clone())))
            template(|ctx| Ok(MeshMaterial3d(ctx.resource::<ProjectileAssets>().firebolt_material.clone())))
            StateMachine InitialState(#Flying)
        Substates [
            #Flying Transitions [
                (Target(#Hit) MessageEdge::<CollidedEntity>::default()),
                (Target(#Done) AlwaysEdge Delay::from_secs_f32(2.0)),
            ],
            #Hit Substates [
                (SubEffectOf(#Hit) InvokedBy(#Root)
                    Name::new("Pierce")
                    HitEffect
                    DamageEffect::physical(
                        "Damage.base@item + Damage@invoker * Damage@ability")),
            ] Transitions [
                (Target(#Done) AlwaysEdge)
            ],
            #Done state(DelayedDespawn::now()),
        ]
    }
}
