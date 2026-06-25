//! Leveling and the ability draft. Kills grant XP; hitting the threshold pauses
//! into `PlayingState::LevelUp` and offers the player an unequipped ability to
//! slot. Equipping mutates `AbilitySlots`, which the equip system reconciles into
//! a live ability entity.

use bevy::prelude::*;
use diesel_avian3d::gauge::prelude::{AttributesMut, InstantExt};
use rand::Rng;

use crate::ability::{AbilitySlots, SlotAbility};
use crate::attributes::{Died, PickupRadius};
use crate::data::abilities::{AbilityDef, ALL};
use crate::enemy::Enemy;
use crate::player::Player;
use crate::states::{AppState, PlayingState};
use crate::ui::{button, title};

const GEM_VALUE: u32 = 1;
const GEM_ATTRACT_SPEED: f32 = 14.0;
const GEM_COLLECT_RADIUS: f32 = 0.6;

#[derive(Resource)]
pub struct Experience {
    pub current: u32,
    pub level: u32,
    pub to_next: u32,
}

impl Default for Experience {
    fn default() -> Self {
        Self {
            current: 0,
            level: 1,
            to_next: 3,
        }
    }
}

/// A rarity tier for a rolled upgrade: rarer tiers grant a bigger percentage.
#[derive(Clone, Copy)]
struct Tier {
    label: &'static str,
    /// Fraction of the stat's current value the upgrade adds/removes.
    pct: f32,
    /// Relative roll weight.
    weight: u32,
}

const TIERS: [Tier; 3] = [
    Tier { label: "Common", pct: 0.10, weight: 60 },
    Tier { label: "Rare", pct: 0.20, weight: 30 },
    Tier { label: "Legendary", pct: 0.35, weight: 10 },
];

/// Weighted roll over [`TIERS`].
fn roll_tier(rng: &mut impl Rng) -> Tier {
    let total: u32 = TIERS.iter().map(|t| t.weight).sum();
    let mut r = rng.random_range(0..total);
    for tier in &TIERS {
        if r < tier.weight {
            return *tier;
        }
        r -= tier.weight;
    }
    TIERS[0]
}

/// How an upgrade mutates a gauge attribute. Percentage ops scale by the stat's
/// own current value (so they compound and never overshoot a floor); flat adds a
/// fixed amount (for integer-ish stats like projectile count).
#[derive(Clone, Copy)]
enum Op {
    /// `stat += stat * pct` — "more is better" (Damage, Area, …).
    AddPct,
    /// `stat -= stat * pct` — "less is better", self-flooring (CooldownMult).
    SubPct,
    /// `stat += amount` — fixed step (ProjectileCount).
    AddFlat(f32),
}

/// Apply an upgrade to one attribute on `entity`. Built as an [`InstantModifierSet`]
/// by hand (not the `instant!` macro) so the attribute name can be dynamic.
fn apply_op(stat: &str, op: Op, pct: f32, entity: Entity, attrs: &mut AttributesMut) {
    use diesel_avian3d::gauge::prelude::InstantModifierSet;
    let mut inst = InstantModifierSet::new();
    match op {
        Op::AddPct => inst.push_add(stat, format!("{stat} * {pct}").as_str()),
        Op::SubPct => inst.push_sub(stat, format!("{stat} * {pct}").as_str()),
        Op::AddFlat(amount) => inst.push_add(stat, amount),
    }
    attrs.apply_instant(&inst, &[], entity);
}

/// A stat an ability exposes for per-ability rank-ups; offered only when the
/// ability actually carries `stat` (e.g. only AoE abilities have `AreaBase`).
struct AbilityStat {
    label: &'static str,
    stat: &'static str,
    op: Op,
}

const ABILITY_STATS: [AbilityStat; 4] = [
    AbilityStat { label: "Damage", stat: "Damage", op: Op::AddPct },
    AbilityStat { label: "Cooldown", stat: "CooldownBase", op: Op::SubPct },
    AbilityStat { label: "Area", stat: "AreaBase", op: Op::AddPct },
    AbilityStat { label: "Speed", stat: "ProjectileSpeedBase", op: Op::AddPct },
];

