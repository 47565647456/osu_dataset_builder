//! Input handling

use bevy::prelude::*;

use crate::playback::PlaybackStateRes;

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SeekConfig>()
            .add_systems(Update, handle_keyboard_input);
    }
}

/// Configuration for seek behavior
#[derive(Resource)]
pub struct SeekConfig {
    /// Seek delta in milliseconds
    pub seek_delta_ms: f64,
    /// Available seek options (in ms)
    pub options: Vec<f64>,
    /// Current option index
    pub current_option: usize,
}

impl Default for SeekConfig {
    fn default() -> Self {
        let options = vec![1000.0, 2000.0, 5000.0, 10000.0];
        Self {
            seek_delta_ms: 5000.0,
            options,
            current_option: 2, // Default to 5s
        }
    }
}

impl SeekConfig {
    /// Cycle to the next seek option
    pub fn cycle_next(&mut self) {
        self.current_option = (self.current_option + 1) % self.options.len();
        self.seek_delta_ms = self.options[self.current_option];
    }

    /// Get formatted string for current seek length
    pub fn format_current(&self) -> String {
        let secs = self.seek_delta_ms / 1000.0;
        if secs == 1.0 {
            "1s".to_string()
        } else {
            format!("{:.0}s", secs)
        }
    }
}

/// System to handle keyboard input
fn handle_keyboard_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut playback: ResMut<PlaybackStateRes>,
    seek_config: Res<SeekConfig>,
    time: Res<Time>,
    mut seek_timer: Local<f32>,
) {
    // Space: toggle play/pause
    if keyboard.just_pressed(KeyCode::Space) {
        playback.toggle_play();
    }

    // Seek rate limiting: allow initial press, then limit to ~10 seeks per second when held
    let seek_interval = 0.1; // 100ms between seeks when holding

    // Left/Right: seek (supports holding)
    let seeking_left = keyboard.pressed(KeyCode::ArrowLeft);
    let seeking_right = keyboard.pressed(KeyCode::ArrowRight);

    if seeking_left || seeking_right {
        let just_pressed = keyboard.just_pressed(KeyCode::ArrowLeft) 
            || keyboard.just_pressed(KeyCode::ArrowRight);

        if just_pressed {
            // Immediate seek on first press
            *seek_timer = 0.0;
            if seeking_left {
                playback.seek_delta(-seek_config.seek_delta_ms);
            } else {
                playback.seek_delta(seek_config.seek_delta_ms);
            }
        } else {
            // Rate-limited seek while holding
            *seek_timer += time.delta_secs();
            if *seek_timer >= seek_interval {
                *seek_timer = 0.0;
                if seeking_left {
                    playback.seek_delta(-seek_config.seek_delta_ms);
                } else {
                    playback.seek_delta(seek_config.seek_delta_ms);
                }
            }
        }
    } else {
        *seek_timer = 0.0;
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

    // R: toggle reverse
    if keyboard.just_pressed(KeyCode::KeyR) {
        playback.toggle_reverse();
    }
}
