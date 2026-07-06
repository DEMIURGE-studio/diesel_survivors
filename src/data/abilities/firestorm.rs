//! Firestorm: a long-cooldown meteor shower. A zone is placed high *above* the
//! aimed target; its repeater drops several waves of falling explosive projectiles
//! in a circle, each bursting into a fire AoE where it lands (on an enemy or the
//! ground). A barrage that rains down over an area.

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::scene::prelude::{bsn, Scene};
use diesel_avian3d::prelude::*;
use bevy_diesel::gauge::prelude::*;
use bevy_diesel::gearbox::prelude::*;
use diesel_avian3d::DirectionOffset;

use super::{ability_base, storm_zone, AbilityDef, AbilityStats, Lifetime, ProjectileAssets};
use crate::damage::{DamageEffect, HitEffect};
use crate::layers::{Layer, TeamFilter};

const ZONE: &str = "abilities/firestorm_zone";
const METEOR: &str = "abilities/firestorm_meteor";
const BURST: &str = "abilities/firestorm_burst";

const COOLDOWN: f32 = 6.0;
/// Radius of each meteor's burst (also its mesh size, see `ProjectileAssets`).
pub(crate) const METEOR_RADIUS: f32 = 1.8;
/// How high above the target the zone is placed (meteors fall from here).
const ZONE_HEIGHT: f32 = 9.0;
/// Radius of the circle the meteors rain down within.
const RAIN_RADIUS: f32 = 4.5;
/// Meteors per wave.
const PER_WAVE: usize = 8;
/// Number of waves dropped before the zone despawns.
const WAVES: &str = "3";

pub static DEF: AbilityDef = AbilityDef {
    id: "firestorm",
    name: "Firestorm",
    base,
    region,
    root_extras: super::no_root_extras,
    stats: AbilityStats { cooldown: true, area: true, projectile_speed: false },
};

fn base() -> bevy_diesel::gauge::prelude::ModifierSet {
    ability_base(COOLDOWN, None, Some(METEOR_RADIUS))
}

/// A single shot that places the firestorm zone above the target.
fn region(root: bevy::ecs::template::EntityTemplate) -> Box<dyn Scene> {
    Box::new(crate::data::items::machine::invoked_region(root, COOLDOWN, |root| {
        single_shot(
            root,
            bsn! {
                template(|_| Ok(SpawnConfig::target(ZONE)
                    .with_offset(Vec3Offset::Fixed(DirectionOffset::new(Dir3::Y, ZONE_HEIGHT)))))
            },
        )
    }))
}

pub(crate) fn register_templates(registry: &mut TemplateRegistry) {
    registry.register(ZONE, || Box::new(zone()));
    registry.register(METEOR, || Box::new(meteor()));
    registry.register(BURST, || Box::new(burst()));
}

/// The zone (invisible, high above the target): the shared [`storm_zone`] shell,
/// dropping `WAVES` waves of `PER_WAVE` meteors scattered in a circle around it.
/// Each meteor falls and bursts where it lands.
fn zone() -> impl Scene {
    storm_zone(
        "FirestormZone",
        WAVES,
        "0.5 / AttackSpeed@invoker",
        bsn! {
            template(|_| Ok(SpawnConfig::root(METEOR).with_gatherer(
                AvianGatherer::Circle { radius: RAIN_RADIUS, count: NumberType::Fixed(PER_WAVE) },
            )))
        },
    )
}

/// A falling explosive projectile: spawned with no target, it drops under gravity
/// and, on collision (an enemy or the ground), spawns a fire burst at the impact
/// point and despawns.
fn meteor() -> impl Scene {
    bsn! {
        #Root
            Name::new("FirestormMeteor")
            ProjectileEffect::new(12.0)
            TeamFilter::Enemies
            CollisionLayers::new([Layer::Projectile], [Layer::Terrain, Layer::Character])
            Collider::sphere(0.3)
            Visibility::Inherited
            template(|ctx| Ok(Mesh3d(ctx.resource::<ProjectileAssets>().firebolt_mesh.clone())))
            template(|ctx| Ok(MeshMaterial3d(ctx.resource::<ProjectileAssets>().firebolt_material.clone())))
            StateMachine InitialState(#Flying)
        Substates [
            #Flying Transitions [
                (Target(#Hit) MessageEdge::<CollidedEntity>::default()),
                // Safety net: despawn if it never collides.
                (Target(#Done) AlwaysEdge Delay::from_secs_f32(4.0)),
            ],
            #Hit Substates [
                (SubEffectOf(#Hit) InvokedBy(#Root)
                    Name::new("SpawnBurst")
                    SpawnConfig::passed(BURST)),
            ] Transitions [
                (Target(#Done) AlwaysEdge)
            ],
            #Done GoOffConfig::root() DespawnEffect,
        ]
    }
}

/// Fire burst: gathers everything in radius and scorches it, then fades.
fn burst() -> impl Scene {
    bsn! {
        #Root
            Name::new("FirestormBurst")
            TeamFilter::Enemies
            Visibility::Inherited
            template(|_| Ok(Lifetime::secs(0.3)))
            template(|ctx| Ok(Mesh3d(ctx.resource::<ProjectileAssets>().meteor_mesh.clone())))
            template(|ctx| Ok(MeshMaterial3d(ctx.resource::<ProjectileAssets>().meteor_material.clone())))
            StateMachine InitialState(#Active)
        Substates [
            #Active GoOffConfig::default()
            Substates [
                #AoE SubEffectOf(#Active) InvokedBy(#Root)
                    TargetMutator::root_gathering(AvianGatherer::AllEntitiesInRadius(METEOR_RADIUS))
                    template(|_| Ok(attributes! { "TargetMutator.gatherer" => "Area@ability" }))
                Substates [
                    (SubEffectOf(#AoE) InvokedBy(#Root)
                        Name::new("Scorch")
                        HitEffect
                        DamageEffect::fire("Damage@invoker * Damage@ability * 0.5")),
                ],
            ],
        ]
    }
}
