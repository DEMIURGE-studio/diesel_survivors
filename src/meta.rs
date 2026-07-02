//! Metaprogression: souls banked across runs buy permanent stat ranks that fold
//! into every future character's starting attributes. Persisted to a ron file.

use std::fs;

use bevy::prelude::*;
use bevy::scene::prelude::{bsn, CommandsSceneExt};
use diesel_avian3d::gauge::prelude::ModifierSet;
use serde::{Deserialize, Serialize};

use crate::attributes::Died;
use crate::enemy::Enemy;
use crate::stats::attr;
use crate::states::AppState;
use crate::ui::{button, label, screen, title, GotoState};

const SAVE_PATH: &str = "diesel_survivors_save.ron";

/// Per-rank bonuses applied to a fresh character's stats.
const VITALITY_PER_RANK: f32 = 2.0;
const DAMAGE_PER_RANK: f32 = 2.0;
const MOVE_SPEED_PER_RANK: f32 = 0.3;

/// Persistent player progress.
#[derive(Resource, Serialize, Deserialize, Default, Clone)]
pub struct MetaProgress {
    pub souls: u32,
    pub vitality_ranks: u32,
    pub damage_ranks: u32,
    pub move_speed_ranks: u32,
}

impl MetaProgress {
    /// Fold the purchased ranks into a character's starting modifier set.
    pub fn apply_to(&self, set: &mut ModifierSet) {
        if self.vitality_ranks > 0 {
            set.add(attr::VITALITY, self.vitality_ranks as f32 * VITALITY_PER_RANK);
        }
        if self.damage_ranks > 0 {
            set.add(attr::DAMAGE, self.damage_ranks as f32 * DAMAGE_PER_RANK);
        }
        if self.move_speed_ranks > 0 {
            set.add(attr::MOVE_SPEED, self.move_speed_ranks as f32 * MOVE_SPEED_PER_RANK);
        }
    }
}

/// The buyable upgrades.
#[derive(Clone, Copy)]
enum Upgrade {
    Vitality,
    Damage,
    MoveSpeed,
}

impl Upgrade {
    const ALL: [Upgrade; 3] = [Upgrade::Vitality, Upgrade::Damage, Upgrade::MoveSpeed];

    fn name(self) -> &'static str {
        match self {
            Upgrade::Vitality => "Vitality",
            Upgrade::Damage => "Damage",
            Upgrade::MoveSpeed => "Move Speed",
        }
    }

    fn ranks(self, meta: &MetaProgress) -> u32 {
        match self {
            Upgrade::Vitality => meta.vitality_ranks,
            Upgrade::Damage => meta.damage_ranks,
            Upgrade::MoveSpeed => meta.move_speed_ranks,
        }
    }

    /// Escalating cost: 10, 20, 30, ...
    fn cost(self, meta: &MetaProgress) -> u32 {
        (self.ranks(meta) + 1) * 10
    }

    /// Buy a rank if affordable; returns true on success.
    fn buy(self, meta: &mut MetaProgress) -> bool {
        let cost = self.cost(meta);
        if meta.souls < cost {
            return false;
        }
        meta.souls -= cost;
        match self {
            Upgrade::Vitality => meta.vitality_ranks += 1,
            Upgrade::Damage => meta.damage_ranks += 1,
            Upgrade::MoveSpeed => meta.move_speed_ranks += 1,
        }
        true
    }
}

fn load() -> MetaProgress {
    fs::read_to_string(SAVE_PATH)
        .ok()
        .and_then(|s| ron::from_str(&s).ok())
        .unwrap_or_default()
}

fn save(meta: &MetaProgress) {
    if let Ok(text) = ron::to_string(meta) {
        let _ = fs::write(SAVE_PATH, text);
    }
}

pub struct MetaPlugin;

impl Plugin for MetaPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(load())
            .add_observer(earn_souls)
            .add_systems(OnEnter(AppState::GameOver), save_on_death)
            .add_systems(OnEnter(AppState::Upgrades), spawn_upgrades_ui)
            .add_systems(
                Update,
                (upgrades_keys, buy_button_clicks, refresh_upgrade_labels)
                    .run_if(in_state(AppState::Upgrades)),
            );
    }
}

/// Each enemy killed banks a soul.
fn earn_souls(died: On<Died>, enemies: Query<(), With<Enemy>>, mut meta: ResMut<MetaProgress>) {
    if enemies.get(died.entity).is_ok() {
        meta.souls += 1;
    }
}

fn save_on_death(meta: Res<MetaProgress>) {
    save(&meta);
}

/// Header text showing the soul balance.
#[derive(Component, Clone, Copy, Default)]
struct SoulsHeader;

/// A buy button for `Upgrade::ALL[index]`; its own text shows rank + cost.
#[derive(Component, Clone, Copy)]
struct BuyButton(usize);

fn upgrade_label(index: usize, meta: &MetaProgress) -> String {
    let up = Upgrade::ALL[index];
    format!(
        "{}   rank {}   -   {} souls",
        up.name(),
        up.ranks(meta),
        up.cost(meta),
    )
}

fn spawn_upgrades_ui(mut commands: Commands, meta: Res<MetaProgress>) {
    let buys: Vec<_> = (0..Upgrade::ALL.len())
        .map(|i| button(&upgrade_label(i, &meta), BuyButton(i)))
        .collect();

    commands.spawn_scene(bsn! {
        screen(AppState::Upgrades)
        Children [
            title("Upgrades"),
            (
                label(&format!("Souls: {}", meta.souls), 26.0, Color::srgb(0.6, 1.0, 0.8))
                SoulsHeader
            ),
            { buys },
            button("Back", GotoState(AppState::MainMenu)),
            label("Click, or 1 / 2 / 3 to buy  |  Esc to go back", 18.0, Color::srgb(0.55, 0.55, 0.55)),
        ]
    });
}

/// Keep the soul header and each buy button's label in sync with the bank.
fn refresh_upgrade_labels(
    meta: Res<MetaProgress>,
    mut header: Query<&mut Text, (With<SoulsHeader>, Without<BuyButton>)>,
    mut buttons: Query<(&BuyButton, &mut Text), Without<SoulsHeader>>,
) {
    if let Ok(mut text) = header.single_mut() {
        *text = Text::new(format!("Souls: {}", meta.souls));
    }
    for (buy, mut text) in &mut buttons {
        *text = Text::new(upgrade_label(buy.0, &meta));
    }
}

fn try_buy(meta: &mut MetaProgress, index: usize) {
    if Upgrade::ALL[index].buy(meta) {
        save(meta);
    }
}

fn upgrades_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut meta: ResMut<MetaProgress>,
    mut next: ResMut<NextState<AppState>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        next.set(AppState::MainMenu);
        return;
    }
    const DIGITS: [KeyCode; 3] = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3];
    for (i, key) in DIGITS.iter().enumerate() {
        if keys.just_pressed(*key) {
            try_buy(&mut meta, i);
        }
    }
}

fn buy_button_clicks(
    buttons: Query<(&Interaction, &BuyButton), (Changed<Interaction>, With<Button>)>,
    mut meta: ResMut<MetaProgress>,
) {
    for (interaction, buy) in &buttons {
        if *interaction == Interaction::Pressed {
            try_buy(&mut meta, buy.0);
        }
    }
}
