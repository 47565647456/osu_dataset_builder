//! Playback state management

/// Playback state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Playing,
    Paused,
    Stopped,
}

/// Manages playback timing and state
pub struct PlaybackManager {
    /// Current state
    pub state: PlaybackState,
    /// Current time in milliseconds
    pub current_time: f64,
    /// Playback speed multiplier
    pub speed: f64,
    /// Total duration in milliseconds
    pub total_duration: f64,
    /// Last update timestamp for manual time tracking
    last_update: std::time::Instant,
}

impl PlaybackManager {
    pub fn new(total_duration: f64) -> Self {
        Self {
            state: PlaybackState::Paused,
            current_time: 0.0,
            speed: 1.0,
            total_duration,
            last_update: std::time::Instant::now(),
        }
    }

    /// Update time when not using audio (manual tracking)
    pub fn update_manual(&mut self) {
        if self.state == PlaybackState::Playing {
            let now = std::time::Instant::now();
            let delta = now.duration_since(self.last_update).as_secs_f64() * 1000.0 * self.speed;
            self.current_time = (self.current_time + delta).min(self.total_duration);
            self.last_update = now;

            // Auto-pause at end
            if self.current_time >= self.total_duration {
                self.state = PlaybackState::Paused;
            }
        } else {
            self.last_update = std::time::Instant::now();
        }
    }

    /// Sync time from audio player
    pub fn sync_from_audio(&mut self, audio_time: f64) {
        self.current_time = audio_time;
    }

    /// Toggle play/pause
    pub fn toggle_play(&mut self) {
        self.state = match self.state {
            PlaybackState::Playing => PlaybackState::Paused,
            PlaybackState::Paused => PlaybackState::Playing,
            PlaybackState::Stopped => {
                self.current_time = 0.0;
                PlaybackState::Playing
            }
        };
        self.last_update = std::time::Instant::now();
    }

    /// Seek to a specific time
    pub fn seek(&mut self, time: f64) {
        self.current_time = time.clamp(0.0, self.total_duration);
        self.last_update = std::time::Instant::now();
    }

    /// Seek by a delta (positive = forward, negative = backward)
    pub fn seek_delta(&mut self, delta_ms: f64) {
        self.seek(self.current_time + delta_ms);
    }

    /// Set playback speed
    pub fn set_speed(&mut self, speed: f64) {
        self.speed = speed.clamp(0.25, 4.0);
    }

    /// Cycle through common speed values
    pub fn cycle_speed(&mut self) {
        self.speed = match self.speed {
            s if s < 0.5 => 0.5,
            s if s < 0.75 => 0.75,
            s if s < 1.0 => 1.0,
            s if s < 1.5 => 1.5,
            s if s < 2.0 => 2.0,
            _ => 0.5,
        };
    }

    /// Get formatted time string (MM:SS.ms)
    pub fn format_time(&self, time_ms: f64) -> String {
        let total_secs = (time_ms / 1000.0).max(0.0);
        let minutes = (total_secs / 60.0) as u32;
        let seconds = total_secs % 60.0;
        format!("{:02}:{:05.2}", minutes, seconds)
    }

    /// Get progress as 0.0 to 1.0
    pub fn progress(&self) -> f32 {
        if self.total_duration > 0.0 {
            (self.current_time / self.total_duration) as f32
        } else {
            0.0
        }
    }
}
