//! The attribute schema, the spine of the gauge showcase.
//!
//! Gauge has no global attribute registry: derived expressions like
//! `MaxHealth = Vitality * 10 + 50` are authored *per entity* as modifiers in
//! the spawn scene. The "schema" is therefore a shared convention living here:
//!
//! - [`attr`]: canonical attribute names, used both for Rust-side reads
//!   (`attributes.value(e, attr::MAX_HEALTH)`) and directly as the keys in the
//!   `mod_set!`/`attributes!` blocks below (the macros accept any `&str` path,
//!   not just string literals), so a rename flows from one place.
//! - [`core_stats`]: the default block every character/enemy composes in, so the
//!   relationships (Vitality drives MaxHealth, etc.) are defined once.
//!
//! ## Schema
//!
//! | Attribute         | Meaning                                  |
//! |-------------------|------------------------------------------|
//! | `Vitality`        | base stat; drives `MaxHealth`            |
//! | `MaxHealth`       | `Vitality * 10` (derived)                |
//! | `MoveSpeed`       | world units / sec                        |
//! | `Damage`          | base ability damage (tagged downstream)  |
//! | `AttackSpeed`     | fire-rate multiplier (1.0 = nominal)     |
//! | `CooldownMult`    | ability cooldown scale (lower = faster)  |
//! | `Area`            | AoE / projectile size scale              |
//! | `ProjectileCount` | extra projectiles per cast               |
//! | `ProjectileSpeed` | projectile travel-speed scale (1.0 = nominal) |
//! | `CritChance`      | 0..1 chance to crit                      |
//! | `CritMult`        | crit damage multiplier                   |
//! | `PickupRadius`    | XP / item attraction radius              |
//! | `Resistance`      | tagged 0..1 damage reduction per element |
//!
//! `Resistance` is a *tagged* attribute: each modifier carries a [`DamageTags`]
//! element mask, and the damage pipeline reads only the slice matching the hit's
//! element (`evaluate_tagged("Resistance", FIRE)`). One attribute holds every
//! element's resistance at once, the gauge tag showcase. A bare `Resistance`
//! value resists nothing until it carries a tagged modifier.
//!
//! Current health lives on the [`Health`] component (not an attribute),
//! initialized from `MaxHealth` and written back so expressions can reference it.

use bevy_gauge::prelude::{AttributeInitializer, ModifierSet};
use bevy_gauge::prelude::*;

use crate::damage::DamageTags;

/// Canonical attribute names. Used both for Rust-side reads and as the keys in
/// [`core_mod_set`] / [`enemy_stats`], so each name has a single source of truth.
pub mod attr {
    pub const VITALITY: &str = "Vitality";
    pub const MAX_HEALTH: &str = "MaxHealth";
    pub const MOVE_SPEED: &str = "MoveSpeed";
    pub const DAMAGE: &str = "Damage";
    pub const ATTACK_SPEED: &str = "AttackSpeed";
    pub const COOLDOWN_MULT: &str = "CooldownMult";
    pub const AREA: &str = "Area";
    pub const PROJECTILE_COUNT: &str = "ProjectileCount";
    pub const PROJECTILE_SPEED: &str = "ProjectileSpeed";
    pub const CRIT_CHANCE: &str = "CritChance";
    pub const CRIT_MULT: &str = "CritMult";
    pub const PICKUP_RADIUS: &str = "PickupRadius";
    pub const RESISTANCE: &str = "Resistance";
}

/// The default block as a raw [`ModifierSet`], so character and enemy definitions
/// can extend it with their own scaling before wrapping it in an initializer.
///
/// `MaxHealth` is expressed in terms of `Vitality` so changing Vitality (from a
/// level-up, an item, or a character's scaling) recomputes health automatically:
/// the core gauge demonstration.
pub fn core_mod_set(vitality: f32, move_speed: f32) -> ModifierSet {
    mod_set! {
        attr::VITALITY => vitality,
        attr::MAX_HEALTH => "Vitality * 10.0",
        attr::MOVE_SPEED => move_speed,
        attr::DAMAGE => 10.0,
        attr::ATTACK_SPEED => 1.0,
        attr::COOLDOWN_MULT => 1.0,
        attr::AREA => 1.0,
        attr::PROJECTILE_COUNT => 1.0,
        // 1.0 = nominal; abilities multiply their base speed by this scale.
        attr::PROJECTILE_SPEED => 1.0,
        attr::CRIT_CHANCE => 0.05,
        attr::CRIT_MULT => 2.0,
        attr::PICKUP_RADIUS => 2.5,
    }
}

/// Enemy stat block: the core stats plus a sample **physical** resistance, so the
/// damage-type pipeline is demonstrable in play. Physical hits (e.g. Orbiting
/// Blade) are partly shrugged off while elemental abilities land in full: the
/// same `Resistance` attribute, sliced by the hit's element tag. Tune or add
/// elements (`[DamageTags::FIRE] => 0.5`) per enemy archetype later.
pub fn enemy_stats(vitality: f32, move_speed: f32) -> AttributeInitializer {
    let mut set = core_mod_set(vitality, move_speed);
    set.add_tagged(attr::RESISTANCE, 0.30, DamageTags::PHYSICAL);
    AttributeInitializer::new(set)
}
