//! Test character: starts with Arcane Storm.

use bevy::prelude::*;

use super::Character;
use crate::data::abilities;

pub fn arcane_storm() -> Character {
    Character {
        name: "Arcane Storm",
        blurb: "A storm zone that rains homing arcane bolts over the target.",
        starter: &abilities::arcane_storm::DEF,
        tint: Color::srgb(0.55, 0.5, 1.0),
        stats: super::test_stats,
    }
}
