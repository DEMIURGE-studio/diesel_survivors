//! Test character: starts with Fireball.

use bevy::prelude::*;

use super::Character;
use crate::data::items;

pub fn fireball() -> Character {
    Character {
        name: "Fireball",
        blurb: "A projectile that bursts into an AoE explosion on impact.",
        starter: &items::PYRE_STAFF,
        tint: Color::srgb(1.0, 0.35, 0.15),
        stats: super::test_stats,
    }
}
