//! Slice: the Sword weapon's ability. A short-cooldown melee sweep: each invoke
//! gathers every enemy in an arc around the wielder and deals physical damage
//! drawn from the *weapon's* own `Damage.base` (read cross-entity via `@item`).
//! Swapping to a heavier sword raises Slice's damage with no change to the ability.
//! The sweep radius scales with the ability's `Area`, so Area rank-ups widen the
//! arc.

use bevy::prelude::*;
use bevy::scene::prelude::{bsn, Scene};
use diesel_avian3d::prelude::*;

use super::{ability_base, AbilityDef, AbilityStats};
use crate::damage::{DamageEffect, HitEffect};
use crate::layers::TeamFilter;

const COOLDOWN: f32 = 0.7;
/// Base arc radius (also the `AreaBase`, so Area rank-ups widen the sweep).
const ARC_RADIUS: f32 = 2.6;

pub static DEF: AbilityDef = AbilityDef {
    id: "slice",
    name: "Slice",
    base,
    region,
    root_extras: super::no_root_extras,
    stats: AbilityStats { cooldown: true, area: true, projectile_speed: false },
};

fn base() -> diesel_avian3d::gauge::prelude::ModifierSet {
    ability_base(COOLDOWN, None, Some(ARC_RADIUS))
}

/// Ready -> Invoking (one terminal sweep) -> Cooldown.
fn region(root: bevy::ecs::template::EntityTemplate) -> Box<dyn Scene> {
    Box::new(crate::data::items::machine::invoked_region(root, COOLDOWN, |root| {
        bsn! {
                #Fire InvokedBy(root) TerminalState TeamFilter::Enemies
                    GoOffConfig::default()
                Substates [
                    // Gather every enemy in the arc around the wielder; radius is
                    // gauge-driven off `Area@ability` so it tracks Area rank-ups.
                    #Sweep SubEffectOf(#Fire) InvokedBy(#Fire)
                        template(|_| Ok(TargetMutator::invoker()
                            .with_gatherer(AvianGatherer::AllEntitiesInRadius(ARC_RADIUS))))
                        template(|_| Ok(bevy_gauge::attributes! {
                            "TargetMutator.gatherer" => "Area@ability"
                        }))
                    Substates [
                        (SubEffectOf(#Sweep) InvokedBy(#Fire)
                            Name::new("Slash")
                            HitEffect
                            DamageEffect::physical(
                                "Damage.base@item + Damage@invoker * Damage@ability")),
                    ],
                ]
            }
        }))
}
