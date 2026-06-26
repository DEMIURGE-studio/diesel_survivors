//! Ability runtime: equipped slots, the systems that drive abilities each frame,
//! and the plugin. The abilities themselves are pure data — see
//! [`crate::data::abilities`] — referenced here as `&'static AbilityDef`.
//!
//! Auto-fire (VS-style) is just spamming `StartInvoke` at the player's abilities
//! every frame; each ability's Cooldown state rate-limits itself.

use bevy::prelude::*;
use bevy::scene::prelude::CommandsSceneExt;
use diesel_avian3d::prelude::*;
use rand::Rng;

use crate::data::abilities::{
    register_projectiles, setup_projectile_assets, AbilityDef, Homing, Lifetime, Orbiter,
};
use crate::enemy::Enemy;
use crate::player::Player;
use crate::states::PlayingState;

const BLADE_ORBIT_RADIUS: f32 = 2.2;
const BLADE_ORBIT_SPEED: f32 = 3.5;

/// Number of ability slots a character runs at once.
pub const SLOT_COUNT: usize = 3;

// ---------------------------------------------------------------------------
// Equipped slots
// ---------------------------------------------------------------------------

/// The player's equipped abilities. Filled left-to-right by the starter and the
/// level-up draft; the equip system spawns one ability entity per filled slot.
#[derive(Component, Default)]
pub struct AbilitySlots {
    slots: [Option<&'static AbilityDef>; SLOT_COUNT],
}

impl AbilitySlots {
    pub fn with_starter(starter: &'static AbilityDef) -> Self {
        let mut slots = [None; SLOT_COUNT];
        slots[0] = Some(starter);
        Self { slots }
    }

    pub fn contains(&self, def: &AbilityDef) -> bool {
        self.slots.iter().any(|s| s.is_some_and(|d| d.same(def)))
    }

    pub fn is_full(&self) -> bool {
        self.slots.iter().all(Option::is_some)
    }

    /// Equip into the first empty slot. Returns false if full or already equipped.
    pub fn equip(&mut self, def: &'static AbilityDef) -> bool {
        if self.contains(def) {
            return false;
        }
        if let Some(slot) = self.slots.iter_mut().find(|s| s.is_none()) {
            *slot = Some(def);
            true
        } else {
            false
        }
    }

    pub fn equipped(&self) -> impl Iterator<Item = &'static AbilityDef> + '_ {
        self.slots.iter().filter_map(|s| *s)
    }
}

/// Tags a spawned ability entity with the ability it fulfills, so the equip system
/// can tell which slots are already live and the draft can find an ability's entity.
#[derive(Component, Clone, Copy)]
pub struct SlotAbility(pub &'static AbilityDef);

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct AbilityPlugin;

impl Plugin for AbilityPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TemplateRegistry>()
            .add_systems(Startup, (setup_projectile_assets, register_projectiles))
            .add_systems(Update, (equip_abilities, tick_lifetimes))
            .add_systems(
                Update,
                (
                    update_aim,
                    auto_invoke,
                    orbit_blades,
                    // Launch sideways first, then curve in toward the target.
                    (launch_homing, home_missiles).chain(),
                )
                    .run_if(in_state(PlayingState::Running)),
            );
    }
}

/// Spawn an ability entity for every filled slot that isn't live yet. Runs when
/// `AbilitySlots` changes (player spawn, level-up draft), so equipping an ability
/// brings it online without disturbing the others' cooldowns.
fn equip_abilities(
    q_player: Query<(Entity, &AbilitySlots, Option<&Invokes>), (With<Player>, Changed<AbilitySlots>)>,
    q_slot: Query<&SlotAbility>,
    mut commands: Commands,
) {
    for (player, slots, invokes) in &q_player {
        let live: Vec<&'static AbilityDef> = invokes
            .into_iter()
            .flat_map(|inv| inv.into_iter())
            .filter_map(|&e| q_slot.get(e).ok().map(|s| s.0))
            .collect();
        for def in slots.equipped() {
            if !live.iter().any(|d| d.same(def)) {
                commands
                    .spawn_scene((def.scene)())
                    .insert((InvokedBy(player), SlotAbility(def)));
            }
        }
    }
}

