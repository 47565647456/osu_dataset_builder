//! Countdown and break overlays

use bevy::prelude::*;

use crate::beatmap::{BeatmapView, CountdownState};
use crate::playback::PlaybackStateRes;
use crate::ui::UiFont;

pub struct OverlaysPlugin;

impl Plugin for OverlaysPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_overlays)
            .add_systems(Update, update_countdown)
            .add_systems(Update, update_break_indicator);
    }
}

/// Marker for countdown text
#[derive(Component)]
pub struct CountdownText;

/// Marker for break indicator container
#[derive(Component)]
pub struct BreakIndicator;

/// Marker for break text
#[derive(Component)]
pub struct BreakText;

/// Marker for break progress bar background
#[derive(Component)]
pub struct BreakProgressBg;

/// Marker for break progress bar fill
#[derive(Component)]
pub struct BreakProgressFill;

/// Marker for break time remaining text
#[derive(Component)]
pub struct BreakTimeText;

fn setup_overlays(mut commands: Commands, ui_font: Res<UiFont>) {
    let font = ui_font.0.clone();

    // Countdown container (centered)
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(35.0),
            left: Val::Percent(0.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
    )).with_children(|parent| {
        parent.spawn((
            Text::new(""),
            TextFont {
                font: font.clone(),
                font_size: 120.0,
                ..default()
            },
            TextColor(Color::WHITE),
            CountdownText,
        ));
    });

    // Break indicator container (centered at top)
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(60.0),
                left: Val::Percent(0.0),
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                display: Display::None,
                ..default()
            },
            BreakIndicator,
        ))
        .with_children(|parent| {
            // Break text
            parent.spawn((
                Text::new("Break"),
                TextFont {
                    font: font.clone(),
                    font_size: 36.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                TextLayout::new_with_justify(Justify::Center),
                BreakText,
            ));

            // Progress bar background
            parent.spawn((
                Node {
                    width: Val::Px(200.0),
                    height: Val::Px(6.0),
                    margin: UiRect::top(Val::Px(10.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.16, 0.16, 0.16, 0.8)),
                BreakProgressBg,
            )).with_children(|progress_parent| {
                // Progress bar fill
                progress_parent.spawn((
                    Node {
                        width: Val::Percent(0.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(Color::WHITE),
                    BreakProgressFill,
                ));
            });

            // Time remaining
            parent.spawn((
                Text::new("0s"),
                TextFont {
                    font: font.clone(),
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::srgb(0.7, 0.7, 0.7)),
                Node {
                    margin: UiRect::top(Val::Px(5.0)),
                    ..default()
                },
                TextLayout::new_with_justify(Justify::Center),
                BreakTimeText,
            ));
        });
}

fn update_countdown(
    beatmap: Res<BeatmapView>,
    playback: Res<PlaybackStateRes>,
    mut query: Query<(&mut Text, &mut TextColor), With<CountdownText>>,
) {
    let state = beatmap.get_countdown_state(playback.current_time);

    for (mut text, mut color) in query.iter_mut() {
        let (content, col) = match state {
            CountdownState::None => (String::new(), Color::WHITE),
            CountdownState::Number(n) => {
                let c = match n {
                    3 => Color::srgb(1.0, 0.4, 0.4),
                    2 => Color::srgb(1.0, 0.8, 0.4),
                    1 => Color::srgb(0.4, 1.0, 0.4),
                    _ => Color::WHITE,
                };
                (n.to_string(), c)
            }
            CountdownState::Go => ("Go!".to_string(), Color::srgb(0.4, 0.8, 1.0)),
        };

        text.0 = content;
        color.0 = col;
    }
}

fn update_break_indicator(
    beatmap: Res<BeatmapView>,
    playback: Res<PlaybackStateRes>,
    mut container_query: Query<&mut Node, With<BreakIndicator>>,
    mut progress_query: Query<&mut Node, (With<BreakProgressFill>, Without<BreakIndicator>)>,
    mut time_query: Query<&mut Text, With<BreakTimeText>>,
) {
    let current_time = playback.current_time;

    if let Some(break_period) = beatmap.is_in_break(current_time) {
        let break_duration = break_period.end_time - break_period.start_time;

        // Only show for long breaks
        if break_duration < 2000.0 {
            for mut node in container_query.iter_mut() {
                node.display = Display::None;
            }
            return;
        }

        let time_in_break = current_time - break_period.start_time;
        let time_remaining = break_period.end_time - current_time;
        let progress = (time_in_break / break_duration) as f32;

        // Show container
        for mut node in container_query.iter_mut() {
            node.display = Display::Flex;
        }

        // Update progress bar
        for mut node in progress_query.iter_mut() {
            node.width = Val::Percent(progress * 100.0);
        }

        // Update time remaining
        let seconds_remaining = (time_remaining / 1000.0).ceil() as i32;
        for mut text in time_query.iter_mut() {
            text.0 = format!("{}s", seconds_remaining);
        }
    } else {
        // Hide container
        for mut node in container_query.iter_mut() {
            node.display = Display::None;
        }
    }
}
