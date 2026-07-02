//! Game content as data: abilities and characters, each variant in its own file.
//! Gameplay code references them through `&'static AbilityDef` / `Character`
//! rather than enums, and behaviour is authored as BSN scenes.

pub mod abilities;
pub mod buffs;
pub mod characters;
pub mod items;
