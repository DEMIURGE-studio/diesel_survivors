//! Component mirrors of gauge attributes. The `AttributeComponent` derive keeps
//! these struct fields in sync with the attribute graph: reads pull from the
//! graph, writes push back so expressions elsewhere can use them.

use bevy::prelude::*;
use diesel_avian3d::gauge::prelude::AttributeComponent;

use crate::states::PlayingState;

/// Health. `current` initializes from `MaxHealth` and writes back to the graph
/// (as `Health.current`) so other expressions can reference live health; `max`
/// tracks the `MaxHealth` attribute.
#[derive(Component, Clone, Debug, Default, AttributeComponent)]
pub struct Health {
    #[read("MaxHealth")]
    pub max: f32,
    #[write]
    #[init_from("MaxHealth")]
    pub current: f32,
}

/// Movement speed mirror, read by the controller.
#[derive(Component, Clone, Debug, Default, AttributeComponent)]
pub struct MoveSpeed {
    #[read("MoveSpeed")]
    pub value: f32,
}

/// Pickup radius mirror, read by the pickup-attraction system.
#[derive(Component, Clone, Debug, Default, AttributeComponent)]
pub struct PickupRadius {
    #[read("PickupRadius")]
    pub value: f32,
}

/// Triggered on an entity when its `current` health reaches 0.
#[derive(EntityEvent, Clone)]
pub struct Died {
    #[event_target]
    pub entity: Entity,
}

pub struct AttributesPlugin;

impl Plugin for AttributesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            check_death.run_if(in_state(PlayingState::Running)),
        );
    }
}

fn check_death(query: Query<(Entity, &Health), Changed<Health>>, mut commands: Commands) {
    for (entity, health) in &query {
        if health.current <= 0.0 && health.max > 0.0 {
            commands.trigger(Died { entity });
        }
    }
}
