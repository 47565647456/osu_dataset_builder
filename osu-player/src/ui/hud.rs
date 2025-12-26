//! HUD elements: combo counter, map stats, FPS graph

use bevy::prelude::*;
use std::collections::VecDeque;
use std::time::Instant;

use crate::beatmap::BeatmapView;
use crate::playback::PlaybackStateRes;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FrametimeTracker>()
            .add_systems(Startup, setup_hud)
            .add_systems(Update, update_combo_counter)
            .add_systems(Update, update_fps_display)
            .add_systems(Update, track_frametime);
    }
}

const FRAMETIME_HISTORY: usize = 60;

/// Resource for tracking frametime
#[derive(Resource)]
pub struct FrametimeTracker {
    pub history: VecDeque<f32>,
    pub last_frame: Instant,
}

impl Default for FrametimeTracker {
    fn default() -> Self {
        Self {
            history: VecDeque::with_capacity(FRAMETIME_HISTORY),
            last_frame: Instant::now(),
        }
    }
}

/// Marker for combo counter text
#[derive(Component)]
pub struct ComboCounterText;

/// Marker for map stats container
#[derive(Component)]
pub struct MapStatsContainer;

/// Marker for FPS display text
#[derive(Component)]
pub struct FpsText;

fn setup_hud(mut commands: Commands, beatmap: Res<BeatmapView>) {
    let bm = &beatmap.beatmap;

    // Combo counter (top-left)
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                left: Val::Px(10.0),
                padding: UiRect::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("0 / 0x"),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                ComboCounterText,
            ));
        });

    // Map stats (below combo)
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(50.0),
                left: Val::Px(10.0),
                padding: UiRect::all(Val::Px(6.0)),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
            MapStatsContainer,
        ))
        .with_children(|parent| {
            let bpm = bm
                .control_points
                .timing_points
                .first()
                .map(|tp| 60000.0 / tp.beat_len)
                .unwrap_or(0.0);

            let stats = [
                format!("AR: {:.1}", bm.approach_rate),
                format!("CS: {:.1}", bm.circle_size),
                format!("OD: {:.1}", bm.overall_difficulty),
                format!("HP: {:.1}", bm.hp_drain_rate),
                format!("BPM: {:.0}", bpm),
            ];

            for stat in stats {
                parent.spawn((
                    Text::new(stat),
                    TextFont {
                        font_size: 11.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            }
        });

    // FPS display (top-right)
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                right: Val::Px(10.0),
                padding: UiRect::all(Val::Px(6.0)),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("FPS: 0"),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                FpsText,
            ));
        });
}

fn update_combo_counter(
    beatmap: Res<BeatmapView>,
    playback: Res<PlaybackStateRes>,
    mut query: Query<&mut Text, With<ComboCounterText>>,
) {
    let current = beatmap.get_current_combo(playback.current_time);
    let total = beatmap.total_combo;

    for mut text in query.iter_mut() {
        text.0 = format!("{} / {}x", current, total);
    }
}

fn track_frametime(mut tracker: ResMut<FrametimeTracker>) {
    let now = Instant::now();
    let frametime = now.duration_since(tracker.last_frame).as_secs_f32() * 1000.0;
    tracker.last_frame = now;

    if tracker.history.len() >= FRAMETIME_HISTORY {
        tracker.history.pop_front();
    }
    tracker.history.push_back(frametime);
}

fn update_fps_display(
    tracker: Res<FrametimeTracker>,
    mut query: Query<&mut Text, With<FpsText>>,
) {
    if tracker.history.is_empty() {
        return;
    }

    let avg_ft: f32 = tracker.history.iter().sum::<f32>() / tracker.history.len() as f32;
    let fps = if avg_ft > 0.0 { 1000.0 / avg_ft } else { 0.0 };

    // Calculate 1% low
    let mut sorted: Vec<f32> = tracker.history.iter().copied().collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let idx = ((sorted.len() as f32) * 0.99) as usize;
    let idx = idx.min(sorted.len().saturating_sub(1));
    let one_percent_low_ft = sorted.get(idx).copied().unwrap_or(avg_ft);
    let one_percent_low = if one_percent_low_ft > 0.0 {
        1000.0 / one_percent_low_ft
    } else {
        0.0
    };

    for mut text in query.iter_mut() {
        text.0 = format!(
            "FPS: {:.0}\nAvg: {:.0} | 1%: {:.0}\n{:.2}ms",
            fps, fps, one_percent_low, avg_ft
        );
    }
}
