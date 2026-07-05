//! diesel_survivors: a Vampire Survivors-like reference game showcasing
//! bevy_diesel (abilities), bevy_gauge (attributes), and bevy_gearbox (state
//! charts). Everything is authored as BSN scenes.

mod ability;
mod arena;
mod attributes;
mod damage;
mod data;
mod enemy;
mod game_over;
mod hud;
mod inventory;
mod layers;
mod menu;
mod meta;
mod pickups;
mod player;
mod progression;
mod stats;
mod states;
mod ui;

use avian3d::prelude::*;
use bevy::prelude::*;
use diesel_avian3d::prelude::*;

use crate::layers::TeamFilter;
use crate::states::{AppState, PlayingState};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "diesel_survivors".into(),
                    ..default()
                }),
                ..default()
            }),
            // PhysicsPlugins::default() includes PhysicsInterpolationPlugin;
            // bodies opt in per-entity via the `TransformInterpolation` component.
            PhysicsPlugins::default(),
            AvianBackend::plugin(),
            CollisionFilterPlugin::<TeamFilter>::default(),
            bevy_gearbox::server::ServerPlugin::default(),
        ))
        .init_state::<AppState>()
        .add_sub_state::<PlayingState>()
        .init_resource::<data::characters::SelectedCharacter>()
        .add_plugins((
            attributes::AttributesPlugin,
            damage::DamagePlugin,
            arena::ArenaPlugin,
            player::PlayerPlugin,
            enemy::EnemyPlugin,
            ability::AbilityPlugin,
            pickups::PickupPlugin,
            game_over::GameOverPlugin,
            menu::MenuPlugin,
            progression::ProgressionPlugin,
            meta::MetaPlugin,
            hud::HudPlugin,
            inventory::InventoryPlugin,
            ui::UiPlugin,
        ))
        .insert_resource(ClearColor(Color::srgb(0.06, 0.06, 0.08)))
        .add_systems(Startup, setup_camera)
        .run();
}

/// Top-down camera: fixed offset looking down at the play field.
fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Name::new("MainCamera"),
        Camera3d::default(),
        Transform::from_translation(player::CAMERA_OFFSET).looking_at(Vec3::ZERO, Vec3::Y),
        AmbientLight {
            color: Color::WHITE,
            brightness: 220.0,
            ..default()
        },
    ));
}
