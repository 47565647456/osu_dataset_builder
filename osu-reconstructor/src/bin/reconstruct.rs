//! CLI tool for reconstructing beatmap folders from parquet dataset

use anyhow::{Context, Result};
use clap::Parser;
use rayon::prelude::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

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

    /// Number of parallel threads (default: 1 for low memory, increase for speed)
    #[arg(short = 't', long, default_value = "1")]
    threads: usize,
}

fn main() -> Result<()> {
    let args = Args::parse();

    println!("=== osu! Beatmap Reconstructor ===");
    println!("Dataset: {}", args.dataset.display());
    println!("Assets: {}", args.assets.display());
    println!("Output: {}", args.output.display());
    println!("Threads: {}", args.threads);

    // Configure thread pool
    rayon::ThreadPoolBuilder::new()
        .num_threads(args.threads)
        .build_global()
        .ok();

    let reader = ParquetReader::new(&args.dataset);
    let reconstructor = FolderReconstructor::new(&args.assets);

    // Determine folder IDs to process
    let folder_ids: Vec<String> = if let Some(ref id) = args.folder_id {
        vec![id.clone()]
    } else {
        println!("\nLoading folder IDs...");
        let mut ids = reader.load_folder_ids().context("Failed to load folder IDs")?;
        println!("Found {} folders", ids.len());
        if let Some(limit) = args.limit {
            ids.truncate(limit);
        }
        ids
    };

    let total = folder_ids.len();
    println!("\nReconstructing {} folder(s)...", total);

    let success = AtomicUsize::new(0);
    let failed = AtomicUsize::new(0);

    folder_ids.par_iter().for_each(|folder_id| {
        // Each thread creates its own reader for parallel file access
        let thread_reader = ParquetReader::new(&args.dataset);
        
        let dataset = match thread_reader.load_dataset_for_folder(folder_id) {
            Ok(d) => d,
            Err(e) => {
                failed.fetch_add(1, Ordering::Relaxed);
                eprintln!("  ✗ {}: Failed to load data: {}", folder_id, e);
                return;
            }
        };

        match reconstructor.reconstruct_folder(folder_id, &args.output, &dataset) {
            Ok(result) => {
                let s = success.fetch_add(1, Ordering::Relaxed) + 1;
                println!(
                    "  [{}/{}] ✓ {}: {} .osu files, {} storyboard elements, {} assets",
                    s, total, folder_id,
                    result.osu_files.len(),
                    result.storyboard_elements,
                    result.assets_copied
                );
            }
            Err(e) => {
                failed.fetch_add(1, Ordering::Relaxed);
                eprintln!("  ✗ {}: {}", folder_id, e);
            }
        }
        // dataset is dropped here, freeing memory
    });

    println!("\n=== Summary ===");
    println!("Reconstructed: {}", success.load(Ordering::Relaxed));
    println!("Failed: {}", failed.load(Ordering::Relaxed));

    Ok(())
}