/// Whether `def` exposes the stat at `ABILITY_STATS[idx]` for a rank-up (Damage is
/// always rankable; the rest follow the ability's declared `stats`).
fn ability_has_stat(def: &AbilityDef, idx: usize) -> bool {
    match idx {
        0 => true,
        1 => def.stats.cooldown,
        2 => def.stats.area,
        3 => def.stats.projectile_speed,
        _ => false,
    }
}

/// A player-wide passive upgrade (applied to the player's global stat, so it
/// scales every ability or the character itself at once).
struct GlobalUpgrade {
    label: &'static str,
    stat: &'static str,
    op: Op,
}

const GLOBAL_UPGRADES: [GlobalUpgrade; 9] = [
    GlobalUpgrade { label: "Power", stat: "Damage", op: Op::AddPct },
    GlobalUpgrade { label: "Swiftness", stat: "AttackSpeed", op: Op::AddPct },
    GlobalUpgrade { label: "Expanse", stat: "Area", op: Op::AddPct },
    GlobalUpgrade { label: "Velocity", stat: "ProjectileSpeed", op: Op::AddPct },
    GlobalUpgrade { label: "Cooldown", stat: "CooldownMult", op: Op::SubPct },
    GlobalUpgrade { label: "Multishot", stat: "ProjectileCount", op: Op::AddFlat(1.0) },
    GlobalUpgrade { label: "Vigor", stat: "Vitality", op: Op::AddPct },
    GlobalUpgrade { label: "Fleet", stat: "MoveSpeed", op: Op::AddPct },
    GlobalUpgrade { label: "Magnet", stat: "PickupRadius", op: Op::AddPct },
];

/// One choice on the level-up screen. Each carries its pre-rolled tier so the
/// offered magnitude is fixed and shown.
#[derive(Clone, Copy)]
enum DraftOption {
    /// Slot a not-yet-equipped ability.
    Equip(&'static AbilityDef),
    /// Buff one stat of an owned ability (instant on the spell entity).
    AbilityUp { def: &'static AbilityDef, stat: usize, tier: Tier },
    /// Buff a player-wide passive (instant on the player).
    Global { kind: usize, tier: Tier },
}

/// Format `+N%` / `−N%` / `+N` for a tier and op.
fn magnitude_label(op: Op, pct: f32) -> String {
    match op {
        Op::AddPct => format!("+{}%", (pct * 100.0).round() as i32),
        Op::SubPct => format!("−{}%", (pct * 100.0).round() as i32),
        Op::AddFlat(amount) => format!("+{}", amount as i32),
    }
}

impl DraftOption {
    fn label(&self) -> String {
        match self {
            DraftOption::Equip(def) => format!("{} — New", def.name),
            DraftOption::AbilityUp { def, stat, tier } => {
                let s = &ABILITY_STATS[*stat];
                format!("{} — {} {} {}", def.name, tier.label, magnitude_label(s.op, tier.pct), s.label)
            }
            DraftOption::Global { kind, tier } => {
                let g = &GLOBAL_UPGRADES[*kind];
                format!("{} {} — {} {}", g.label, magnitude_label(g.op, tier.pct), tier.label, "(global)")
            }
        }
    }
}

/// The choices offered on the current level-up screen.
#[derive(Resource, Default)]
struct Draft {
    options: Vec<DraftOption>,
}

#[derive(Component)]
struct LevelUpUi;

/// A draft choice button for `Draft::options[index]`.
#[derive(Component, Clone, Copy)]
struct DraftButton(usize);

/// A dropped experience pickup worth `value` XP.
#[derive(Component)]
pub struct XpGem {
    value: u32,
}

/// Cached gem visuals.
#[derive(Resource)]
struct GemAssets {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

pub struct ProgressionPlugin;

impl Plugin for ProgressionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Experience>()
            .init_resource::<Draft>()
            .add_observer(drop_gem)
            .add_systems(Startup, setup_gem_assets)
            .add_systems(OnEnter(AppState::Playing), (reset_progress, despawn_gems))
            .add_systems(
                Update,
                (collect_gems, check_level_up)
                    .chain()
                    .run_if(in_state(PlayingState::Running)),
            )
            .add_systems(OnEnter(PlayingState::LevelUp), open_draft)
            .add_systems(OnExit(PlayingState::LevelUp), close_draft)
            .add_systems(
                Update,
                (draft_input, draft_button_clicks).run_if(in_state(PlayingState::LevelUp)),
            );
    }
}

fn reset_progress(mut xp: ResMut<Experience>) {
    *xp = Experience::default();
}

fn setup_gem_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(GemAssets {
        mesh: meshes.add(Sphere::new(0.18)),
        material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.3, 1.0, 0.7),
            emissive: LinearRgba::new(0.0, 4.0, 2.0, 1.0),
            ..default()
        }),
    });
}

