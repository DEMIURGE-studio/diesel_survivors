//! Test character: starts with Firebolt.

use bevy::prelude::*;

use super::Character;
use crate::data::abilities;

pub fn firebolt() -> Character {
    Character {
        name: "Firebolt",
        blurb: "A slow, hard-hitting straight fire bolt — lead your target.",
        starter: &abilities::firebolt::DEF,
        tint: Color::srgb(1.0, 0.5, 0.2),
        stats: super::test_stats,
    }
}
