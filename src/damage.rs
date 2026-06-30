//! Damage typing and the hit→damage pipeline.
//!
//! Tags let gauge modifiers filter by element: a "+10 fire damage" modifier
//! matches fireballs but not frost novae.
//!
//! The pipeline is diesel-native: an ability's firing state has `DamageEffect`
//! sub-effects (`SubEffectOf`). When the state activates, diesel fires a `GoOff`
//! per sub-effect carrying the defender as its target; [`damage_effect_system`]
//! reads those, evaluates the damage expression against the attacker/defender
//! attribute graphs, and subtracts from the defender's [`Health`] component.

use bevy::prelude::*;
use diesel_avian3d::gauge::prelude::*;
use diesel_avian3d::prelude::*;

use crate::attributes::Health;
use crate::layers::{Team, TeamFilter};

define_tags! {
    DamageTags,
    element {
        physical,
        fire,
        cold,
        lightning,
        arcane,
    },
}

/// A sub-effect that deals damage when its parent state fires. The ability only
/// declares a raw-damage expression and an element; resistance and health
/// subtraction are the pipeline's job.
#[derive(Component, Clone, Debug, Default)]
pub struct DamageEffect {
    /// Expression for raw damage, evaluated with `invoker`/`target` roles.
    pub expr: String,
    /// Element tag, used for resistance lookups on the defender.
    pub damage_type: TagMask,
}

impl DamageEffect {
    pub fn new(expr: impl Into<String>, damage_type: TagMask) -> Self {
        Self {
            expr: expr.into(),
            damage_type,
        }
    }
    pub fn physical(expr: impl Into<String>) -> Self {
        Self::new(expr, DamageTags::PHYSICAL)
    }
    pub fn fire(expr: impl Into<String>) -> Self {
        Self::new(expr, DamageTags::FIRE)
    }
    pub fn cold(expr: impl Into<String>) -> Self {
        Self::new(expr, DamageTags::COLD)
    }
    pub fn lightning(expr: impl Into<String>) -> Self {
        Self::new(expr, DamageTags::LIGHTNING)
    }
    pub fn arcane(expr: impl Into<String>) -> Self {
        Self::new(expr, DamageTags::ARCANE)
    }
}

/// Marks a `GoOff` as a discrete hit (paired with [`DamageEffect`] on real
/// strikes; omitted on DOT/tick sources so they don't fire hit triggers).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct HitEffect;

pub struct DamagePlugin;

impl Plugin for DamagePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_tags).add_systems(
            diesel_avian3d::gearbox::GearboxSchedule,
            damage_effect_system.in_set(diesel_avian3d::bevy_diesel::DieselSet::Effects),
        );
    }
}

fn setup_tags(mut resolver: ResMut<TagResolver>) {
    DamageTags::register(&mut resolver);
}

/// Walk the `InvokedBy` chain from an effect to the first `TeamFilter`.
fn find_team_filter<'a>(
    start: Entity,
    q_filter: &'a Query<&TeamFilter>,
    q_invoked_by: &Query<&InvokedBy>,
) -> Option<&'a TeamFilter> {
    let mut entity = start;
    loop {
        if let Ok(filter) = q_filter.get(entity) {
            return Some(filter);
        }
        match q_invoked_by.get(entity) {
            Ok(invoked_by) => entity = invoked_by.0,
            Err(_) => return None,
        }
    }
}

/// Walk the `InvokedBy` chain from an effect to the owning ability (first
/// `Ability`). The chain is preserved across spawns, so this resolves to the
/// top-level spell — the entity whose `Damage` multiplier `@ability` should read.
fn find_owning_ability(
    start: Entity,
    q_ability: &Query<(), With<Ability>>,
    q_invoked_by: &Query<&InvokedBy>,
) -> Option<Entity> {
    let mut entity = start;
    loop {
        if q_ability.get(entity).is_ok() {
            return Some(entity);
        }
        match q_invoked_by.get(entity) {
            Ok(invoked_by) => entity = invoked_by.0,
            Err(_) => return None,
        }
    }
}

/// Reads `GoOff`, resolves the `DamageEffect`, evaluates damage, and subtracts
/// from the defender's `Health`. Re-checks the ability's `TeamFilter` here too,
/// since area-gathered targets (AoE) bypass the collision-layer team check.
fn damage_effect_system(
    mut reader: MessageReader<GoOff>,
    q_effect: Query<&DamageEffect>,
    q_invoked_by: Query<&InvokedBy>,
    q_ability: Query<(), With<Ability>>,
    q_filter: Query<&TeamFilter>,
    q_team: Query<&Team>,
    mut q_health: Query<&mut Health>,
    mut attributes: AttributesMut,
) {
    for go_off in reader.read() {
        let effect_entity = go_off.entity;
        let Ok(effect) = q_effect.get(effect_entity) else {
            continue;
        };
        let Some(defender) = go_off.target.entity else {
            continue;
        };
        let attacker = q_invoked_by.root_ancestor(effect_entity);
        // `@ability` = the owning spell (its per-ability `Damage` multiplier), not
        // the leaf effect entity. Falls back to the effect for non-ability sources.
        let ability = find_owning_ability(effect_entity, &q_ability, &q_invoked_by)
            .unwrap_or(effect_entity);
        // The item *is* the ability root now (one entity), so `@item` — a weapon's
        // own `Damage.base` — resolves to the same entity as `@ability`.
        let item = ability;

        // Team gate: skip if the ability's filter rejects attacker → defender.
        if let Some(filter) = find_team_filter(effect_entity, &q_filter, &q_invoked_by) {
            let invoker_team = q_team.get(attacker).ok();
            let target_team = q_team.get(defender).ok();
            if !filter.can_target(invoker_team, target_team) {
                continue;
            }
        }

        let Ok(expr) = Expr::compile(&effect.expr, None) else {
            warn!("DamageEffect: failed to compile expr '{}'", effect.expr);
            continue;
        };

        let roles: &[(&str, Entity)] = &[
            ("attacker", attacker),
            ("invoker", attacker),
            ("defender", defender),
            ("target", defender),
            ("ability", ability),
            ("item", item),
        ];
        let scope_extras: Vec<(&str, f32)> =
            go_off.scope.iter().map(|&(k, v)| (k, v)).collect();
        let raw = attributes.evaluate_expr_with_roles_ctx(&expr, attacker, roles, Some(&scope_extras));

        // `evaluate_tagged` registers the (Resistance, element) tag query on first
        // use and sums the defender's matching tagged modifiers. A read-only
        // `value_tagged` returns 0 until some other path registers that query, so
        // resistances would silently never apply.
        let resistance = attributes
            .evaluate_tagged(defender, "Resistance", effect.damage_type)
            .clamp(0.0, 1.0);

        let final_damage = raw * (1.0 - resistance);
        if final_damage <= 0.0 {
            continue;
        }
        if let Ok(mut health) = q_health.get_mut(defender) {
            health.current -= final_damage;
        }
    }
}
