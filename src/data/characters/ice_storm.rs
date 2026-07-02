//! Test character: starts with Ice Storm.

use bevy::prelude::*;

use super::Character;
use crate::data::items;

pub fn ice_storm() -> Character {
    Character {
        name: "Ice Storm",
        blurb: "A placed zone that pulses cold AoE on the target for a few seconds.",
        starter: &items::GLACIER_STAFF,
        tint: Color::srgb(0.6, 0.8, 1.0),
        stats: super::test_stats,
    }
}
