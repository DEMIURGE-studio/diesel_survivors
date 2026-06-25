//! Leveling and the ability draft. Kills grant XP; hitting the threshold pauses
//! into `PlayingState::LevelUp` and offers the player an unequipped ability to
//! slot. Equipping mutates `AbilitySlots`, which the equip system reconciles into
//! a live ability entity.

use bevy::prelude::*;

use crate::ability::{AbilityId, AbilitySlots};
use crate::attributes::{Died, PickupRadius};
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

/// The abilities offered on the current level-up screen.
#[derive(Resource, Default)]
struct Draft {
    options: Vec<AbilityId>,
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

/// Consume XP and level up. Only pauses for a draft when a slot is free (with a
/// 3-ability pool, a free slot guarantees at least one unequipped option).
fn check_level_up(
    mut xp: ResMut<Experience>,
    player: Query<&AbilitySlots, With<Player>>,
    mut next: ResMut<NextState<PlayingState>>,
) {
    if xp.current < xp.to_next {
        return;
    }
    xp.current -= xp.to_next;
    xp.level += 1;
    xp.to_next += 2;

    let slot_free = player.single().map(|s| !s.is_full()).unwrap_or(false);
    if slot_free {
        next.set(PlayingState::LevelUp);
    }
}

fn open_draft(
    mut draft: ResMut<Draft>,
    player: Query<&AbilitySlots, With<Player>>,
    mut commands: Commands,
) {
    let equipped: Vec<AbilityId> = player
        .single()
        .map(|s| s.equipped().collect())
        .unwrap_or_default();
    draft.options = AbilityId::ALL
        .into_iter()
        .filter(|id| !equipped.contains(id))
        .collect();

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
            p.spawn(title("Level Up! Choose an Ability"));
            for (i, id) in draft.options.iter().enumerate() {
                p.spawn(button(id.name(), DraftButton(i)));
            }
        });
}

fn pick_draft(slots: &mut AbilitySlots, draft: &Draft, index: usize, next: &mut NextState<PlayingState>) {
    if let Some(id) = draft.options.get(index) {
        slots.equip(*id);
        next.set(PlayingState::Running);
    }
}

fn draft_button_clicks(
    buttons: Query<(&Interaction, &DraftButton), (Changed<Interaction>, With<Button>)>,
    draft: Res<Draft>,
    mut player: Query<&mut AbilitySlots, With<Player>>,
    mut next: ResMut<NextState<PlayingState>>,
) {
    for (interaction, draft_button) in &buttons {
        if *interaction == Interaction::Pressed {
            if let Ok(mut slots) = player.single_mut() {
                pick_draft(&mut slots, &draft, draft_button.0, &mut next);
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
    mut player: Query<&mut AbilitySlots, With<Player>>,
    mut next: ResMut<NextState<PlayingState>>,
) {
    const DIGITS: [KeyCode; 3] = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3];
    for (i, key) in DIGITS.iter().enumerate() {
        if keys.just_pressed(*key) {
            if let Ok(mut slots) = player.single_mut() {
                pick_draft(&mut slots, &draft, i, &mut next);
            }
        }
    }
}
