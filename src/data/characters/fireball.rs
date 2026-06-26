//! Test character: starts with Fireball.

use bevy::prelude::*;

use super::Character;
use crate::data::abilities;

pub fn fireball() -> Character {
    Character {
        name: "Fireball",
        blurb: "A projectile that bursts into an AoE explosion on impact.",
        starter: &abilities::fireball::DEF,
        tint: Color::srgb(1.0, 0.35, 0.15),
        stats: super::test_stats,
    }
}
