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

/// Reads `GoOff`, resolves the `DamageEffect`, evaluates damage, and subtracts
/// from the defender's `Health`. Team gating already happened at the collision
/// layer (`TeamFilter`), so a `GoOff` here is always a valid hit.
fn damage_effect_system(
    mut reader: MessageReader<GoOff>,
    q_effect: Query<&DamageEffect>,
    q_invoked_by: Query<&InvokedBy>,
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

        let Ok(expr) = Expr::compile(&effect.expr, None) else {
            warn!("DamageEffect: failed to compile expr '{}'", effect.expr);
            continue;
        };

        let roles: &[(&str, Entity)] = &[
            ("attacker", attacker),
            ("invoker", attacker),
            ("defender", defender),
            ("target", defender),
            ("ability", effect_entity),
        ];
        let scope_extras: Vec<(&str, f32)> =
            go_off.scope.iter().map(|&(k, v)| (k, v)).collect();
        let raw = attributes.evaluate_expr_with_roles_ctx(&expr, attacker, roles, Some(&scope_extras));

        let resistance = attributes
            .get_attributes(defender)
            .map(|a| a.value_tagged("Resistance", effect.damage_type))
            .unwrap_or(0.0)
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
