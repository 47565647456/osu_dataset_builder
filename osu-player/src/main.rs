//! osu-player: Bevy-powered .osu beatmap player
//!
//! Usage: osu-player <path-to-osu-file>

mod audio;
mod beatmap;
mod input;
mod playback;
mod rendering;
mod ui;

use anyhow::{Context, Result};
use bevy::asset::UnapprovedPathMode;
use bevy::prelude::*;
use bevy::window::WindowResolution;
use bevy::window::PresentMode;
use bevy_kira_audio::AudioPlugin;
use clap::Parser;
use std::path::PathBuf;

use audio::AudioPlayerPlugin;
use beatmap::BeatmapView;
use input::InputPlugin;
use playback::PlaybackPlugin;
use rendering::RenderingPlugin;
use ui::UiPlugin;

#[derive(Parser, Debug)]
#[command(name = "osu-player")]
#[command(about = "Bevy-powered .osu beatmap player with 2D rendering")]
struct Args {
    /// Path to the .osu file to play
    #[arg(required = true)]
    osu_file: PathBuf,
}

/// Resource holding the path to the audio file
#[derive(Resource)]
pub struct AudioFilePath(pub Option<PathBuf>);

/// Resource holding the beatmap title for the window
#[derive(Resource)]
pub struct BeatmapTitle(pub String);

fn main() -> Result<()> {
    env_logger::init();

    let args = Args::parse();

    // Validate the file exists
    if !args.osu_file.exists() {
        anyhow::bail!("File not found: {}", args.osu_file.display());
    }

    if args.osu_file.extension().map_or(true, |e| e != "osu") {
        anyhow::bail!("File must have .osu extension");
    }

    // Parse the beatmap
    log::info!("Loading beatmap: {}", args.osu_file.display());
    let beatmap: rosu_map::Beatmap =
        rosu_map::from_path(&args.osu_file).context("Failed to parse .osu file")?;

    log::info!(
        "Loaded: {} - {} [{}]",
        beatmap.artist,
        beatmap.title,
        beatmap.version
    );
    log::info!("Hit objects: {}", beatmap.hit_objects.len());

    // Get audio file path
    let audio_path = args
        .osu_file
        .parent()
        .map(|p| p.join(&beatmap.audio_file))
        .filter(|p| p.exists());

    if audio_path.is_none() {
        log::warn!(
            "Audio file not found: {}. Playback will be silent.",
            beatmap.audio_file
        );
    }

    let title = format!(
        "{} - {} [{}] - osu-player",
        beatmap.artist, beatmap.title, beatmap.version
    );

    // Create beatmap view
    let beatmap_view = BeatmapView::new(beatmap);

    // Run Bevy app
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: title.clone(),
                        resolution: WindowResolution::new(1280, 720),
                        // present_mode: PresentMode::AutoVsync,
                        present_mode: PresentMode::Immediate,
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    // Allow loading assets from external paths (outside assets folder)
                    unapproved_path_mode: UnapprovedPathMode::Allow,
                    ..default()
                }),
        )
        .add_plugins(AudioPlugin)
        .add_plugins(AudioPlayerPlugin)
        .add_plugins(PlaybackPlugin)
        .add_plugins(RenderingPlugin)
        .add_plugins(UiPlugin)
        .add_plugins(InputPlugin)
        .add_systems(Startup, configure_gizmos)
        .insert_resource(beatmap_view)
        .insert_resource(AudioFilePath(audio_path))
        .insert_resource(BeatmapTitle(title))
        .run();

    Ok(())
}

/// Configure gizmos to render on top of all materials
fn configure_gizmos(mut config_store: ResMut<GizmoConfigStore>) {
    let (config, _) = config_store.config_mut::<DefaultGizmoConfigGroup>();
    // Set depth_bias to push gizmos closer to camera
    config.depth_bias = -1.0;
}
