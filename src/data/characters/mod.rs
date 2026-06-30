//! Playable characters as pure data. A `Character` is a small `Copy` descriptor
//! (name, blurb, starter ability, tint, stat-block builder); each variant lives in
//! its own module. `player_scene` builds the BSN scene that spawns the player
//! entity (stats, visuals, starter) for the chosen character.

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::scene::prelude::{bsn, Scene};
use diesel_avian3d::gauge::prelude::{AttributeInitializer, ModifierSet};
use diesel_avian3d::prelude::*;

use crate::ability::Inventory;
use crate::attributes::{Health, MoveSpeed, PickupRadius};
use crate::data::items::ItemDef;
use crate::layers::{Layer, Team};
use crate::meta::MetaProgress;
use crate::player::{MoveInput, Player};
use crate::stats::core_mod_set;

pub mod arcane_storm;
pub mod fireball;
pub mod firebolt;
pub mod firestorm;
pub mod frost_shard;
pub mod ice_storm;
pub mod magic_missile;
pub mod orbiting_blade;

/// A playable character as data: how it presents, which ability it starts with,
/// and a builder for its starting stat block (baseline + its scaling rule).
#[derive(Clone, Copy)]
pub struct Character {
    pub name: &'static str,
    pub blurb: &'static str,
    pub starter: &'static ItemDef,
    pub tint: Color,
    /// Builds the character's base modifier set (callers fold in metaprogression).
    pub stats: fn() -> ModifierSet,
}

/// Every selectable character, in menu order. One **test character per ability**:
/// same neutral loadout, different starter, so each ability can be tried directly
/// from the select screen.
pub fn all() -> [Character; 8] {
    [
        magic_missile::magic_missile(),
        firebolt::firebolt(),
        frost_shard::frost_shard(),
        fireball::fireball(),
        orbiting_blade::orbiting_blade(),
        ice_storm::ice_storm(),
        firestorm::firestorm(),
        arcane_storm::arcane_storm(),
    ]
}

/// Generous, neutral starting stats shared by the per-ability test characters:
/// enough vitality to survive while testing, plus a small `ProjectileCount` bump
/// so volley abilities show their spread and homing curve.
pub(crate) fn test_stats() -> ModifierSet {
    let mut set = core_mod_set(12.0, 5.0);
    set.add("ProjectileCount", 1.0);
    set
}

/// The character chosen on the select screen, read when spawning the player.
#[derive(Resource, Clone, Copy)]
pub struct SelectedCharacter(pub Character);

impl Default for SelectedCharacter {
    fn default() -> Self {
        Self(magic_missile::magic_missile())
    }
}

/// The BSN scene that spawns the player for `character`: shared controller and
/// physics infra, the character-specific stat block (metaprogression folded in at
/// spawn), starter ability slot, and tinted capsule.
pub fn player_scene(character: Character) -> impl Scene {
    let stats = character.stats;
    let starter = character.starter;
    let tint = character.tint;
    bsn! {
        Name::new("Player")
        Player
        MoveInput
        Team::player()
        InvokerTarget::position(Vec3::ZERO)
        Health::default()
        MoveSpeed::default()
        PickupRadius::default()
        Transform::from_xyz(0.0, 0.7, 0.0)
        Visibility::Inherited
        Collider::capsule(0.4, 0.8)
        CollisionLayers::new([Layer::Character], LayerMask::ALL)
        TransformInterpolation
        template(|_| Ok(RigidBody::Kinematic))
        template(move |_| Ok(Inventory::with_starter(starter)))
        template(move |ctx| {
            // Character baseline + scaling, then permanent metaprogression bonuses.
            let mut set = (stats)();
            ctx.resource::<MetaProgress>().apply_to(&mut set);
            Ok(AttributeInitializer::new(set))
        })
        template(move |ctx| {
            let mesh = ctx.resource_mut::<Assets<Mesh>>().add(Capsule3d::new(0.4, 0.8));
            Ok(Mesh3d(mesh))
        })
        template(move |ctx| {
            let material = ctx
                .resource_mut::<Assets<StandardMaterial>>()
                .add(StandardMaterial { base_color: tint, ..default() });
            Ok(MeshMaterial3d(material))
        })
    }
}
