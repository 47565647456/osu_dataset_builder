//! osu-viewer: GPU-accelerated .osu file viewer
//!
//! Usage: osu-viewer <path-to-osu-file>

mod app;
mod audio;
mod beatmap;
mod playback;
mod renderer;
mod timeline;

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "osu-viewer")]
#[command(about = "GPU-accelerated .osu file viewer with timeline scrubbing")]
struct Args {
    /// Path to the .osu file to view
    #[arg(required = true)]
    osu_file: PathBuf,
}

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

    // Run the application
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_title(format!(
                "{} - {} [{}] - osu-viewer",
                beatmap.artist, beatmap.title, beatmap.version
            )),
        vsync: true,
        ..Default::default()
    };

    eframe::run_native(
        "osu-viewer",
        options,
        Box::new(move |cc| {
            Ok(Box::new(app::OsuViewerApp::new(cc, beatmap, audio_path)))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Failed to run application: {}", e))
}
