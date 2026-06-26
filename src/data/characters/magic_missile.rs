//! Test character: starts with Magic Missile.

use bevy::prelude::*;

use super::Character;
use crate::data::abilities;

pub fn magic_missile() -> Character {
    Character {
        name: "Magic Missile",
        blurb: "Homing arcane bolts that fan out and curve in.",
        starter: &abilities::magic_missile::DEF,
        tint: Color::srgb(0.6, 0.45, 1.0),
        stats: super::test_stats,
    }
}
