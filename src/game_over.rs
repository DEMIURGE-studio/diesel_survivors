//! Lose condition and restart. The player dying flips `AppState` to `GameOver`,
//! which shows a prompt; pressing R re-enters `Playing`, which resets the arena
//! and respawns the player.

use bevy::prelude::*;

use crate::attributes::Died;
use crate::enemy::Enemy;
use crate::player::Player;
use crate::states::AppState;

pub struct GameOverPlugin;

impl Plugin for GameOverPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_player_died)
            .add_systems(OnEnter(AppState::Playing), reset_arena)
            .add_systems(OnEnter(AppState::GameOver), spawn_game_over_ui)
            .add_systems(Update, restart.run_if(in_state(AppState::GameOver)));
    }
}

#[derive(Component)]
struct GameOverUi;

/// When the player dies, end the run and despawn it (its abilities are
/// linked-spawned, so they go with it).
fn on_player_died(
    died: On<Died>,
    q_player: Query<(), With<Player>>,
    mut commands: Commands,
    mut next: ResMut<NextState<AppState>>,
) {
    if q_player.get(died.entity).is_ok() {
        next.set(AppState::GameOver);
        if let Ok(mut entity) = commands.get_entity(died.entity) {
            entity.try_despawn();
        }
    }
}

/// Clear leftover enemies when (re-)entering Playing. Projectiles expire on
/// their own; the player is spawned fresh by `PlayerPlugin`.
fn reset_arena(mut commands: Commands, enemies: Query<Entity, With<Enemy>>) {
    for entity in &enemies {
        if let Ok(mut ec) = commands.get_entity(entity) {
            ec.try_despawn();
        }
    }
}

fn spawn_game_over_ui(mut commands: Commands) {
    commands
        .spawn((
            GameOverUi,
            DespawnOnExit(AppState::GameOver),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("You Died\nPress R to Restart"),
                TextFont {
                    font_size: FontSize::Px(48.0),
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.35, 0.35)),
            ));
        });
}

fn restart(keys: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<AppState>>) {
    if keys.just_pressed(KeyCode::KeyR) {
        next.set(AppState::Playing);
    }
}
