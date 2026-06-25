//! Playable characters. Each is a small data definition plus a gauge scaling
//! rule and a starter ability — the same machinery, two very different feels,
//! which is the point of the gauge showcase.

use bevy::prelude::*;
use diesel_avian3d::gauge::prelude::ModifierSet;

use crate::ability::AbilityId;
use crate::stats::core_mod_set;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharacterId {
    Sentinel,
    Arcanist,
}

pub struct CharacterDef {
    pub id: CharacterId,
    pub name: &'static str,
    pub blurb: &'static str,
    pub vitality: f32,
    pub move_speed: f32,
}

pub const CHARACTERS: [CharacterDef; 2] = [
    CharacterDef {
        id: CharacterId::Sentinel,
        name: "Sentinel",
        blurb: "Tanky, slow. Firebolts hit harder the more max health you have.",
        vitality: 12.0,
        move_speed: 4.0,
    },
    CharacterDef {
        id: CharacterId::Arcanist,
        name: "Arcanist",
        blurb: "Fragile, fast. Fires a volley of extra homing missiles.",
        vitality: 4.0,
        move_speed: 6.0,
    },
];

/// The character chosen on the select screen, read when spawning the player.
#[derive(Resource, Clone, Copy)]
pub struct SelectedCharacter(pub CharacterId);

impl Default for SelectedCharacter {
    fn default() -> Self {
        Self(CharacterId::Arcanist)
    }
}

impl CharacterId {
    pub fn def(self) -> &'static CharacterDef {
        CHARACTERS.iter().find(|c| c.id == self).expect("known character")
    }

    /// The shared baseline plus this character's scaling, as a raw modifier set
    /// so callers can fold in metaprogression before wrapping it.
    pub fn mod_set(self) -> ModifierSet {
        let def = self.def();
        let mut set: ModifierSet = core_mod_set(def.vitality, def.move_speed);
        match self {
            // Sentinel: bonus damage proportional to max health.
            CharacterId::Sentinel => set.add_expr("Damage", "MaxHealth * 0.1"),
            // Arcanist: two extra projectiles per volley.
            CharacterId::Arcanist => set.add("ProjectileCount", 2.0),
        }
        set
    }

    /// The character's starter ability (occupies slot 0).
    pub fn starter_ability(self) -> AbilityId {
        match self {
            CharacterId::Sentinel => AbilityId::Firebolt,
            CharacterId::Arcanist => AbilityId::MagicMissile,
        }
    }
}
