//! Front-end flow: MainMenu -> CharSelect -> Playing. Clickable buttons, with
//! key shortcuts (Enter / 1 / 2 / U).

use bevy::prelude::*;

use crate::data::characters::{self, Character, SelectedCharacter};
use crate::states::AppState;
use crate::ui::{button, label, screen, title, GotoState};

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::MainMenu), spawn_main_menu)
            .add_systems(
                Update,
                main_menu_keys.run_if(in_state(AppState::MainMenu)),
            )
            .add_systems(OnEnter(AppState::CharSelect), spawn_char_select)
            .add_systems(
                Update,
                (char_select_keys, char_button_clicks).run_if(in_state(AppState::CharSelect)),
            );
    }
}

/// Button carrying which character it selects.
#[derive(Component, Clone, Copy)]
struct CharButton(Character);

fn spawn_main_menu(mut commands: Commands) {
    commands.spawn(screen(AppState::MainMenu)).with_children(|p| {
        p.spawn(title("DIESEL SURVIVORS"));
        p.spawn(button("Play", GotoState(AppState::CharSelect)));
        p.spawn(button("Upgrades", GotoState(AppState::Upgrades)));
        p.spawn(label(
            "Enter to play  |  U for upgrades",
            18.0,
            Color::srgb(0.55, 0.55, 0.55),
        ));
    });
}

fn main_menu_keys(keys: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<AppState>>) {
    if keys.just_pressed(KeyCode::Enter) {
        next.set(AppState::CharSelect);
    } else if keys.just_pressed(KeyCode::KeyU) {
        next.set(AppState::Upgrades);
    }
}

fn spawn_char_select(mut commands: Commands) {
    commands
        .spawn(screen(AppState::CharSelect))
        .with_children(|p| {
            p.spawn(title("Choose Your Survivor"));
            for character in characters::all() {
                p.spawn(button(character.name, CharButton(character)));
                p.spawn(label(character.blurb, 18.0, Color::srgb(0.65, 0.65, 0.65)));
            }
            p.spawn(label(
                "Click, or press 1-8",
                18.0,
                Color::srgb(0.55, 0.55, 0.55),
            ));
        });
}

fn begin_run(commands: &mut Commands, next: &mut NextState<AppState>, character: Character) {
    commands.insert_resource(SelectedCharacter(character));
    next.set(AppState::Playing);
}

fn char_select_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut next: ResMut<NextState<AppState>>,
) {
    const DIGITS: [KeyCode; 8] = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
    ];
    let roster = characters::all();
    for (i, key) in DIGITS.iter().enumerate() {
        if keys.just_pressed(*key) {
            if let Some(character) = roster.get(i) {
                begin_run(&mut commands, &mut next, *character);
            }
        }
    }
}

fn char_button_clicks(
    buttons: Query<(&Interaction, &CharButton), (Changed<Interaction>, With<Button>)>,
    mut commands: Commands,
    mut next: ResMut<NextState<AppState>>,
) {
    for (interaction, char_button) in &buttons {
        if *interaction == Interaction::Pressed {
            begin_run(&mut commands, &mut next, char_button.0);
        }
    }
}
