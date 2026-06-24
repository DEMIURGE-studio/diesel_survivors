//! diesel_survivors — a Vampire Survivors-like reference game showcasing
//! bevy_diesel (abilities), bevy_gauge (attributes), and bevy_gearbox (state
//! charts). Everything is authored as BSN scenes.

mod layers;
mod states;

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
            PhysicsPlugins::default(),
            AvianBackend::plugin(),
            CollisionFilterPlugin::<TeamFilter>::default(),
            diesel_avian3d::gearbox::server::ServerPlugin::default(),
        ))
        .init_state::<AppState>()
        .add_sub_state::<PlayingState>()
        .insert_resource(ClearColor(Color::srgb(0.06, 0.06, 0.08)))
        .add_systems(Startup, setup_camera)
        .run();
}

/// Top-down camera: fixed offset looking down at the play field. Follow/smoothing
/// comes later with the player controller.
fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Name::new("MainCamera"),
        Camera3d::default(),
        Transform::from_xyz(0.0, 24.0, 0.0).looking_at(Vec3::ZERO, Vec3::NEG_Z),
    ));
}
