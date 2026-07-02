//! Timed buffs as diesel sustained-modifier scenes: equipment's attribute-change
//! mechanism on a timer.
//!
//! A buff is a small state machine attached to the player. Its `Active` state
//! carries an [`AttributeModifiers`] set plus a [`SustainedModifierConfig`]
//! targeting the invoker (the player, threaded in as `InvokedBy(player)`). The
//! sustained-modifier system applies those modifiers when the state gains
//! `Active` and removes them when it loses it. An `AlwaysEdge` with a `Delay`
//! leaves `Active` after the duration, removing the modifiers and despawning the
//! buff. Pick up a buff orb, get a temporary stat boost, and it expires on its
//! own with nothing left behind.

use bevy::prelude::*;
use bevy::scene::prelude::{bsn, Scene};
use diesel_avian3d::gauge::prelude::ModifierSet;
use diesel_avian3d::prelude::*;

use super::abilities::state;
use crate::stats::attr;

const RAGE_DURATION: f32 = 6.0;
/// Flat bonus added to the player's `Damage` while raging.
const RAGE_BONUS: f32 = 12.0;
const HASTE_DURATION: f32 = 6.0;
/// Flat bonus added to the player's `MoveSpeed` while hasted.
const HASTE_BONUS: f32 = 3.0;

/// Shared shell: a buff that applies `mods` to `player` for `duration` seconds,
/// then removes them and despawns. The modifiers live on the `#Active` state, so
/// leaving it on the timed edge hands them to the sustained-modifier remover, no
/// bookkeeping in game code.
fn timed_buff(
    player: Entity,
    name: &'static str,
    duration: f32,
    mods: ModifierSet,
) -> impl Scene {
    bsn! {
        #Root
            Name::new(name)
            StateMachine InitialState(#Active)
        Substates [
            #Active
                template(move |_| Ok(InvokedBy(player)))
                template(move |_| Ok(AttributeModifiers(mods.clone())))
                SustainedModifierConfig::invoker()
                Transitions [
                    (Target(#Done) AlwaysEdge Delay::from_secs_f32(duration))
                ],
            #Done state(DelayedDespawn::now()),
        ]
    }
}

/// Rage: a temporary flat boost to `Damage`.
pub fn rage(player: Entity) -> Box<dyn Scene> {
    let mut mods = ModifierSet::new();
    mods.add(attr::DAMAGE, RAGE_BONUS);
    Box::new(timed_buff(player, "Rage", RAGE_DURATION, mods))
}

/// Haste: a temporary flat boost to `MoveSpeed`.
pub fn haste(player: Entity) -> Box<dyn Scene> {
    let mut mods = ModifierSet::new();
    mods.add(attr::MOVE_SPEED, HASTE_BONUS);
    Box::new(timed_buff(player, "Haste", HASTE_DURATION, mods))
}