/// Drop an XP gem where an enemy dies.
fn drop_gem(
    died: On<Died>,
    enemies: Query<&GlobalTransform, With<Enemy>>,
    assets: Res<GemAssets>,
    mut commands: Commands,
) {
    let Ok(transform) = enemies.get(died.entity) else {
        return;
    };
    let mut pos = transform.translation();
    pos.y = 0.4;
    commands.spawn((
        Name::new("XpGem"),
        XpGem { value: GEM_VALUE },
        Mesh3d(assets.mesh.clone()),
        MeshMaterial3d(assets.material.clone()),
        Transform::from_translation(pos),
    ));
}

/// Vacuum nearby gems toward the player and collect those that reach it.
fn collect_gems(
    time: Res<Time>,
    player: Query<(&GlobalTransform, &PickupRadius), With<Player>>,
    mut gems: Query<(Entity, &mut Transform, &XpGem)>,
    mut xp: ResMut<Experience>,
    mut commands: Commands,
) {
    let Ok((player_tf, radius)) = player.single() else {
        return;
    };
    let player_pos = player_tf.translation();
    for (entity, mut transform, gem) in &mut gems {
        let distance = transform.translation.distance(player_pos);
        if distance <= GEM_COLLECT_RADIUS {
            xp.current += gem.value;
            if let Ok(mut ec) = commands.get_entity(entity) {
                ec.try_despawn();
            }
        } else if distance <= radius.value {
            let dir = (player_pos - transform.translation).normalize_or_zero();
            transform.translation += dir * GEM_ATTRACT_SPEED * time.delta_secs();
        }
    }
}

fn despawn_gems(mut commands: Commands, gems: Query<Entity, With<XpGem>>) {
    for entity in &gems {
        if let Ok(mut ec) = commands.get_entity(entity) {
            ec.try_despawn();
        }
    }
}

/// Consume XP and level up, then open the draft. There's always at least one
/// owned ability to rank up (the starter), so a draft is always offered.
fn check_level_up(
    mut xp: ResMut<Experience>,
    mut next: ResMut<NextState<PlayingState>>,
) {
    if xp.current < xp.to_next {
        return;
    }
    xp.current -= xp.to_next;
    xp.level += 1;
    xp.to_next += 2;
    next.set(PlayingState::LevelUp);
}

