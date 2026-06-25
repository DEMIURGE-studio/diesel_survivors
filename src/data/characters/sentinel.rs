//! Sentinel — tanky and slow; its Firebolts hit harder the more max health it has
//! (the gauge showcase: `Damage` derived from `MaxHealth`).

use bevy::prelude::*;
use diesel_avian3d::gauge::prelude::ModifierSet;

use super::Character;
use crate::data::abilities::firebolt;
use crate::stats::core_mod_set;

pub fn sentinel() -> Character {
    Character {
        name: "Sentinel",
        blurb: "Tanky, slow. Firebolts hit harder the more max health you have.",
        starter: &firebolt::DEF,
        tint: Color::srgb(0.9, 0.5, 0.3),
        stats,
    }
}

fn stats() -> ModifierSet {
    let mut set = core_mod_set(12.0, 4.0);
    // Bonus damage proportional to max health.
    set.add_expr("Damage", "MaxHealth * 0.1");
    set
}
