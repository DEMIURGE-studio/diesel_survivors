//! Shared UI helpers: text-buttons with hover/press feedback and a generic
//! "navigate to AppState" button so screens don't each reimplement chrome.

use bevy::prelude::*;
use bevy::scene::prelude::{bsn, Scene};

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

/// A clickable text-button scene. `marker` is the component a screen's handler
/// queries for (e.g. [`GotoState`] or a screen-specific id); it is `Copy` so the
/// scene can re-insert it on each build.
pub fn button<M: Component + Copy>(text: &str, marker: M) -> impl Scene + use<M> {
    // Own the text up front so the returned scene is `'static` (a `Scene` must
    // be); the move-closure re-inserts an owned clone on each build. Taking a
    // borrowed `&str` and baking it into the scene directly would tie the scene
    // to the caller's lifetime, which callers building it from a temporary
    // (e.g. `&format!(..)`) can't satisfy.
    let text = Text::new(text);
    bsn! {
        Button
        Node {
            min_width: Val::Px(280.0),
            padding: UiRect::axes(Val::Px(18.0), Val::Px(10.0)),
            margin: UiRect::all(Val::Px(3.0)),
            justify_content: JustifyContent::Center,
        }
        BackgroundColor(NORMAL)
        template(move |_| Ok(text.clone()))
        TextFont { font_size: FontSize::Px(24.0) }
        TextColor(Color::WHITE)
        template(move |_| Ok(marker))
    }
}

pub fn title(text: &str) -> impl Scene + use<> {
    let text = Text::new(text);
    bsn! {
        template(move |_| Ok(text.clone()))
        TextFont { font_size: FontSize::Px(52.0) }
        TextColor(Color::srgb(0.85, 0.85, 1.0))
    }
}

pub fn label(text: &str, size: f32, color: Color) -> impl Scene + use<> {
    let text = Text::new(text);
    bsn! {
        template(move |_| Ok(text.clone()))
        TextFont { font_size: FontSize::Px(size) }
        TextColor(color)
    }
}

/// Fullscreen centered column, despawned when leaving `state`. Meant to be
/// included at the root of a screen scene (`screen(state) Children [...]`).
pub fn screen(state: AppState) -> impl Scene {
    bsn! {
        template(move |_| Ok(DespawnOnExit(state)))
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: Val::Px(12.0),
        }
    }
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