/// Build the draft pool — equip options for empty slots, a freshly-rolled per-stat
/// rank-up for each stat an owned ability carries, and a rolled global passive for
/// each kind — then offer up to three distinct picks.
fn open_draft(
    mut draft: ResMut<Draft>,
    player: Query<&AbilitySlots, With<Player>>,
    mut commands: Commands,
) {
    let Ok(slots) = player.single() else {
        return;
    };
    let equipped: Vec<&'static AbilityDef> = slots.equipped().collect();
    let mut rng = rand::rng();

    let mut pool: Vec<DraftOption> = Vec::new();

    // Equip a not-yet-owned ability (only while a slot is free).
    if !slots.is_full() {
        for def in ALL {
            if !equipped.iter().any(|d| d.same(def)) {
                pool.push(DraftOption::Equip(def));
            }
        }
    }

    // Per-ability stat rank-ups — offer a stat only if the ability declares it
    // (only AoE abilities have Area, only projectile abilities have Speed, the
    // sustained blade has only Damage).
    for &def in &equipped {
        for idx in 0..ABILITY_STATS.len() {
            if ability_has_stat(def, idx) {
                pool.push(DraftOption::AbilityUp { def, stat: idx, tier: roll_tier(&mut rng) });
            }
        }
    }

    // Player-wide passives — one rolled option per kind.
    for idx in 0..GLOBAL_UPGRADES.len() {
        pool.push(DraftOption::Global { kind: idx, tier: roll_tier(&mut rng) });
    }

    // Partial Fisher–Yates: surface up to three distinct options.
    let offered = pool.len().min(3);
    for i in 0..offered {
        let j = rng.random_range(i..pool.len());
        pool.swap(i, j);
    }
    pool.truncate(offered);
    draft.options = pool;

    commands
        .spawn((
            LevelUpUi,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(14.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
        ))
        .with_children(|p| {
            p.spawn(title("Level Up! Choose an Upgrade"));
            for (i, option) in draft.options.iter().enumerate() {
                p.spawn(button(&option.label(), DraftButton(i)));
            }
        });
}

/// Resolve a draft pick: equip a new ability, apply a per-ability stat instant to
/// the live spell, or apply a global passive instant to the player. Returns once
/// handled so the caller leaves the draft.
fn pick_draft(
    index: usize,
    draft: &Draft,
    player_entity: Entity,
    slots: &mut AbilitySlots,
    q_ability: &Query<(Entity, &SlotAbility)>,
    attributes: &mut AttributesMut,
    next: &mut NextState<PlayingState>,
) {
    let Some(&option) = draft.options.get(index) else {
        return;
    };
    match option {
        DraftOption::Equip(def) => {
            slots.equip(def);
        }
        DraftOption::AbilityUp { def, stat, tier } => {
            if let Some((entity, _)) = q_ability.iter().find(|(_, slot)| slot.0.same(def)) {
                let s = &ABILITY_STATS[stat];
                apply_op(s.stat, s.op, tier.pct, entity, attributes);
            }
        }
        DraftOption::Global { kind, tier } => {
            let g = &GLOBAL_UPGRADES[kind];
            apply_op(g.stat, g.op, tier.pct, player_entity, attributes);
        }
    }
    next.set(PlayingState::Running);
}

fn draft_button_clicks(
    buttons: Query<(&Interaction, &DraftButton), (Changed<Interaction>, With<Button>)>,
    draft: Res<Draft>,
    mut player: Query<(Entity, &mut AbilitySlots), With<Player>>,
    q_ability: Query<(Entity, &SlotAbility)>,
    mut attributes: AttributesMut,
    mut next: ResMut<NextState<PlayingState>>,
) {
    for (interaction, draft_button) in &buttons {
        if *interaction == Interaction::Pressed {
            if let Ok((player_entity, mut slots)) = player.single_mut() {
                pick_draft(
                    draft_button.0,
                    &draft,
                    player_entity,
                    &mut slots,
                    &q_ability,
                    &mut attributes,
                    &mut next,
                );
            }
        }
    }
}

fn close_draft(mut commands: Commands, ui: Query<Entity, With<LevelUpUi>>) {
    for entity in &ui {
        if let Ok(mut ec) = commands.get_entity(entity) {
            ec.try_despawn();
        }
    }
}

fn draft_input(
    keys: Res<ButtonInput<KeyCode>>,
    draft: Res<Draft>,
    mut player: Query<(Entity, &mut AbilitySlots), With<Player>>,
    q_ability: Query<(Entity, &SlotAbility)>,
    mut attributes: AttributesMut,
    mut next: ResMut<NextState<PlayingState>>,
) {
    const DIGITS: [KeyCode; 3] = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3];
    for (i, key) in DIGITS.iter().enumerate() {
        if keys.just_pressed(*key) {
            if let Ok((player_entity, mut slots)) = player.single_mut() {
                pick_draft(i, &draft, player_entity, &mut slots, &q_ability, &mut attributes, &mut next);
            }
        }
    }
}
