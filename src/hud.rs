//! In-game HUD: a health bar, level/XP readout, and souls counter, shown for the
//! duration of a Playing session.

use bevy::prelude::*;

use crate::attributes::Health;
use crate::meta::MetaProgress;
use crate::player::Player;
use crate::progression::Experience;
use crate::states::AppState;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Playing), spawn_hud)
            .add_systems(
                Update,
                (update_health_bar, update_xp_text, update_souls_text)
                    .run_if(in_state(AppState::Playing)),
            );
    }
}

#[derive(Component)]
struct HealthBarFill;

#[derive(Component)]
struct XpText;

#[derive(Component)]
struct SoulsText;

fn spawn_hud(mut commands: Commands) {
    commands
        .spawn((
            DespawnOnExit(AppState::Playing),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(12.0),
                left: Val::Px(12.0),
                width: Val::Px(260.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..default()
            },
        ))
        .with_children(|p| {
            // Health bar: dark track with a red fill whose width tracks HP.
            p.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(22.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            ))
            .with_children(|bar| {
                bar.spawn((
                    HealthBarFill,
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.85, 0.2, 0.2)),
                ));
            });
            p.spawn((
                XpText,
                Text::new("Lv 1"),
                TextFont {
                    font_size: FontSize::Px(20.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
            p.spawn((
                SoulsText,
                Text::new("Souls: 0"),
                TextFont {
                    font_size: FontSize::Px(18.0),
                    ..default()
                },
                TextColor(Color::srgb(0.6, 1.0, 0.8)),
            ));
        });
}

fn update_health_bar(
    player: Query<&Health, With<Player>>,
    mut fill: Query<&mut Node, With<HealthBarFill>>,
) {
    let Ok(health) = player.single() else {
        return;
    };
    let frac = if health.max > 0.0 {
        (health.current / health.max).clamp(0.0, 1.0)
    } else {
        0.0
    };
    for mut node in &mut fill {
        node.width = Val::Percent(frac * 100.0);
    }
}

fn update_xp_text(
    xp: Res<Experience>,
    player: Query<&Health, With<Player>>,
    mut text: Query<&mut Text, With<XpText>>,
) {
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let hp = player
        .single()
        .map(|h| format!("   HP {:.0}/{:.0}", h.current.max(0.0), h.max))
        .unwrap_or_default();
    *text = Text::new(format!("Lv {}   XP {}/{}{}", xp.level, xp.current, xp.to_next, hp));
}

fn update_souls_text(meta: Res<MetaProgress>, mut text: Query<&mut Text, With<SoulsText>>) {
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    *text = Text::new(format!("Souls: {}", meta.souls));
}
