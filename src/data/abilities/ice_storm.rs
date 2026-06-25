//! Ice Storm — a placed zone that pulses cold AoE for a few seconds. The ability
//! spawns the zone; the zone's repeater spawns a frost pulse each tick (a depth-2
//! spawn — `@ability` still resolves to this spell).

use bevy::prelude::*;
use bevy::scene::prelude::{bsn, Scene};
use diesel_avian3d::prelude::*;

use super::{
    ability_base, configure_root_spawn, configure_zone_spawn, state, AbilityDef, AbilityStats,
    Lifetime, ProjectileAssets,
};
use crate::damage::{DamageEffect, HitEffect};
use crate::layers::TeamFilter;

const ZONE: &str = "abilities/ice_storm_zone";
const PULSE: &str = "abilities/frost_pulse";
const COOLDOWN: f32 = 4.0;
/// Base pulse radius (also the zone/pulse mesh size — see `ProjectileAssets`).
pub(crate) const STORM_RADIUS: f32 = 3.2;
const PULSE_COUNT: &str = "8";

pub static DEF: AbilityDef = AbilityDef {
    id: "ice_storm",
    name: "Ice Storm",
    scene,
    stats: AbilityStats { cooldown: true, area: true, projectile_speed: false },
};

pub fn scene() -> Box<dyn Scene> {
    Box::new(invoked_with(
        "Ice Storm",
        COOLDOWN,
        ability_base(COOLDOWN, None, Some(STORM_RADIUS)),
        |root| {
            repeater(
                root,
                "1",
                "0.1 / AttackSpeed@invoker",
                configure_zone_spawn(ZONE),
            )
        },
    ))
}

pub(crate) fn register_templates(registry: &mut TemplateRegistry) {
    registry.register(ZONE, || Box::new(zone()));
    registry.register(PULSE, || Box::new(pulse()));
}

/// The zone: a repeater spawns a frost pulse at its position every tick, then it
/// despawns when the volley is done.
fn zone() -> impl Scene {
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
                    #Root, PULSE_COUNT, "0.5 / AttackSpeed@invoker",
                    configure_root_spawn(PULSE),
                ),
            ],
            #Done state(DelayedDespawn::now()),
        ]
    }
}

/// A single cold AoE tick (like the explosion, but chilling).
fn pulse() -> impl Scene {
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
