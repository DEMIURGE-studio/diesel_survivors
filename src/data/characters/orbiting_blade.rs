//! Test character: starts with Orbiting Blade.

use bevy::prelude::*;

use super::Character;
use crate::data::items;

pub fn orbiting_blade() -> Character {
    Character {
        name: "Orbiting Blade",
        blurb: "A sustained blade that circles you, slashing what it sweeps.",
        starter: &items::WHIRLING_BLADE,
        tint: Color::srgb(0.8, 0.85, 0.95),
        stats: super::test_stats,
    }
}
