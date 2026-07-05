//! The item state machine: every owned item is one persistent gearbox machine
//! with a `Stored` <-> `Equipped` location zone. Entering the `Equipped` zone
//! starts the ability's auto-fire loop and applies its wearer passive; leaving it
//! parks everything. The machine (and its rank-up instants) lives for the whole
//! run, since equip/unequip never despawns it, so upgrades persist across swaps.
//! Modelled on the `survivors` location-zone pattern.
//!
//! ```text
//! Item (StateMachine, Ability, attrs = ability base + item local)
//! └── EquipZone (InitialState = Stored)
//!     ├── Stored      ──EquipIt──> Equipped
//!     └── Equipped    state(Equipped) + wearer sustained mods   ──Unequip──> Stored
//!         └── <ability region>   e.g. Ready→Invoking→Cooldown (auto-fire loop)
//! ```
//!
//! The auto-fire loop is pure data: while `Equipped` is active the ability cycles
//! `Ready->Invoking->Cooldown->Ready` on its own. `Ready->Invoking` is an
//! `AlwaysEdge`, so the machine self-drives with no external trigger; the
//! `Equipped` location-zone is the firing gate. A `Stored` item's region is
//! inactive, so it doesn't tick.

use avian3d::prelude::ColliderDisabled;
use bevy::ecs::template::EntityTemplate;
use bevy::prelude::*;
use bevy::scene::prelude::{bsn, Scene};
use bevy_gauge::prelude::AttributeInitializer;
use bevy_gearbox::{AcceptAll, GearboxMessage};
use diesel_avian3d::prelude::*;
use bevy_gauge::prelude::*;
use bevy_gearbox::prelude::*;

use crate::ability::SlotItem;
use crate::data::abilities::state;
use crate::stats::attr;
use crate::states::AppState;

use super::{Item, ItemDef};

// ---------------------------------------------------------------------------
// Location-zone transition messages
// ---------------------------------------------------------------------------

/// Drive an item's machine `Stored -> Equipped`.
#[derive(Message, Clone, Debug, Reflect)]
pub struct EquipIt {
    pub item: Entity,
}
/// Drive an item's machine `Equipped -> Stored`.
#[derive(Message, Clone, Debug, Reflect)]
pub struct Unequip {
    pub item: Entity,
}

impl EquipIt {
    pub fn new(item: Entity) -> Self {
        Self { item }
    }
}
impl Unequip {
    pub fn new(item: Entity) -> Self {
        Self { item }
    }
}

impl GearboxMessage for EquipIt {
    type Validator = AcceptAll;
    fn target(&self) -> Entity {
        self.item
    }
}
impl GearboxMessage for Unequip {
    type Validator = AcceptAll;
    fn target(&self) -> Entity {
        self.item
    }
}

// ---------------------------------------------------------------------------
// Equipped marker (driven by the `Equipped` state via `StateComponent`)
// ---------------------------------------------------------------------------

/// Placed on the item-machine root while its `Equipped` state is active (via
/// `state(Equipped)` -> `StateComponent`). Systems gate on it: the orbiting blade
/// only orbits / collides / shows `With<Equipped>` (see [`crate::ability`]).
#[derive(Component, Clone, Copy, Default)]
pub struct Equipped;

// ---------------------------------------------------------------------------
// Builders
// ---------------------------------------------------------------------------

/// The standard invoked auto-fire region, merged onto the `Equipped` state:
/// `Ready -> Invoking -> Cooldown -> Ready`. Like diesel's `invoked_with` without
/// the outer machine: here the item is the `Ability` machine root. The loop is
/// self-driving (`Ready -> Invoking` is an `AlwaysEdge`): it cycles continuously
/// while `Equipped` is active and freezes when the item returns to `Stored`.
pub fn invoked_region<F, S>(
    root: EntityTemplate,
    cooldown_secs: f32,
    make_inner: F,
) -> impl Scene
where
    F: Fn(EntityTemplate) -> S + Send + Sync + 'static,
    S: Scene,
{
    bsn! {
        InitialState(#Ready)
        Substates [
            #Ready Transitions [
                (Target(#Invoking) AlwaysEdge)
            ],
            #Invoking InitialState(#Inner) Transitions [
                (Target(#Cooldown) MessageEdge::<Done>::default())
            ] Substates [
                #Inner make_inner(root)
            ],
            #Cooldown Transitions [
                (Target(#Ready) AlwaysEdge Delay::from_secs_f32(cooldown_secs)
                    InvokedBy(root)
                    template(|_| Ok(attributes! { "Delay" => "Cooldown@ability" })))
            ],
        ]
    }
}

/// Build the full item state machine for `def`, owned by `player`.
///
/// The item entity is the `Ability` machine root carrying the merged attributes
/// (the ability's `base` + the item's `local`, read as `@ability`/`@item`). Its
/// one region is the `Stored <-> Equipped` zone; the `Equipped` state hosts the
/// ability's auto-fire region and applies the wearer passive as a sustained
/// modifier on the player. Persistent-visual abilities (the blade) contribute
/// root components via `root_extras`.
pub fn equipped_item(player: Entity, def: &'static ItemDef) -> impl Scene {
    let ability = def.ability;

    // Merge the ability's base attributes with the item's local attributes, and
    // default the `Damage` multiplier to 1.0 (like diesel's `invoked_with`).
    let mut attrs = (ability.base)();
    let local = (def.local)();
    for entry in local.entries() {
        attrs.add_tagged(&entry.attribute, entry.value.clone(), entry.tag);
    }
    if !attrs.entries().iter().any(|e| e.attribute == attr::DAMAGE) {
        attrs.add(attr::DAMAGE, 1.0);
    }

    let root_extras = (ability.root_extras)();
    let name = ability.name;

    bsn! {
        #Item StateMachine Ability
            Name::new(name)
            template(move |_| Ok(SlotItem(def)))
            template(move |_| Ok(InvokedBy(player)))
            Item
            template(|_| Ok(DespawnOnExit(AppState::Playing)))
            // Parked by default; `on_equipped` reveals + enables persistent
            // visuals/collider (blade) when the item enters the Equipped zone.
            Visibility::Hidden
            template(|_| Ok(ColliderDisabled))
            template(move |_| Ok(AttributeInitializer::new(attrs.clone())))
            { root_extras }
        Substates [
            #Zone equip_zone(#Item, def)
        ]
    }
}

/// The `Stored <-> Equipped` location zone (a sequential region under the item
/// root). The ability's auto-fire region is merged onto `Equipped`; the wearer
/// passive is a sustained modifier that applies while `Equipped` is active.
fn equip_zone(root: EntityTemplate, def: &'static ItemDef) -> impl Scene {
    let wearer = (def.wearer)();
    let region = (def.ability.region)(root);

    bsn! {
        #EquipZone InitialState(#Stored)
        Substates [
            #Stored Transitions [
                (Target(#Equipped) MessageEdge::<EquipIt>::default())
            ],
            #Equipped state(Equipped)
                InvokedBy(root)
                template(move |_| Ok(AttributeModifiers(wearer.clone())))
                SustainedModifierConfig::invoker()
                Transitions [
                    (Target(#Stored) MessageEdge::<Unequip>::default())
                ]
                { region }
        ]
    }
}
