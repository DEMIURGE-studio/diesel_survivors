//! Playable characters as pure data. A `Character` is a small `Copy` descriptor —
//! name, blurb, starter ability, tint, and a stat-block builder — with no enum;
//! each variant lives in its own module. `player_scene` turns the chosen character
//! into the BSN scene that spawns the player entity (stats + visuals + starter).

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::scene::prelude::{bsn, Scene};
use diesel_avian3d::gauge::prelude::{AttributeInitializer, ModifierSet};
use diesel_avian3d::prelude::*;

use crate::ability::AbilitySlots;
use crate::attributes::{Health, MoveSpeed, PickupRadius};
use crate::data::abilities::AbilityDef;
use crate::layers::{Layer, Team};
use crate::meta::MetaProgress;
use crate::player::{MoveInput, Player};

pub mod arcanist;
pub mod sentinel;

/// A playable character as data: how it presents, which ability it starts with,
/// and a builder for its starting stat block (baseline + its scaling rule).
#[derive(Clone, Copy)]
pub struct Character {
    pub name: &'static str,
    pub blurb: &'static str,
    pub starter: &'static AbilityDef,
    pub tint: Color,
    /// Builds the character's base modifier set (callers fold in metaprogression).
    pub stats: fn() -> ModifierSet,
}

/// Every selectable character, in menu order.
pub fn all() -> [Character; 2] {
    [sentinel::sentinel(), arcanist::arcanist()]
}

/// The character chosen on the select screen, read when spawning the player.
#[derive(Resource, Clone, Copy)]
pub struct SelectedCharacter(pub Character);

impl Default for SelectedCharacter {
    fn default() -> Self {
        Self(arcanist::arcanist())
    }
}

/// The BSN scene that spawns the player for `character`: shared controller +
/// physics infra, plus the character-specific stat block (with metaprogression
/// folded in at spawn), starter ability slot, and tinted capsule.
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
        template(move |_| Ok(AbilitySlots::with_starter(starter)))
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
