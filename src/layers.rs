//! Physics layers and team-based collision filtering for the diesel hit
//! pipeline. `Team` is read by [`TeamFilter`] so an ability only hits entities
//! on a different team than its invoker.

use avian3d::prelude::*;
use bevy::prelude::*;
use diesel_avian3d::prelude::*;

#[derive(PhysicsLayer, Clone, Copy, Debug, Default)]
pub enum Layer {
    #[default]
    Terrain,
    Character,
    Projectile,
    Pickup,
}

/// Which side an entity fights for. Looked up by [`TeamFilter`].
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct Team(pub u32);

impl Team {
    /// Bare-bsn-friendly alias for the player's team.
    pub fn player() -> Self {
        PLAYER_TEAM
    }
    /// Bare-bsn-friendly alias for the enemy team.
    pub fn enemies() -> Self {
        ENEMY_TEAM
    }
}

pub const PLAYER_TEAM: Team = Team(0);
pub const ENEMY_TEAM: Team = Team(1);

/// Filters diesel hits by team. `Enemies` restricts an ability to entities on a
/// different team than its invoker.
#[derive(Component, Clone, Debug, Default, FromTemplate)]
pub enum TeamFilter {
    #[default]
    Enemies,
}

impl CollisionFilter for TeamFilter {
    type Lookup = Team;

    fn can_target(&self, invoker: Option<&Team>, target: Option<&Team>) -> bool {
        match (self, invoker, target) {
            (TeamFilter::Enemies, Some(i), Some(t)) => i.0 != t.0,
            _ => true,
        }
    }
}
