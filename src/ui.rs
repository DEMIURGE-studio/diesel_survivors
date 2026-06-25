//! Shared UI helpers: text-buttons with hover/press feedback and a generic
//! "navigate to AppState" button so screens don't each reimplement chrome.

use bevy::prelude::*;

use crate::states::AppState;

pub const NORMAL: Color = Color::srgb(0.14, 0.14, 0.20);
pub const HOVERED: Color = Color::srgb(0.24, 0.24, 0.34);
pub const PRESSED: Color = Color::srgb(0.34, 0.34, 0.50);

/// A button that switches `AppState` when clicked.
#[derive(Component, Clone, Copy)]
pub struct GotoState(pub AppState);

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (button_feedback, goto_handler));
    }
}

/// A clickable text-button bundle. `action` is any marker component(s) a screen's
/// handler queries for (e.g. [`GotoState`] or a screen-specific id).
pub fn button(text: &str, action: impl Bundle) -> impl Bundle {
    (
        Button,
        Node {
            min_width: Val::Px(280.0),
            padding: UiRect::axes(Val::Px(18.0), Val::Px(10.0)),
            margin: UiRect::all(Val::Px(3.0)),
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(NORMAL),
        Text::new(text),
        TextFont {
            font_size: FontSize::Px(24.0),
            ..default()
        },
        TextColor(Color::WHITE),
        action,
    )
}

pub fn title(text: &str) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size: FontSize::Px(52.0),
            ..default()
        },
        TextColor(Color::srgb(0.85, 0.85, 1.0)),
    )
}

pub fn label(text: &str, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
    )
}

/// Fullscreen centered column, despawned when leaving `state`.
pub fn screen(state: AppState) -> impl Bundle {
    (
        DespawnOnExit(state),
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: Val::Px(12.0),
            ..default()
        },
    )
}

fn button_feedback(
    mut buttons: Query<(&Interaction, &mut BackgroundColor), (Changed<Interaction>, With<Button>)>,
) {
    for (interaction, mut color) in &mut buttons {
        *color = match interaction {
            Interaction::Pressed => PRESSED.into(),
            Interaction::Hovered => HOVERED.into(),
            Interaction::None => NORMAL.into(),
        };
    }
}

fn goto_handler(
    buttons: Query<(&Interaction, &GotoState), (Changed<Interaction>, With<Button>)>,
    mut next: ResMut<NextState<AppState>>,
) {
    for (interaction, goto) in &buttons {
        if *interaction == Interaction::Pressed {
            next.set(goto.0);
        }
    }
}
