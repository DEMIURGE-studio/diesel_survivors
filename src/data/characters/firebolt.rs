//! Test character: starts with Firebolt.

use bevy::prelude::*;

use super::Character;
use crate::data::items;

pub fn firebolt() -> Character {
    Character {
        name: "Firebolt",
        blurb: "A slow, hard-hitting straight fire bolt — lead your target.",
        starter: &items::EMBERWOOD_STAFF,
        tint: Color::srgb(1.0, 0.5, 0.2),
        stats: super::test_stats,
    }
}
