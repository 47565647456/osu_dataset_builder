//! Timeline with density minimap and scrubbing

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use crate::beatmap::BeatmapView;
use crate::playback::PlaybackStateRes;
use crate::ui::UiFont;

pub struct TimelinePlugin;

impl Plugin for TimelinePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TimelineDensity>()
            .add_systems(Startup, setup_timeline)
            .add_systems(Update, spawn_timing_markers.run_if(resource_changed::<BeatmapView>))
            .add_systems(Update, compute_density.run_if(resource_changed::<BeatmapView>))
            .add_systems(Update, update_timeline)
            .add_systems(Update, handle_timeline_click);
    }
}

const NUM_BUCKETS: usize = 100;

/// Cached density data
#[derive(Resource, Default)]
pub struct TimelineDensity {
    pub buckets: Vec<f32>,
}

/// Marker for timeline container
#[derive(Component)]
pub struct TimelineContainer;

/// Marker for timeline track background (the clickable area)
#[derive(Component)]
pub struct TimelineTrack;

/// Marker for timeline progress fill
#[derive(Component)]
pub struct TimelineProgress;

/// Marker for timeline playhead
#[derive(Component)]
pub struct TimelinePlayhead;

/// Marker for current time text
#[derive(Component)]
pub struct CurrentTimeText;

/// Marker for total time text
#[derive(Component)]
pub struct TotalTimeText;

/// Marker for density bar
#[derive(Component)]
pub struct DensityBar(pub usize);

/// Marker for timing point lines
#[derive(Component)]
pub struct TimingPointMarker;

fn setup_timeline(mut commands: Commands, ui_font: Res<UiFont>) {
    let font = ui_font.0.clone();

    // Timeline container at bottom
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(0.0),
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                height: Val::Px(70.0),
                padding: UiRect::all(Val::Px(10.0)),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(Color::srgb(0.12, 0.12, 0.16)),
            Interaction::default(),
            TimelineContainer,
        ))
        .with_children(|parent| {
            // Density minimap row
            parent
                .spawn(Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(20.0),
                    margin: UiRect::bottom(Val::Px(5.0)),
                    padding: UiRect::horizontal(Val::Px(80.0)), // Align with scrubber track (70px label + 10px margin)
                    justify_content: JustifyContent::SpaceBetween,
                    ..default()
                })
                .with_children(|minimap| {
                    // Density bars
                    for i in 0..NUM_BUCKETS {
                        minimap.spawn((
                            Node {
                                width: Val::Percent(100.0 / NUM_BUCKETS as f32),
                                height: Val::Percent(0.0),
                                align_self: AlignSelf::FlexEnd,
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.4, 0.6, 1.0)),
                            DensityBar(i),
                        ));
                    }
                });

            // Scrubber row
            parent
                .spawn(Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(30.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|scrubber| {
                    // Current time label
                    scrubber.spawn((
                        Text::new("00:00.00"),
                        TextFont {
                            font: font.clone(),
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                        Node {
                            width: Val::Px(70.0),
                            ..default()
                        },
                        CurrentTimeText,
                    ));

                    // Track background - clickable area with RelativeCursorPosition
                    scrubber.spawn((
                        Node {
                            flex_grow: 1.0,
                            height: Val::Px(24.0),
                            margin: UiRect::horizontal(Val::Px(10.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.3, 0.3, 0.35)),
                        TimelineTrack,
                        Interaction::None,
                        RelativeCursorPosition::default(),
                    )).with_children(|track| {
                        // Progress fill
                        track.spawn((
                            Node {
                                width: Val::Percent(0.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.4, 0.6, 1.0)),
                            TimelineProgress,
                        ));

                        // Playhead
                        track.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                width: Val::Px(4.0),
                                height: Val::Px(32.0),
                                left: Val::Percent(0.0),
                                top: Val::Px(-4.0),
                                ..default()
                            },
                            BackgroundColor(Color::WHITE),
                            TimelinePlayhead,
                        ));
                    });

                    // Total time label
                    scrubber.spawn((
                        Text::new("00:00.00"),
                        TextFont {
                            font: font.clone(),
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.6, 0.6, 0.6)),
                        Node {
                            width: Val::Px(70.0),
                            ..default()
                        },
                        TotalTimeText,
                    ));
                });
        });
}

fn compute_density(beatmap: Res<BeatmapView>, mut density: ResMut<TimelineDensity>) {
    if beatmap.total_duration <= 0.0 {
        return;
    }
    
    let bucket_duration = beatmap.total_duration / NUM_BUCKETS as f64;
    let mut buckets = vec![0u32; NUM_BUCKETS];

    for obj in &beatmap.objects {
        let start_bucket = ((obj.start_time / bucket_duration) as usize).min(NUM_BUCKETS - 1);
        let end_bucket = ((obj.end_time / bucket_duration) as usize).min(NUM_BUCKETS - 1);

        for bucket in start_bucket..=end_bucket {
            buckets[bucket] += 1;
        }
    }

    let max_density = buckets.iter().copied().max().unwrap_or(1) as f32;
    density.buckets = buckets
        .into_iter()
        .map(|count| count as f32 / max_density)
        .collect();
}

