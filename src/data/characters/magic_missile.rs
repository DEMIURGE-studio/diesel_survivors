//! Test character: starts with Magic Missile.

use bevy::prelude::*;

use super::Character;
use crate::data::items;

pub fn magic_missile() -> Character {
    Character {
        name: "Magic Missile",
        blurb: "Homing arcane bolts that fan out and curve in.",
        starter: &items::APPRENTICE_STAFF,
        tint: Color::srgb(0.6, 0.45, 1.0),
        stats: super::test_stats,
    }
}
