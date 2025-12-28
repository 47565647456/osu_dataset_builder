//! Audio playback using bevy_kira_audio

use bevy::prelude::*;
use bevy_kira_audio::prelude::*;
use std::path::PathBuf;

use crate::playback::{PlaybackState, PlaybackStateRes};
use crate::AudioFilePath;

pub struct AudioPlayerPlugin;

impl Plugin for AudioPlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AudioState>()
            .add_systems(Startup, setup_audio)
            .add_systems(Update, start_audio_when_ready)
            .add_systems(Update, sync_audio_with_playback)
            .add_systems(Update, handle_audio_seek)
            .add_systems(Update, sync_time_from_audio);
    }
}

/// Resource tracking audio state
#[derive(Resource, Default)]
pub struct AudioState {
    pub handle: Option<Handle<bevy_kira_audio::AudioSource>>,
    pub instance: Option<Handle<AudioInstance>>,
    pub speed: f64,
    pub started: bool,
    pub audio_path: Option<PathBuf>,
    pub last_seek_time: f64,
}

/// System to load audio on startup
fn setup_audio(
    audio_path: Res<AudioFilePath>,
    mut audio_state: ResMut<AudioState>,
) {
    if let Some(path) = &audio_path.0 {
        log::info!("Audio path set: {}", path.display());
        audio_state.audio_path = Some(path.clone());
        audio_state.speed = 1.0;
    }
}

/// System to start audio when play is pressed
fn start_audio_when_ready(
    audio: Res<Audio>,
    asset_server: Res<AssetServer>,
    mut audio_state: ResMut<AudioState>,
    mut audio_instances: ResMut<Assets<AudioInstance>>,
    playback_state: Res<PlaybackStateRes>,
) {
    // Only start audio once when playing and not already started
    if playback_state.state == PlaybackState::Playing && !audio_state.started {
        // Stop any existing audio instance first
        if let Some(instance_handle) = &audio_state.instance {
            if let Some(instance) = audio_instances.get_mut(instance_handle) {
                instance.stop(AudioTween::default());
            }
        }
        audio_state.instance = None;

        if let Some(path) = &audio_state.audio_path {
            log::info!("Starting audio from: {}", path.display());
            
            // Load audio file
            let audio_handle: Handle<bevy_kira_audio::AudioSource> = 
                asset_server.load(path.to_string_lossy().to_string());
            
            // Start playing, seek to current time
            let instance = audio.play(audio_handle.clone())
                .with_playback_rate(playback_state.speed)
                .start_from(playback_state.current_time / 1000.0) // Convert ms to seconds
                .handle();
            
            audio_state.handle = Some(audio_handle);
            audio_state.instance = Some(instance);
            audio_state.started = true;
            audio_state.last_seek_time = playback_state.current_time;
        }
    }
}

/// System to sync audio play/pause/speed with playback state
fn sync_audio_with_playback(
    mut audio_state: ResMut<AudioState>,
    mut audio_instances: ResMut<Assets<AudioInstance>>,
    playback_state: Res<PlaybackStateRes>,
) {
    if !audio_state.started {
        return;
    }
    
    if let Some(instance_handle) = &audio_state.instance {
        if let Some(instance) = audio_instances.get_mut(instance_handle) {
            match playback_state.state {
                PlaybackState::Playing => {
                    instance.resume(AudioTween::default());
                }
                PlaybackState::Paused | PlaybackState::Stopped => {
                    instance.pause(AudioTween::default());
                }
            }
            
            // Update speed if changed
            if (playback_state.speed - audio_state.speed).abs() > 0.01 {
                // bevy_kira_audio/kira supports negative playback rates for reverse
                instance.set_playback_rate(playback_state.speed, AudioTween::default());
                audio_state.speed = playback_state.speed;
            }
        }
    }
}

/// System to handle audio seeking when playback time changes significantly
fn handle_audio_seek(
    mut audio_state: ResMut<AudioState>,
    mut audio_instances: ResMut<Assets<AudioInstance>>,
    playback_state: Res<PlaybackStateRes>,
) {
    if !audio_state.started {
        return;
    }

    // Check if we need to seek (time jumped by more than 500ms)
    let time_diff = (playback_state.current_time - audio_state.last_seek_time).abs();
    if time_diff > 500.0 {
        if let Some(instance_handle) = &audio_state.instance {
            if let Some(instance) = audio_instances.get_mut(instance_handle) {
                let seek_seconds = playback_state.current_time / 1000.0;
                instance.seek_to(seek_seconds);
                audio_state.last_seek_time = playback_state.current_time;
                log::info!("Audio seeking to {}s", seek_seconds);
            }
        }
    }
}

/// System to sync playback time from audio position (when playing)
fn sync_time_from_audio(
    mut audio_state: ResMut<AudioState>,
    mut audio_instances: ResMut<Assets<AudioInstance>>,
    mut playback_state: ResMut<PlaybackStateRes>,
) {
    // Only sync when audio is playing and started
    if !audio_state.started || playback_state.state != PlaybackState::Playing {
        return;
    }

    // Clone the handle to avoid borrow conflict
    let instance_handle = match audio_state.instance.clone() {
        Some(h) => h,
        None => return,
    };

    if let Some(instance) = audio_instances.get(&instance_handle) {
        // Check if audio has stopped/finished
        let state = instance.state();
        
        // Get audio position and sync to playback if available
        if let Some(position_secs) = state.position() {
            let audio_time_ms = position_secs * 1000.0;
            
            // Update last seek time to stay in sync
            audio_state.last_seek_time = audio_time_ms;
            
            // Sync playback time from audio
            let diff = (playback_state.current_time - audio_time_ms).abs();
            if diff > 10.0 && diff < 500.0 {
                playback_state.current_time = audio_time_ms;
            }
            
            // Detect if audio has reached either end
            if playback_state.speed > 0.0 && audio_time_ms >= playback_state.total_duration - 100.0 {
                // Audio finished at end - stop it and reset for replay
                if let Some(mut instance) = audio_instances.remove(&instance_handle) {
                    instance.stop(AudioTween::default());
                }
                audio_state.started = false;
                audio_state.instance = None;
                playback_state.state = PlaybackState::Paused;
                log::info!("Audio reached end, pausing");
            } else if playback_state.speed < 0.0 && audio_time_ms <= 50.0 {
                // Audio reached start while reversing - stop it
                if let Some(mut instance) = audio_instances.remove(&instance_handle) {
                    instance.stop(AudioTween::default());
                }
                audio_state.started = false;
                audio_state.instance = None;
                playback_state.state = PlaybackState::Paused;
                playback_state.current_time = 0.0;
                log::info!("Audio reached start, pausing");
            }
        } else {
            // No position available - audio may have stopped
            // Stop and reset so it can be restarted on next play
            if let Some(mut instance) = audio_instances.remove(&instance_handle) {
                instance.stop(AudioTween::default());
            }
            audio_state.started = false;
            audio_state.instance = None;
            log::info!("Audio position unavailable, resetting");
        }
    }
}