/// Spawn timing point markers on the timeline
fn spawn_timing_markers(
    mut commands: Commands,
    beatmap: Res<BeatmapView>,
    track_query: Query<Entity, With<TimelineTrack>>,
    existing_markers: Query<Entity, With<TimingPointMarker>>,
) {
    // Remove existing markers
    for entity in existing_markers.iter() {
        commands.entity(entity).despawn();
    }

    if beatmap.total_duration <= 0.0 {
        return;
    }

    // Get the timeline track entity
    let track_entity = match track_query.iter().next() {
        Some(e) => e,
        None => return,
    };

    // Spawn timing point markers as children of the track
    commands.entity(track_entity).with_children(|track| {
        // Red markers for timing points (BPM changes/sections)
        for timing_point in &beatmap.beatmap.control_points.timing_points {
            let time_ms = timing_point.time;
            let progress = (time_ms / beatmap.total_duration).clamp(0.0, 1.0) as f32;

            track.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(2.0),
                    height: Val::Percent(100.0),
                    left: Val::Percent(progress * 100.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(1.0, 0.3, 0.3)),
                TimingPointMarker,
            ));
        }

        // Yellow markers for Kiai sections
        for effect_point in &beatmap.beatmap.control_points.effect_points {
            if effect_point.kiai {
                let time_ms = effect_point.time;
                let progress = (time_ms / beatmap.total_duration).clamp(0.0, 1.0) as f32;

                track.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        width: Val::Px(2.0),
                        height: Val::Percent(60.0), // Shorter than timing markers
                        top: Val::Percent(20.0),
                        left: Val::Percent(progress * 100.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(1.0, 1.0, 0.3)), // Yellow for Kiai
                    TimingPointMarker,
                ));
            }
        }
    });
}

fn update_timeline(
    playback: Res<PlaybackStateRes>,
    density: Res<TimelineDensity>,
    mut current_time_query: Query<&mut Text, (With<CurrentTimeText>, Without<TotalTimeText>)>,
    mut total_time_query: Query<&mut Text, (With<TotalTimeText>, Without<CurrentTimeText>)>,
    mut progress_query: Query<&mut Node, (With<TimelineProgress>, Without<TimelinePlayhead>, Without<DensityBar>)>,
    mut playhead_query: Query<&mut Node, (With<TimelinePlayhead>, Without<TimelineProgress>, Without<DensityBar>)>,
    mut density_query: Query<(&mut Node, &mut BackgroundColor, &DensityBar), (Without<TimelineProgress>, Without<TimelinePlayhead>)>,
) {
    let progress = playback.progress();

    // Update time labels
    for mut text in current_time_query.iter_mut() {
        text.0 = PlaybackStateRes::format_time(playback.current_time);
    }

    for mut text in total_time_query.iter_mut() {
        text.0 = PlaybackStateRes::format_time(playback.total_duration);
    }

    // Update progress bar
    for mut node in progress_query.iter_mut() {
        node.width = Val::Percent(progress * 100.0);
    }

    // Update playhead position
    for mut node in playhead_query.iter_mut() {
        node.left = Val::Percent((progress * 100.0).max(0.0).min(99.0));
    }

    // Update density bars with color gradient
    for (mut node, mut bg_color, bar) in density_query.iter_mut() {
        if let Some(&d) = density.buckets.get(bar.0) {
            node.height = Val::Percent(d * 100.0);
            
            // Color gradient: low = blue, medium = yellow, high = red
            let color = density_to_color(d);
            bg_color.0 = color;
        }
    }
}

/// Convert density value (0-1) to a color gradient
/// Low = blue, Medium = yellow/green, High = red/orange
fn density_to_color(density: f32) -> Color {
    if density < 0.33 {
        // Low: blue to cyan
        let t = density / 0.33;
        Color::srgb(0.2, 0.4 + t * 0.3, 0.8 + t * 0.2)
    } else if density < 0.66 {
        // Medium: cyan to yellow/green
        let t = (density - 0.33) / 0.33;
        Color::srgb(0.2 + t * 0.6, 0.7 + t * 0.1, 1.0 - t * 0.6)
    } else {
        // High: yellow/orange to red
        let t = (density - 0.66) / 0.34;
        Color::srgb(0.8 + t * 0.2, 0.8 - t * 0.5, 0.4 - t * 0.3)
    }
}

/// Handle mouse clicks on timeline using RelativeCursorPosition
fn handle_timeline_click(
    mut playback: ResMut<PlaybackStateRes>,
    track_query: Query<(&Interaction, &RelativeCursorPosition), (With<TimelineTrack>, Changed<Interaction>)>,
    track_hold_query: Query<(&Interaction, &RelativeCursorPosition), With<TimelineTrack>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
) {
    // Handle initial click (Interaction changed to Pressed)
    for (interaction, relative_cursor) in track_query.iter() {
        if *interaction == Interaction::Pressed {
            if let Some(relative_pos) = relative_cursor.normalized {
                // RelativeCursorPosition uses center origin: (-0.5, -0.5) to (0.5, 0.5)
                // Convert to 0-1 range by adding 0.5
                let progress = (relative_pos.x + 0.5).clamp(0.0, 1.0);
                let seek_time = progress as f64 * playback.total_duration;
                playback.seek(seek_time);
                log::info!("Timeline clicked at {:.1}%", progress * 100.0);
            }
        }
    }

    // Handle dragging (mouse held down while hovering)
    if mouse_button.pressed(MouseButton::Left) {
        for (interaction, relative_cursor) in track_hold_query.iter() {
            if *interaction == Interaction::Pressed {
                if let Some(relative_pos) = relative_cursor.normalized {
                    // Convert center-origin to 0-1 range
                    let progress = (relative_pos.x + 0.5).clamp(0.0, 1.0);
                    let seek_time = progress as f64 * playback.total_duration;
                    playback.seek(seek_time);
                }
            }
        }
    }
}
