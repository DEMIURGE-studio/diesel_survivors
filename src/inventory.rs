//! The inventory panel: a right-side UI showing the player's **equipment** slots
//! (the 3 live abilities) above the **backpack** (6 carried-but-inactive items).
//!
//! Interaction is two-click swap: click a slot to pick it up, then click another
//! to swap their contents. Moving an item between the backpack and an equipment
//! slot mutates [`Inventory`], which the equip system reconciles into a live
//! ability hot-swap (see [`crate::ability`]) — the player-facing face of the
//! diesel/gauge equip/unequip showcase. Backpack-only swaps just reorder.

use bevy::prelude::*;

use crate::ability::{Inventory, EQUIP_COUNT, TOTAL_SLOTS};
use crate::player::Player;
use crate::states::AppState;
use crate::ui::{HOVERED, NORMAL};

/// Empty slot.
const EMPTY: Color = Color::srgb(0.08, 0.08, 0.11);
/// Picked-up slot, awaiting a swap destination.
const SELECTED: Color = Color::srgb(0.5, 0.42, 0.16);
/// An occupied equipment slot (distinct from a backpack slot).
const EQUIP_TINT: Color = Color::srgb(0.17, 0.15, 0.23);

/// Which slot is currently picked up (awaiting a destination click).
#[derive(Resource, Default)]
struct Selection(Option<usize>);

/// A clickable inventory slot button, carrying its flat slot index.
#[derive(Component, Clone, Copy)]
struct SlotButton(usize);

pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Selection>()
            .add_systems(OnEnter(AppState::Playing), spawn_panel)
            .add_systems(OnExit(AppState::Playing), reset_selection)
            .add_systems(
                Update,
                (slot_clicks, refresh_slots).chain().run_if(in_state(AppState::Playing)),
            );
    }
}

fn reset_selection(mut selection: ResMut<Selection>) {
    selection.0 = None;
}

fn spawn_panel(mut commands: Commands) {
    commands
        .spawn((
            DespawnOnExit(AppState::Playing),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(12.0),
                right: Val::Px(12.0),
                width: Val::Px(190.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
        ))
        .with_children(|p| {
            p.spawn(heading("Equipped"));
            for i in 0..EQUIP_COUNT {
                p.spawn(slot(i));
            }
            p.spawn(heading("Backpack"));
            for i in EQUIP_COUNT..TOTAL_SLOTS {
                p.spawn(slot(i));
            }
            p.spawn((
                Text::new("Click a slot, then another, to swap"),
                TextFont {
                    font_size: FontSize::Px(11.0),
                    ..default()
                },
                TextColor(Color::srgb(0.5, 0.5, 0.55)),
                Node {
                    margin: UiRect::top(Val::Px(4.0)),
                    ..default()
                },
            ));
        });
}

fn heading(text: &str) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size: FontSize::Px(14.0),
            ..default()
        },
        TextColor(Color::srgb(0.7, 0.7, 0.82)),
        Node {
            margin: UiRect::top(Val::Px(4.0)),
            ..default()
        },
    )
}

fn slot(index: usize) -> impl Bundle {
    (
        Button,
        SlotButton(index),
        Node {
            width: Val::Percent(100.0),
            min_height: Val::Px(26.0),
            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(EMPTY),
        Text::new("—"),
        TextFont {
            font_size: FontSize::Px(13.0),
            ..default()
        },
        TextColor(Color::WHITE),
    )
}

/// Two-click swap: first click picks a slot up, second click swaps it with the
/// target (or cancels if it's the same slot). Swapping into/out of an equipment
/// slot triggers the equip reconcile via `Changed<Inventory>`.
fn slot_clicks(
    buttons: Query<(&Interaction, &SlotButton), (Changed<Interaction>, With<Button>)>,
    mut selection: ResMut<Selection>,
    mut player: Query<&mut Inventory, With<Player>>,
) {
    let Ok(mut inv) = player.single_mut() else {
        return;
    };
    for (interaction, slot) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match selection.0 {
            // Nothing held yet — pick up this slot (only if it has an item).
            None => {
                if inv.get(slot.0).is_some() {
                    selection.0 = Some(slot.0);
                }
            }
            // Holding a slot — drop onto this one (swap), or cancel if it's the
            // same slot.
            Some(src) => {
                if src != slot.0 {
                    inv.swap(src, slot.0);
                }
                selection.0 = None;
            }
        }
    }
}

/// Repaint every slot from the inventory each frame: its label (item name +
/// weapon type), and its colour (selected / hovered / occupied / empty, with
/// equipment slots tinted apart from the backpack).
fn refresh_slots(
    selection: Res<Selection>,
    player: Query<&Inventory, With<Player>>,
    mut slots: Query<(&SlotButton, &Interaction, &mut BackgroundColor, &mut Text)>,
) {
    let Ok(inv) = player.single() else {
        return;
    };
    for (slot, interaction, mut bg, mut text) in &mut slots {
        let item = inv.get(slot.0);
        *text = Text::new(match item {
            Some(def) => format!("{} [{}]", def.name, def.weapon.label()),
            None => "—".to_string(),
        });
        *bg = if selection.0 == Some(slot.0) {
            SELECTED.into()
        } else if *interaction == Interaction::Hovered {
            HOVERED.into()
        } else if item.is_some() {
            if Inventory::is_equip_slot(slot.0) {
                EQUIP_TINT.into()
            } else {
                NORMAL.into()
            }
        } else {
            EMPTY.into()
        };
    }
}
