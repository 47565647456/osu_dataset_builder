//! CLI tool for reconstructing beatmap folders from parquet dataset

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;

use osu_reconstructor::{ParquetReader, FolderReconstructor};

#[derive(Parser, Debug)]
#[command(name = "reconstruct")]
#[command(about = "Reconstruct osu! beatmap folders from parquet dataset")]
struct Args {
    /// Path to the dataset directory containing parquet files
    #[arg(short, long)]
    dataset: PathBuf,

    /// Path to the assets directory
    #[arg(short, long)]
    assets: PathBuf,

    /// Output directory for reconstructed folders
    #[arg(short, long)]
    output: PathBuf,

    /// Specific folder ID to reconstruct (optional, reconstructs all if not specified)
    #[arg(short, long)]
    folder_id: Option<String>,

    /// Limit number of folders to reconstruct (for testing)
    #[arg(long)]
    limit: Option<usize>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    println!("=== osu! Beatmap Reconstructor ===");
    println!("Dataset: {}", args.dataset.display());
    println!("Assets: {}", args.assets.display());
    println!("Output: {}", args.output.display());

    // Load dataset
    println!("\nLoading parquet dataset...");
    let reader = ParquetReader::new(&args.dataset);
    let dataset = reader.load_all().context("Failed to load dataset")?;

    println!("  Beatmaps: {}", dataset.beatmaps.len());
    println!("  Hit objects: {}", dataset.hit_objects.len());
    println!("  Timing points: {}", dataset.timing_points.len());
    println!("  Storyboard elements: {}", dataset.storyboard_elements.len());
    println!("  Storyboard commands: {}", dataset.storyboard_commands.len());
    println!("  Slider control points: {}", dataset.slider_control_points.len());
    println!("  Slider data: {}", dataset.slider_data.len());

    // Create reconstructor
    let reconstructor = FolderReconstructor::new(&args.assets);

    // Determine folder IDs to process
    let folder_ids: Vec<String> = if let Some(ref id) = args.folder_id {
        vec![id.clone()]
    } else {
        let mut ids = FolderReconstructor::get_folder_ids(&dataset);
        if let Some(limit) = args.limit {
            ids.truncate(limit);
        }
        ids
    };

    println!("\nReconstructing {} folder(s)...", folder_ids.len());

    let mut success = 0;
    let mut failed = 0;

    for folder_id in &folder_ids {
        match reconstructor.reconstruct_folder(folder_id, &args.output, &dataset) {
            Ok(result) => {
                println!(
                    "  ✓ {}: {} .osu files, {} storyboard elements, {} assets",
                    folder_id,
                    result.osu_files.len(),
                    result.storyboard_elements,
                    result.assets_copied
                );
                success += 1;
            }
            Err(e) => {
                println!("  ✗ {}: {}", folder_id, e);
                failed += 1;
            }
        }
    }

    println!("\n=== Summary ===");
    println!("Reconstructed: {}", success);
    println!("Failed: {}", failed);

    Ok(())
}
