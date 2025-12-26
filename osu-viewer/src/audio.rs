//! Audio playback using kira

use anyhow::{Context, Result};
use kira::{
    AudioManager, AudioManagerSettings, DefaultBackend,
    sound::static_sound::{StaticSoundData, StaticSoundHandle},
    Tween, Decibels,
};
use std::path::Path;

/// Audio player wrapper
pub struct AudioPlayer {
    manager: AudioManager<DefaultBackend>,
    sound_handle: Option<StaticSoundHandle>,
    /// Playback speed multiplier
    speed: f64,
    /// Whether we have audio loaded
    has_audio: bool,
}

impl AudioPlayer {
    /// Create a new audio player
    pub fn new() -> Result<Self> {
        let manager = AudioManager::<DefaultBackend>::new(AudioManagerSettings::default())
            .context("Failed to create audio manager")?;

        Ok(Self {
            manager,
            sound_handle: None,
            speed: 1.0,
            has_audio: false,
        })
    }

    /// Load audio from a file
    pub fn load<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let sound_data = StaticSoundData::from_file(path.as_ref())
            .context("Failed to load audio file")?;

        let handle = self.manager.play(sound_data).context("Failed to play audio")?;
        
        // Immediately pause
        self.sound_handle = Some(handle);
        self.pause();
        self.has_audio = true;

        Ok(())
    }

    /// Check if audio is loaded
    pub fn has_audio(&self) -> bool {
        self.has_audio
    }

    /// Play audio
    pub fn play(&mut self) {
        if let Some(handle) = &mut self.sound_handle {
            let _ = handle.resume(Tween::default());
        }
    }

    /// Pause audio
    pub fn pause(&mut self) {
        if let Some(handle) = &mut self.sound_handle {
            let _ = handle.pause(Tween::default());
        }
    }

    /// Check if audio is playing
    pub fn is_playing(&self) -> bool {
        self.sound_handle
            .as_ref()
            .map(|h| h.state() == kira::sound::PlaybackState::Playing)
            .unwrap_or(false)
    }

    /// Get current position in milliseconds
    pub fn position_ms(&self) -> f64 {
        self.sound_handle
            .as_ref()
            .map(|h| h.position() * 1000.0)
            .unwrap_or(0.0)
    }

    /// Seek to position in milliseconds
    pub fn seek_ms(&mut self, position_ms: f64) {
        if let Some(handle) = &mut self.sound_handle {
            let _ = handle.seek_to(position_ms / 1000.0);
        }
    }

    /// Set playback speed (1.0 = normal)
    pub fn set_speed(&mut self, speed: f64) {
        self.speed = speed.clamp(0.25, 4.0);
        if let Some(handle) = &mut self.sound_handle {
            let _ = handle.set_playback_rate(self.speed, Tween::default());
        }
    }

    /// Get current playback speed
    pub fn speed(&self) -> f64 {
        self.speed
    }

    /// Set volume (0.0 to 1.0)
    pub fn set_volume(&mut self, volume: f64) {
        if let Some(handle) = &mut self.sound_handle {
            // Convert amplitude (0-1) to decibels: dB = 20 * log10(amplitude)
            let db = if volume <= 0.001 {
                Decibels::SILENCE
            } else {
                // Convert amplitude to decibels
                Decibels(20.0 * (volume as f32).log10())
            };
            let _ = handle.set_volume(db, Tween::default());
        }
    }
}

impl Default for AudioPlayer {
    fn default() -> Self {
        Self::new().expect("Failed to create audio player")
    }
}
