//! Top-level app flow. Each state owns a BSN scene root that is despawned on
//! exit via `DespawnOnExit`.

use bevy::prelude::*;

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    #[default]
    MainMenu,
    CharSelect,
    Playing,
    GameOver,
}

/// Sub-state of [`AppState::Playing`]. Gameplay systems run only in `Running`;
/// `Paused` and `LevelUp` freeze the simulation while UI is shown.
#[derive(SubStates, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[source(AppState = AppState::Playing)]
pub enum PlayingState {
    #[default]
    Running,
    Paused,
    LevelUp,
}

/// Despawn this entity when leaving the `Playing` state.
pub fn playing_scope() -> DespawnOnExit<AppState> {
    DespawnOnExit(AppState::Playing)
}
