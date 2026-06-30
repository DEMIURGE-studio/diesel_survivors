//! Ability runtime: equipped slots, the systems that drive abilities each frame,
//! and the plugin. The abilities themselves are pure data — see
//! [`crate::data::abilities`] — referenced here as `&'static AbilityDef`.
//!
//! Auto-fire (VS-style) is just spamming `StartInvoke` at the player's abilities
//! every frame; each ability's Cooldown state rate-limits itself.

use avian3d::prelude::ColliderDisabled;
use bevy::prelude::*;
use bevy::scene::prelude::CommandsSceneExt;
use diesel_avian3d::prelude::*;
use rand::Rng;

use crate::data::abilities::{
    register_projectiles, setup_projectile_assets, Homing, Lifetime, Orbiter,
};
use crate::data::items::machine::{equipped_item, EquipIt, Equipped, Unequip};
use crate::data::items::ItemDef;
use crate::enemy::Enemy;
use crate::player::Player;
use crate::states::PlayingState;

const BLADE_ORBIT_RADIUS: f32 = 2.2;
const BLADE_ORBIT_SPEED: f32 = 3.5;

/// Equipment slots (the live abilities) and backpack slots (carried, inactive).
pub const EQUIP_COUNT: usize = 3;
pub const BACKPACK_COUNT: usize = 6;
pub const TOTAL_SLOTS: usize = EQUIP_COUNT + BACKPACK_COUNT;

/// The player's inventory: a flat array whose first [`EQUIP_COUNT`] entries are
/// the **equipped** items (each backed by a live ability + item entity + wearer
/// passive) and the rest are the **backpack** (carried but inactive). The equip
/// system reconciles only the equipped slice, so moving an item between backpack
/// and an equip slot hot-swaps just that one ability — the inventory panel
/// ([`crate::inventory`]) drives it by swapping slots.
#[derive(Component, Default)]
pub struct Inventory {
    slots: [Option<&'static ItemDef>; TOTAL_SLOTS],
}

impl Inventory {
    pub fn with_starter(starter: &'static ItemDef) -> Self {
        let mut slots = [None; TOTAL_SLOTS];
        slots[0] = Some(starter);
        Self { slots }
    }

    /// Is `index` an equipment slot (vs. a backpack slot)?
    pub fn is_equip_slot(index: usize) -> bool {
        index < EQUIP_COUNT
    }

    pub fn get(&self, index: usize) -> Option<&'static ItemDef> {
        self.slots.get(index).copied().flatten()
    }

    /// Held anywhere (equipped or backpack).
    pub fn contains(&self, def: &ItemDef) -> bool {
        self.slots.iter().any(|s| s.is_some_and(|d| d.same(def)))
    }

    /// Held in an equipment slot (i.e. currently live).
    pub fn is_equipped(&self, def: &ItemDef) -> bool {
        self.slots[..EQUIP_COUNT]
            .iter()
            .any(|s| s.is_some_and(|d| d.same(def)))
    }

    /// The equipped items, in slot order.
    pub fn equipped(&self) -> impl Iterator<Item = &'static ItemDef> + '_ {
        self.slots[..EQUIP_COUNT].iter().filter_map(|s| *s)
    }

    pub fn backpack_has_room(&self) -> bool {
        self.slots[EQUIP_COUNT..].iter().any(Option::is_none)
    }

    /// Put a newly-acquired item in the first free backpack slot. Returns false
    /// if already held or the backpack is full.
    pub fn acquire(&mut self, def: &'static ItemDef) -> bool {
        if self.contains(def) {
            return false;
        }
        if let Some(slot) = self.slots[EQUIP_COUNT..].iter_mut().find(|s| s.is_none()) {
            *slot = Some(def);
            true
        } else {
            false
        }
    }

    /// Swap the contents of two slots (the inventory panel's core move).
    pub fn swap(&mut self, a: usize, b: usize) {
        self.slots.swap(a, b);
    }
}

