//! Arcanist — fragile and fast; fires a volley of extra homing missiles (two
//! extra projectiles per cast via a base `ProjectileCount` bump).

use bevy::prelude::*;
use diesel_avian3d::gauge::prelude::ModifierSet;

use super::Character;
use crate::data::abilities::magic_missile;
use crate::stats::core_mod_set;

pub fn arcanist() -> Character {
    Character {
        name: "Arcanist",
        blurb: "Fragile, fast. Fires a volley of extra homing missiles.",
        starter: &magic_missile::DEF,
        tint: Color::srgb(0.6, 0.45, 1.0),
        stats,
    }
}

fn stats() -> ModifierSet {
    let mut set = core_mod_set(4.0, 6.0);
    // Two extra projectiles per volley.
    set.add("ProjectileCount", 2.0);
    set
}
