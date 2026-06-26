//! Test character: starts with Frost Shard.

use bevy::prelude::*;

use super::Character;
use crate::data::abilities;

pub fn frost_shard() -> Character {
    Character {
        name: "Frost Shard",
        blurb: "Fast, low-cooldown homing darts that snap to the target.",
        starter: &abilities::frost_shard::DEF,
        tint: Color::srgb(0.5, 0.85, 1.0),
        stats: super::test_stats,
    }
}
