//! Ice Storm: a placed zone that pulses cold AoE for a few seconds. The ability
//! spawns the zone; the zone's repeater spawns a frost pulse each tick (a depth-2
//! spawn, `@ability` still resolves to this spell).

use bevy::prelude::*;
use bevy::scene::prelude::{bsn, Scene};
use diesel_avian3d::prelude::*;
use bevy_gauge::prelude::*;
use bevy_gearbox::prelude::*;

use super::{
    ability_base, configure_root_spawn, configure_zone_spawn, AbilityDef, AbilityStats,
    Lifetime, ProjectileAssets,
};
use crate::damage::{DamageEffect, HitEffect};
use crate::layers::TeamFilter;

const ZONE: &str = "abilities/ice_storm_zone";
const PULSE: &str = "abilities/frost_pulse";
const COOLDOWN: f32 = 4.0;
/// Base pulse radius (also the zone/pulse mesh size, see `ProjectileAssets`).
pub(crate) const STORM_RADIUS: f32 = 3.2;
const PULSE_COUNT: &str = "8";

pub static DEF: AbilityDef = AbilityDef {
    id: "ice_storm",
    name: "Ice Storm",
    base,
    region,
    root_extras: super::no_root_extras,
    stats: AbilityStats { cooldown: true, area: true, projectile_speed: false },
};

fn base() -> bevy_gauge::prelude::ModifierSet {
    ability_base(COOLDOWN, None, Some(STORM_RADIUS))
}

fn region(root: bevy::ecs::template::EntityTemplate) -> Box<dyn Scene> {
    Box::new(crate::data::items::machine::invoked_region(root, COOLDOWN, |root| {
        repeater(
            root,
            "1",
            "0.1 / AttackSpeed@invoker",
            configure_zone_spawn(ZONE),
        )
    }))
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
            #Done GoOffConfig::root() DespawnEffect,
        ]
    }
}

/// A single cold AoE tick.
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
                    template(|_| Ok(attributes! { "TargetMutator.gatherer" => "Area@ability" }))
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