/// Tags a spawned ability entity with the item it fulfills, so the sync system
/// can reconcile inventory ↔ live abilities and the draft can find an item's
/// entity. One such entity exists per *owned* item (equipped or backpack), for
/// the whole run — so per-ability rank-ups applied to it survive un/re-equip.
#[derive(Component, Clone, Copy)]
pub struct SlotItem(pub &'static ItemDef);

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct AbilityPlugin;

impl Plugin for AbilityPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TemplateRegistry>()
            // Item-machine location-zone transitions + the `Equipped` state marker.
            .register_transition::<EquipIt>()
            .register_transition::<Unequip>()
            .register_state_component::<Equipped>()
            .add_systems(Startup, (setup_projectile_assets, register_projectiles))
            .add_systems(
                Update,
                (
                    sync_inventory,
                    drive_equip_zones,
                    on_equipped,
                    on_unequipped,
                    tick_lifetimes,
                ),
            )
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

/// Reconcile the owned item-machines against the player's `Inventory` whenever it
/// changes (player spawn, draft acquire, inventory swap). One item state machine is
/// **materialized per owned item** — equipped *or* backpack — and lives the whole
/// run; equip/unequip only sends `EquipIt` / `Unequip` to drive its `Stored ↔
/// Equipped` zone, never despawning it. That's what lets per-ability rank-ups
/// (gauge instants applied to the machine by the draft) survive un/re-equip. Each
/// machine carries `DespawnOnExit(Playing)`, so a run ending cleans it up.
fn sync_inventory(
    q_player: Query<(Entity, &Inventory), (With<Player>, Changed<Inventory>)>,
    q_live: Query<(Entity, &SlotItem)>,
    mut commands: Commands,
) {
    for (player, inv) in &q_player {
        let live: Vec<(Entity, &'static ItemDef)> =
            q_live.iter().map(|(e, s)| (e, s.0)).collect();

        // Discard: despawn any machine whose item left the inventory entirely (no
        // discard UI yet, but keeps the reconcile total).
        for &(entity, def) in &live {
            if !inv.contains(def) {
                commands.entity(entity).try_despawn();
            }
        }

        // Materialize one machine per owned item (starts parked in `Stored`).
        // `drive_equip_zones` then walks it into the `Equipped` zone if it's slotted.
        for index in 0..TOTAL_SLOTS {
            let Some(item) = inv.get(index) else {
                continue;
            };
            if !live.iter().any(|(_, d)| d.same(item)) {
                commands.spawn_scene(equipped_item(player, item));
            }
        }
    }
}

/// Drive each item-machine's location zone to match its inventory slot, every
/// frame. Idempotent and self-healing: it re-sends `EquipIt` until the machine's
/// `Equipped` marker actually appears (so it survives the one-frame race between
/// spawning a machine and its `Stored` state going active), and `Unequip` until
/// the marker clears. Once the zone matches the slot, it sends nothing.
fn drive_equip_zones(
    q_player: Query<&Inventory, With<Player>>,
    q_machines: Query<(Entity, &SlotItem, Has<Equipped>)>,
    mut equip_w: MessageWriter<EquipIt>,
    mut unequip_w: MessageWriter<Unequip>,
) {
    let Ok(inv) = q_player.single() else {
        return;
    };
    for (entity, slot, is_equipped) in &q_machines {
        let want_equipped = inv.is_equipped(slot.0);
        if want_equipped && !is_equipped {
            equip_w.write(EquipIt::new(entity));
        } else if !want_equipped && is_equipped {
            unequip_w.write(Unequip::new(entity));
        }
    }
}

/// React to the `Equipped` state-marker landing on an item-machine root (its
/// `Equipped` zone became active): reveal it and enable its collider. For invoked
/// abilities the root has no visuals, so this is a harmless no-op; for the blade it
/// brings the orbiter online. (The wearer passive is applied by the zone's own
/// sustained-modifier sub-chart.)
fn on_equipped(q_new: Query<Entity, Added<Equipped>>, mut commands: Commands) {
    for entity in &q_new {
        commands
            .entity(entity)
            .insert(Visibility::Inherited)
            .remove::<ColliderDisabled>();
    }
}

/// React to the `Equipped` marker leaving (the item returned to its `Stored`
/// zone): park the root — hidden and non-colliding — so a benched persistent
/// ability does nothing while keeping its rank-up attributes.
fn on_unequipped(
    mut removed: RemovedComponents<Equipped>,
    q_item: Query<(), With<SlotItem>>,
    mut commands: Commands,
) {
    for entity in removed.read() {
        // Skip machines that were despawned (run end) rather than parked.
        if q_item.get(entity).is_ok() {
            commands
                .entity(entity)
                .insert((Visibility::Hidden, ColliderDisabled));
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

/// Auto-fire: drive every item-machine's invoke loop each frame by holding
/// `InvokeStatus::TryInvoke` (so it re-fires on each `Ready` re-entry) and writing
/// `StartInvoke`. A `Stored` item's `Ready` state is inactive, so this no-ops for
/// benched items — the `Equipped` zone is the firing gate. Persistent abilities
/// (the blade) have no `Ready` state and simply ignore it.
fn auto_invoke(
    player: Query<&InvokerTarget, With<Player>>,
    mut machines: Query<(Entity, &mut InvokeStatus), With<SlotItem>>,
    mut writer: MessageWriter<StartInvoke>,
) {
    let Ok(target) = player.single() else {
        return;
    };
    let aim = AbilityTarget::position(target.position);
    for (entity, mut status) in &mut machines {
        if *status != InvokeStatus::TryInvoke {
            *status = InvokeStatus::TryInvoke;
        }
        writer.write(StartInvoke::new(entity, aim));
    }
}

/// Circle each *equipped* orbiting blade around the player on the XZ plane. A
/// benched blade keeps its angle but stops moving (and is hidden / non-colliding).
fn orbit_blades(
    time: Res<Time>,
    player: Query<&GlobalTransform, With<Player>>,
    mut blades: Query<(&mut Orbiter, &mut Transform), With<Equipped>>,
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
