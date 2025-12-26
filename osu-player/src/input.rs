//! Input handling

use bevy::prelude::*;

use crate::playback::PlaybackStateRes;

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, handle_keyboard_input);
    }
}

/// System to handle keyboard input
fn handle_keyboard_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut playback: ResMut<PlaybackStateRes>,
) {
    // Space: toggle play/pause
    if keyboard.just_pressed(KeyCode::Space) {
        playback.toggle_play();
    }

    // Left/Right: seek
    if keyboard.just_pressed(KeyCode::ArrowLeft) {
        playback.seek_delta(-5000.0); // -5 seconds
    }
    if keyboard.just_pressed(KeyCode::ArrowRight) {
        playback.seek_delta(5000.0); // +5 seconds
    }

    // Up/Down: playback speed
    if keyboard.just_pressed(KeyCode::ArrowUp) {
        let current_speed = playback.speed;
        playback.set_speed(current_speed + 0.25);
    }
    if keyboard.just_pressed(KeyCode::ArrowDown) {
        let current_speed = playback.speed;
        playback.set_speed(current_speed - 0.25);
    }

    // Home: go to start
    if keyboard.just_pressed(KeyCode::Home) {
        playback.seek(0.0);
    }

    // End: go to end
    if keyboard.just_pressed(KeyCode::End) {
        let total = playback.total_duration;
        playback.seek(total - 1000.0);
    }
}
