//! Control bar with play/pause, speed, and info display

use bevy::prelude::*;

use crate::beatmap::BeatmapView;
use crate::playback::{PlaybackState, PlaybackStateRes};
use crate::rendering::ZoomLevel;
use crate::ui::UiFont;

pub struct ControlsPlugin;

impl Plugin for ControlsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_controls)
            .add_systems(Update, update_play_button)
            .add_systems(Update, update_speed_display)
            .add_systems(Update, update_object_count)
            .add_systems(Update, update_zoom_display)
            .add_systems(Update, handle_button_clicks)
            .add_systems(Update, handle_zoom_clicks);
    }
}

/// Marker for play/pause button
#[derive(Component)]
pub struct PlayPauseButton;

/// Marker for speed button
#[derive(Component)]
pub struct SpeedButton;

/// Marker for object count text
#[derive(Component)]
pub struct ObjectCountText;

/// Marker for audio status text
#[derive(Component)]
#[allow(dead_code)]
pub struct AudioStatusText;

/// Marker for zoom minus button
#[derive(Component)]
pub struct ZoomMinusButton;

/// Marker for zoom plus button
#[derive(Component)]
pub struct ZoomPlusButton;

/// Marker for zoom display text
#[derive(Component)]
pub struct ZoomDisplayText;

fn setup_controls(mut commands: Commands, beatmap: Res<BeatmapView>, ui_font: Res<UiFont>) {
    let font = ui_font.0.clone();

    // Control bar above timeline
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(70.0),
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                height: Val::Px(40.0),
                padding: UiRect::horizontal(Val::Px(10.0)),
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.06, 0.06, 0.08)),
        ))
        .with_children(|parent| {
            // Play/Pause button
            parent
                .spawn((
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(15.0), Val::Px(8.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.25, 0.25, 0.3)),
                    PlayPauseButton,
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("▶ Play"),
                        TextFont {
                            font: font.clone(),
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });

            // Separator
            parent.spawn((
                Node {
                    width: Val::Px(1.0),
                    height: Val::Px(20.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.3, 0.3, 0.35)),
            ));

            // Speed label
            parent.spawn((
                Text::new("Speed:"),
                TextFont {
                    font: font.clone(),
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::srgb(0.7, 0.7, 0.7)),
            ));

            // Speed button
            parent
                .spawn((
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.25, 0.25, 0.3)),
                    SpeedButton,
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("1.00x"),
                        TextFont {
                            font: font.clone(),
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });

            // Separator
            parent.spawn((
                Node {
                    width: Val::Px(1.0),
                    height: Val::Px(20.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.3, 0.3, 0.35)),
            ));

            // Object count
            parent.spawn((
                Text::new(format!("Objects: {}", beatmap.objects.len())),
                TextFont {
                    font: font.clone(),
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::srgb(0.7, 0.7, 0.7)),
                ObjectCountText,
            ));

            // Separator
            parent.spawn((
                Node {
                    width: Val::Px(1.0),
                    height: Val::Px(20.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.3, 0.3, 0.35)),
            ));

            // Zoom label
            parent.spawn((
                Text::new("Zoom:"),
                TextFont {
                    font: font.clone(),
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::srgb(0.7, 0.7, 0.7)),
            ));

            // Zoom minus button
            parent
                .spawn((
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.25, 0.25, 0.3)),
                    ZoomMinusButton,
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("-"),
                        TextFont {
                            font: font.clone(),
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });

            // Zoom display
            parent.spawn((
                Text::new("100%"),
                TextFont {
                    font: font.clone(),
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                ZoomDisplayText,
            ));

            // Zoom plus button
            parent
                .spawn((
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.25, 0.25, 0.3)),
                    ZoomPlusButton,
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("+"),
                        TextFont {
                            font: font.clone(),
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });

            // Spacer
            parent.spawn(Node {
                flex_grow: 1.0,
                ..default()
            });

            // Controls help
            parent.spawn((
                Text::new("Space: Play/Pause | ←/→: Seek | ↑/↓: Speed"),
                TextFont {
                    font: font.clone(),
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::srgb(0.5, 0.5, 0.5)),
            ));
        });
}

fn update_play_button(
    playback: Res<PlaybackStateRes>,
    query: Query<&Children, With<PlayPauseButton>>,
    mut text_query: Query<&mut Text>,
) {
    for children in query.iter() {
        for child in children.iter() {
            if let Ok(mut text) = text_query.get_mut(child) {
                text.0 = match playback.state {
                    PlaybackState::Playing => "⏸ Pause".to_string(),
                    _ => "▶ Play".to_string(),
                };
            }
        }
    }
}

fn update_speed_display(
    playback: Res<PlaybackStateRes>,
    query: Query<&Children, With<SpeedButton>>,
    mut text_query: Query<&mut Text>,
) {
    for children in query.iter() {
        for child in children.iter() {
            if let Ok(mut text) = text_query.get_mut(child) {
                text.0 = format!("{:.2}x", playback.speed);
            }
        }
    }
}

fn update_object_count(
    beatmap: Res<BeatmapView>,
    playback: Res<PlaybackStateRes>,
    mut query: Query<&mut Text, With<ObjectCountText>>,
) {
    let visible = beatmap.visible_objects(playback.current_time).len();
    let total = beatmap.objects.len();

    for mut text in query.iter_mut() {
        text.0 = format!("Objects: {} / {} visible", total, visible);
    }
}

fn handle_button_clicks(
    mut playback: ResMut<PlaybackStateRes>,
    mouse: Res<ButtonInput<MouseButton>>,
    play_query: Query<&Interaction, (Changed<Interaction>, With<PlayPauseButton>)>,
    speed_query: Query<&Interaction, With<SpeedButton>>,
) {
    for interaction in play_query.iter() {
        if *interaction == Interaction::Pressed {
            playback.toggle_play();
        }
    }

    // Speed button: left-click to speed up, right-click to slow down
    for interaction in speed_query.iter() {
        if *interaction == Interaction::Hovered || *interaction == Interaction::Pressed {
            if mouse.just_pressed(MouseButton::Left) {
                playback.cycle_speed();
            }
            if mouse.just_pressed(MouseButton::Right) {
                playback.cycle_speed_reverse();
            }
        }
    }
}

fn update_zoom_display(
    zoom: Res<ZoomLevel>,
    mut query: Query<&mut Text, With<ZoomDisplayText>>,
) {
    for mut text in query.iter_mut() {
        text.0 = format!("{:.0}%", zoom.level * 100.0);
    }
}

fn handle_zoom_clicks(
    mut zoom: ResMut<ZoomLevel>,
    minus_query: Query<&Interaction, (Changed<Interaction>, With<ZoomMinusButton>)>,
    plus_query: Query<&Interaction, (Changed<Interaction>, With<ZoomPlusButton>)>,
) {
    let step = 0.1;
    let min_zoom = 0.3;
    let max_zoom = 2.0;

    for interaction in minus_query.iter() {
        if *interaction == Interaction::Pressed {
            zoom.level = (zoom.level - step).max(min_zoom);
        }
    }

    for interaction in plus_query.iter() {
        if *interaction == Interaction::Pressed {
            zoom.level = (zoom.level + step).min(max_zoom);
        }
    }
}
