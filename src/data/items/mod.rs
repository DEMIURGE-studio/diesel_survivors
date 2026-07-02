//! Items: the equippable unit a slot holds.
//!
//! An [`ItemDef`] bundles a weapon type (sword / bow / staff), the ability it
//! grants, its own local attributes (a sword's `Damage.base`, read by the granted
//! ability via `@item`), and a wearer passive it applies to the player while
//! equipped. The gauge/diesel showcase the project is built around:
//!
//! - Cross-entity sources: the item is a separate entity carrying its own
//!   `Attributes`; the granted ability's effects read `Damage.base@item`, so a
//!   beefier weapon raises the ability's damage with no change to the ability.
//! - Sustained modifiers: the wearer passive applies on equip and reverses on
//!   unequip/hot-swap, like [`crate::data::buffs`].
//!
//! The equip/hot-swap machinery that spawns the item entity, wires `@item`, and
//! applies the passive lives in [`crate::ability`]. Slots hold `&'static ItemDef`.

use bevy::prelude::*;
use diesel_avian3d::gauge::prelude::ModifierSet;
use diesel_avian3d::prelude::*;

use super::abilities::{self, AbilityDef, AbilityStats};
use crate::stats::attr;

pub mod machine;

// ---------------------------------------------------------------------------
// Weapon types
// ---------------------------------------------------------------------------

/// The broad category of a weapon. Drives presentation; a hook for class
/// restrictions or per-type bonuses later.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WeaponType {
    Sword,
    Bow,
    Staff,
}

impl WeaponType {
    pub fn label(self) -> &'static str {
        match self {
            WeaponType::Sword => "Sword",
            WeaponType::Bow => "Bow",
            WeaponType::Staff => "Staff",
        }
    }
}

// ---------------------------------------------------------------------------
// ItemDef: the data slots, the draft, and characters reference
// ---------------------------------------------------------------------------

/// A piece of equipment as pure data: a stable id, a display name, a weapon type,
/// the ability it grants, and two stat blocks. `local` is the item's own
/// attributes, read by its ability via `@item`; `wearer` is a passive applied to
/// the player while equipped. Each lives as a `static`; the game holds
/// `&'static ItemDef` everywhere.
pub struct ItemDef {
    pub id: &'static str,
    pub name: &'static str,
    pub weapon: WeaponType,
    pub ability: &'static AbilityDef,
    pub local: fn() -> ModifierSet,
    pub wearer: fn() -> ModifierSet,
}

impl ItemDef {
    /// Identity by id: each def is a unique static, so id equality is identity.
    pub fn same(&self, other: &ItemDef) -> bool {
        self.id == other.id
    }

    /// The rankable stats of the granted ability (the draft offers these).
    pub fn stats(&self) -> &AbilityStats {
        &self.ability.stats
    }
}

// ---------------------------------------------------------------------------
// Runtime link components (managed by the equip system in `crate::ability`)
// ---------------------------------------------------------------------------

/// Marker on a spawned item-machine root entity. The item is the ability root;
/// its attributes are read as `@ability`/`@item`.
#[derive(Component, Clone, Copy, Default)]
pub struct Item;

// ---------------------------------------------------------------------------
// Stat blocks
// ---------------------------------------------------------------------------

fn empty() -> ModifierSet {
    ModifierSet::new()
}

/// A sword's local damage: Slice reads this via `@item`.
fn sword_local() -> ModifierSet {
    mod_set! { "Damage.base" => 25.0 }
}
/// A sword keeps the wielder light on their feet.
fn sword_wearer() -> ModifierSet {
    mod_set! { attr::MOVE_SPEED => 0.5 }
}
/// A bow's local damage: Arrow reads this via `@item`.
fn bow_local() -> ModifierSet {
    mod_set! { "Damage.base" => 14.0 }
}
/// A bow trains a faster draw: quicker projectiles while wielded.
fn bow_wearer() -> ModifierSet {
    mod_set! { attr::PROJECTILE_SPEED => 0.2 }
}

// ---------------------------------------------------------------------------
// The catalog
// ---------------------------------------------------------------------------

// Weapons: the cross-entity-source showcase, their abilities read `@item`.
pub static IRON_SWORD: ItemDef = ItemDef {
    id: "iron_sword",
    name: "Iron Sword",
    weapon: WeaponType::Sword,
    ability: &abilities::slice::DEF,
    local: sword_local,
    wearer: sword_wearer,
};
pub static HUNTING_BOW: ItemDef = ItemDef {
    id: "hunting_bow",
    name: "Hunting Bow",
    weapon: WeaponType::Bow,
    ability: &abilities::arrow::DEF,
    local: bow_local,
    wearer: bow_wearer,
};
pub static WHIRLING_BLADE: ItemDef = ItemDef {
    id: "whirling_blade",
    name: "Whirling Blade",
    weapon: WeaponType::Sword,
    ability: &abilities::orbiting_blade::DEF,
    local: empty,
    wearer: empty,
};

// Staves: the spell abilities. They scale through the player's own stats; the
// item is the slottable shell that carries (and hot-swaps) the spell.
pub static APPRENTICE_STAFF: ItemDef = ItemDef {
    id: "apprentice_staff",
    name: "Apprentice Staff",
    weapon: WeaponType::Staff,
    ability: &abilities::magic_missile::DEF,
    local: empty,
    wearer: empty,
};
pub static EMBERWOOD_STAFF: ItemDef = ItemDef {
    id: "emberwood_staff",
    name: "Emberwood Staff",
    weapon: WeaponType::Staff,
    ability: &abilities::firebolt::DEF,
    local: empty,
    wearer: empty,
};
pub static FROSTPINE_STAFF: ItemDef = ItemDef {
    id: "frostpine_staff",
    name: "Frostpine Staff",
    weapon: WeaponType::Staff,
    ability: &abilities::frost_shard::DEF,
    local: empty,
    wearer: empty,
};
pub static PYRE_STAFF: ItemDef = ItemDef {
    id: "pyre_staff",
    name: "Pyre Staff",
    weapon: WeaponType::Staff,
    ability: &abilities::fireball::DEF,
    local: empty,
    wearer: empty,
};
pub static GLACIER_STAFF: ItemDef = ItemDef {
    id: "glacier_staff",
    name: "Glacier Staff",
    weapon: WeaponType::Staff,
    ability: &abilities::ice_storm::DEF,
    local: empty,
    wearer: empty,
};
pub static CINDER_STAFF: ItemDef = ItemDef {
    id: "cinder_staff",
    name: "Cinder Staff",
    weapon: WeaponType::Staff,
    ability: &abilities::firestorm::DEF,
    local: empty,
    wearer: empty,
};
pub static RUNED_STAFF: ItemDef = ItemDef {
    id: "runed_staff",
    name: "Runed Staff",
    weapon: WeaponType::Staff,
    ability: &abilities::arcane_storm::DEF,
    local: empty,
    wearer: empty,
};

/// Every item in the game, in menu/draft order.
pub const ALL: [&ItemDef; 10] = [
    &APPRENTICE_STAFF,
    &EMBERWOOD_STAFF,
    &FROSTPINE_STAFF,
    &PYRE_STAFF,
    &WHIRLING_BLADE,
    &GLACIER_STAFF,
    &CINDER_STAFF,
    &RUNED_STAFF,
    &IRON_SWORD,
    &HUNTING_BOW,
];