/// Aim the player at the nearest enemy (entity + position) so auto-fired
/// abilities target it and homing projectiles can track it.
fn update_aim(
    mut player: Query<(&GlobalTransform, &mut InvokerTarget), With<Player>>,
    enemies: Query<(Entity, &GlobalTransform), With<Enemy>>,
) {
    let Ok((player_tf, mut target)) = player.single_mut() else {
        return;
    };
    let origin = player_tf.translation();
    let nearest = enemies.iter().min_by(|a, b| {
        let da = a.1.translation().distance_squared(origin);
        let db = b.1.translation().distance_squared(origin);
        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
    });
    if let Some((entity, enemy_tf)) = nearest {
        target.entity = Some(entity);
        target.position = enemy_tf.translation();
    }
}

/// Fire homing projectiles out to the side on launch, so the curve back toward the
/// target reads as homing rather than a straight shot.
fn launch_homing(
    mut projectiles: Query<&mut LinearProjectile, (Added<LinearProjectile>, With<Homing>)>,
) {
    let mut rng = rand::rng();
    for mut proj in &mut projectiles {
        // Rotate the to-target heading by a wide random angle on the ground plane.
        let magnitude = rng.random_range(1.0f32..1.9); // ~57°..109°
        let sign = if rng.random_bool(0.5) { 1.0 } else { -1.0 };
        let rot = Quat::from_axis_angle(Vec3::Y, magnitude * sign);
        proj.direction = (rot * proj.direction).normalize_or_zero();
    }
}

/// Steer homing projectiles toward their target's current position (XZ plane),
/// turning at most `Homing::turn_rate` radians this frame so they arc in.
fn home_missiles(
    time: Res<Time>,
    mut missiles: Query<(&GlobalTransform, &mut LinearProjectile, &AbilityTarget, &Homing)>,
    targets: Query<&GlobalTransform>,
) {
    let dt = time.delta_secs();
    for (tf, mut proj, target, homing) in &mut missiles {
        let Some(target_entity) = target.entity else {
            continue;
        };
        let Ok(target_tf) = targets.get(target_entity) else {
            continue;
        };
        let mut delta = target_tf.translation() - tf.translation();
        delta.y = 0.0;
        let desired = delta.normalize_or_zero();
        if desired == Vec3::ZERO {
            continue;
        }
        let current = proj.direction.normalize_or_zero();
        if current == Vec3::ZERO {
            proj.direction = desired;
            continue;
        }
        let max_step = homing.turn_rate * dt;
        let angle = current.angle_between(desired);
        proj.direction = if angle <= max_step {
            desired
        } else {
            let axis = current.cross(desired).normalize_or_zero();
            let axis = if axis == Vec3::ZERO { Vec3::Y } else { axis };
            (Quat::from_axis_angle(axis, max_step) * current).normalize_or_zero()
        };
    }
}

/// Auto-fire: try to invoke every ability the player owns each frame. Cooldown
/// states gate the actual firing.
fn auto_invoke(
    player: Query<(&Invokes, &InvokerTarget), With<Player>>,
    mut writer: MessageWriter<StartInvoke>,
) {
    let Ok((invokes, target)) = player.single() else {
        return;
    };
    let aim = AbilityTarget::position(target.position);
    for &ability in invokes.into_iter() {
        writer.write(StartInvoke::new(ability, aim));
    }
}

/// Circle each orbiting blade around the player on the XZ plane.
fn orbit_blades(
    time: Res<Time>,
    player: Query<&GlobalTransform, With<Player>>,
    mut blades: Query<(&mut Orbiter, &mut Transform)>,
) {
    let Ok(player_tf) = player.single() else {
        return;
    };
    let center = player_tf.translation();
    for (mut orbiter, mut transform) in &mut blades {
        orbiter.angle += BLADE_ORBIT_SPEED * time.delta_secs();
        transform.translation = center
            + Vec3::new(
                orbiter.angle.cos() * BLADE_ORBIT_RADIUS,
                0.6,
                orbiter.angle.sin() * BLADE_ORBIT_RADIUS,
            );
    }
}

/// Despawn entities whose lifetime has elapsed (transient AoE bursts).
fn tick_lifetimes(
    time: Res<Time>,
    mut lifetimes: Query<(Entity, &mut Lifetime)>,
    mut commands: Commands,
) {
    for (entity, mut lifetime) in &mut lifetimes {
        if lifetime.0.tick(time.delta()).just_finished() {
            if let Ok(mut ec) = commands.get_entity(entity) {
                ec.try_despawn();
            }
        }
    }
}
