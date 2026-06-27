//! Arcane Storm — the composability payoff. A zone is placed on the target; its
//! repeater rains waves of **Magic Missile's own homing bolt** scattered around
//! the area, each locking onto a nearby enemy. No new projectile, no new shell:
//! it reuses [`magic_missile`]'s registered projectile template and the shared
//! [`storm_zone`] (the same shell Firestorm uses). The only new code is the zone's
//! spawn leaf — proof that "Firestorm + Magic Missile = a missile storm" is a few
//! lines once the parts exist.
//!
//! `@ability` still resolves to *this* spell across the zone→missile spawn chain,
//! so the bolt deals Arcane Storm's `Damage`/`ProjectileSpeed`, not Magic Missile's.

use bevy::prelude::*;
use bevy::scene::prelude::{bsn, Scene};
use diesel_avian3d::prelude::*;

use super::{ability_base, configure_zone_spawn, magic_missile, storm_zone, AbilityDef, AbilityStats};

const ZONE: &str = "abilities/arcane_storm_zone";
const COOLDOWN: f32 = 5.0;
/// Base bolt speed (folded with the player's `ProjectileSpeed` on the root).
const SPEED: f32 = 16.0;
/// Radius the bolts scatter within, around the placed zone.
const SCATTER_RADIUS: f32 = 5.0;
/// How far each spawned bolt looks for an enemy to home toward.
const TARGET_RADIUS: f32 = 10.0;
/// Bolts per wave.
const PER_WAVE: usize = 5;
/// Waves dropped before the zone despawns.
const WAVES: &str = "4";

pub static DEF: AbilityDef = AbilityDef {
    id: "arcane_storm",
    name: "Arcane Storm",
    scene,
    stats: AbilityStats { cooldown: true, area: false, projectile_speed: true },
};

/// A single shot that places the storm zone on the target.
pub fn scene() -> Box<dyn Scene> {
    Box::new(invoked_with(
        "Arcane Storm",
        COOLDOWN,
        ability_base(COOLDOWN, Some(SPEED), None),
        |root| single_shot(root, configure_zone_spawn(ZONE)),
    ))
}

pub(crate) fn register_templates(registry: &mut TemplateRegistry) {
    // Only the zone is new — the bolt is Magic Missile's, registered by its module.
    registry.register(ZONE, || Box::new(zone()));
}

/// The shared storm shell, raining Magic Missile bolts: scatter the spawn points
/// in a circle, and hand each bolt the nearest enemy so its homing kicks in.
fn zone() -> impl Scene {
    storm_zone(
        "ArcaneStormZone",
        WAVES,
        "0.4 / AttackSpeed@invoker",
        bsn! {
            template(|_| Ok(SpawnConfig::root(magic_missile::PROJECTILE)
                .with_gatherer(AvianGatherer::Circle {
                    radius: SCATTER_RADIUS,
                    count: NumberType::Fixed(PER_WAVE),
                })
                .with_target_generator(
                    TargetGenerator::at_spawn()
                        .with_gatherer(AvianGatherer::NearestEntities(TARGET_RADIUS)),
                )))
        },
    )
}
