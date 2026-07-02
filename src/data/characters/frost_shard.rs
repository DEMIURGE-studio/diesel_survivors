//! Test character: starts with Frost Shard.

use bevy::prelude::*;

use super::Character;
use crate::data::items;

pub fn frost_shard() -> Character {
    Character {
        name: "Frost Shard",
        blurb: "Fast, low-cooldown homing darts that snap to the target.",
        starter: &items::FROSTPINE_STAFF,
        tint: Color::srgb(0.5, 0.85, 1.0),
        stats: super::test_stats,
    }
}
