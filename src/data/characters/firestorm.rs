//! Test character: starts with Firestorm.

use bevy::prelude::*;

use super::Character;
use crate::data::items;

pub fn firestorm() -> Character {
    Character {
        name: "Firestorm",
        blurb: "A meteor shower that rains falling explosions over the target.",
        starter: &items::CINDER_STAFF,
        tint: Color::srgb(1.0, 0.45, 0.1),
        stats: super::test_stats,
    }
}
