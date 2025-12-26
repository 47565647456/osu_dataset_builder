//! osu-enricher: Enrich beatmap data with API metadata and PP calculations
//!
//! Reads beatmap_ids from existing dataset, fetches API data, calculates PP,
//! and writes enriched data to new parquet files.

mod batch_writer;
mod clients;

use anyhow::{Context, Result};
use arrow::array::*;
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use rosu_pp::{Beatmap as PpBeatmap, Difficulty, Performance};
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use futures::stream::{self, StreamExt};

/// Enrich beatmap data with osu! API metadata and PP calculations
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// Path to dataset directory containing beatmaps.parquet
    #[arg(long, default_value = r"E:\osu_model\dataset")]
    dataset_dir: PathBuf,

    /// Path to source directory containing extracted .osu files
    #[arg(long, default_value = r"E:\osu_model\osu_archives_extracted")]
    source_dir: PathBuf,

    /// Path to credentials file (first line: client_id, second line: client_secret)
    #[arg(long, default_value = r"E:\osu_model\osu_credentials.txt")]
    credentials: PathBuf,

    /// Force re-enrichment even if beatmap already exists in output
    #[arg(long, short)]
    force: bool,
}

fn read_credentials(path: &Path) -> Result<Vec<(u64, String)>> {
    let file = File::open(path)
        .with_context(|| format!("Failed to open credentials file: {}", path.display()))?;
    let reader = BufReader::new(file);
    
    let mut credentials = Vec::new();
    let lines: Vec<String> = reader.lines()
        .filter_map(|l| l.ok())
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    
    let mut iter = lines.into_iter();
    while let Some(client_id_str) = iter.next() {
        let client_id = client_id_str.parse::<u64>()
            .with_context(|| format!("client_id '{}' must be a number", client_id_str))?;
            
        let client_secret = iter.next()
            .context("Credentials file missing client_secret for a client_id")?;
            
        credentials.push((client_id, client_secret));
    }
    
    if credentials.is_empty() {
        anyhow::bail!("Credentials file is empty or contains no valid pairs");
    }
    
    Ok(credentials)
}

// ============ Data Structures ============

/// Comprehensive beatmap row combining API metadata and PP calculations
#[derive(Default)]
pub(crate) struct BeatmapRow {
    // Identifiers
    beatmap_id: u32,
    beatmapset_id: u32,
    folder_id: String,
    osu_file: String,
    
    // API Metadata (BeatmapExtended)
    mode: String,           // osu, taiko, catch, mania
    version: String,        // Difficulty name
    url: String,
    status: String,         // Ranked, Loved, etc.
    is_scoreable: bool,
    convert: bool,
    
    // Difficulty settings (API)
    ar: f32,
    cs: f32,
    od: f32,
    hp: f32,
    bpm: f32,
    
    // Counts
    count_circles: u32,
    count_sliders: u32,
    count_spinners: u32,
    
    // Length
    seconds_drain: u32,
    seconds_total: u32,
    
    // Stats
    playcount: u32,
    passcount: u32,
    max_combo_api: Option<u32>,
    stars_api: f32,
    
    // Other
    checksum: String,
    creator_id: u32,
    last_updated: Option<i64>,  // Unix timestamp
    
    // PP Calculation Results (rosu-pp)
    stars_calc: f64,
    max_pp: f64,
    max_combo_calc: u32,
    
    // osu! specific (null for other modes)
    osu_aim: Option<f64>,
    osu_speed: Option<f64>,
    osu_flashlight: Option<f64>,
    osu_slider_factor: Option<f64>,
    osu_speed_note_count: Option<f64>,
    osu_aim_difficult_slider_count: Option<f64>,
    osu_aim_difficult_strain_count: Option<f64>,
    osu_speed_difficult_strain_count: Option<f64>,
    osu_great_hit_window: Option<f64>,
    osu_ok_hit_window: Option<f64>,
    osu_meh_hit_window: Option<f64>,
    osu_n_large_ticks: Option<u32>,
    
    // taiko specific
    taiko_stamina: Option<f64>,
    taiko_rhythm: Option<f64>,
    taiko_color: Option<f64>,
    taiko_reading: Option<f64>,
    taiko_great_hit_window: Option<f64>,
    taiko_ok_hit_window: Option<f64>,
    taiko_mono_stamina_factor: Option<f64>,
    
    // catch specific
    catch_ar: Option<f64>,
    catch_n_fruits: Option<u32>,
    catch_n_droplets: Option<u32>,
    catch_n_tiny_droplets: Option<u32>,
    
    // mania specific
    mania_n_objects: Option<u32>,
    mania_n_hold_notes: Option<u32>,
    
    // Common for converts
    is_convert: Option<bool>,
    
    // PP calculation status
    pp_failed: Option<String>,  // Reason if PP calculation failed (e.g., "Suspicious map: Density")
}

pub(crate) struct CommentRow {
    beatmapset_id: u32,
    comment_id: u32,
    parent_id: Option<u32>,
    user_id: Option<u32>,
    legacy_name: Option<String>,
    message: Option<String>,
    message_html: Option<String>,
    votes_count: u32,
    replies_count: u32,
    pinned: bool,
    commentable_type: String,
    created_at: i64,       // Unix timestamp
    updated_at: i64,       // Unix timestamp
    edited_at: Option<i64>,
    edited_by_id: Option<u32>,
    deleted_at: Option<i64>,
}

// ============ Main ============

#[tokio::main]
async fn main() -> Result<()> {
    let args = Arc::new(Args::parse());

    // Load API credentials from file
    println!("Reading credentials from {}...", args.credentials.display());
    let credentials = read_credentials(&args.credentials)?;

    println!("Initializing {} osu! API clients...", credentials.len());
    let pool = clients::OsuClientPool::new(credentials).await?;

    // Read existing beatmap IDs from dataset
    println!("Reading existing beatmap IDs from dataset...");
    let all_beatmap_ids = read_beatmap_ids(&args.dataset_dir)?;
    println!("Found {} beatmaps with valid IDs", all_beatmap_ids.len());

    // Read already-enriched beatmap IDs (unless --force)
    let existing_enriched: HashSet<u32> = if !args.force {
        read_existing_enriched_ids(&args.dataset_dir)
    } else {
        HashSet::new()
    };

    // Load failed beatmaps list (format: "id: reason")
    let failed_path = args.dataset_dir.join("failed_beatmaps.txt");
    let failed_lines: Vec<String> = if failed_path.exists() {
        std::fs::read_to_string(&failed_path)
            .unwrap_or_default()
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        Vec::new()
    };
    let failed_id_set: HashSet<u32> = failed_lines
        .iter()
        .filter_map(|line| line.split(':').next()?.trim().parse().ok())
        .collect();
    let failed_ids: HashSet<String> = failed_lines.into_iter().collect();
    let initial_failed_count = failed_id_set.len();

    // Filter out already-enriched and failed beatmaps
    let beatmap_ids: Vec<_> = all_beatmap_ids
        .into_iter()
        .filter(|(id, _, _)| !existing_enriched.contains(id) && !failed_id_set.contains(id))
        .collect();

    if !existing_enriched.is_empty() {
        println!("Skipping {} already enriched beatmaps (use --force to re-fetch)", existing_enriched.len());
    }
    if initial_failed_count > 0 {
        println!("Skipping {} permanently failed beatmaps", initial_failed_count);
    }

    if beatmap_ids.is_empty() {
        println!("No new beatmap IDs to enrich. Exiting.");
        return Ok(());
    }

    println!("Enriching {} new beatmaps", beatmap_ids.len());

    // Prepare output paths
    let enriched_path = args.dataset_dir.join("beatmap_enriched.parquet");
    let comments_path = args.dataset_dir.join("beatmap_comments.parquet");

    // Initialize batch writers for streaming output
    let enriched_writer = Arc::new(Mutex::new(batch_writer::EnrichedBatchWriter::new(&enriched_path)?));
    let comments_writer = Arc::new(Mutex::new(batch_writer::CommentsBatchWriter::new(&comments_path)?));

    // Shared thread-safe collections
    let beatmapset_ids = Arc::new(Mutex::new(HashSet::new()));
    let failed_ids = Arc::new(Mutex::new(failed_ids));

    // Set up graceful shutdown
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let shutdown_clone = shutdown_requested.clone();
    ctrlc::set_handler(move || {
        println!("\n⏳ Ctrl+C received! Finishing current request then stopping...");
        shutdown_clone.store(true, Ordering::SeqCst);
    }).expect("Error setting Ctrl+C handler");

    let mut interrupted = false;

    let pb = ProgressBar::new(beatmap_ids.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")
            .unwrap()
            .progress_chars("#>-"),
    );

    // Fetch metadata for each beatmap in parallel
    let pool = Arc::new(pool);
    let parallelism = pool.client_count() * 2;
    
    let mut stream = stream::iter(beatmap_ids.iter())
        .map(|(beatmap_id, folder_id, osu_file)| {
            let pool = Arc::clone(&pool);
            let source_dir = args.source_dir.clone();
            let beatmapset_ids = Arc::clone(&beatmapset_ids);
            let failed_ids = Arc::clone(&failed_ids);
            let enriched_writer = Arc::clone(&enriched_writer);
            let shutdown_requested = Arc::clone(&shutdown_requested);
            let pb = pb.clone();
            
            async move {
                if shutdown_requested.load(Ordering::SeqCst) {
                    return Ok(());
                }

                pb.set_message(format!("Fetching {}", beatmap_id));
                
                let osu_client = pool.get_next();
                osu_client.rate_limiter.until_ready().await;

                let mut row = BeatmapRow {
                    beatmap_id: *beatmap_id,
                    folder_id: folder_id.clone(),
                    osu_file: osu_file.clone(),
                    ..Default::default()
                };

                match osu_client.client.beatmap().map_id(*beatmap_id).await {
                    Ok(beatmap) => {
                        beatmapset_ids.lock().unwrap().insert(beatmap.mapset_id);
                        
                        row.beatmapset_id = beatmap.mapset_id;
                        row.mode = format!("{:?}", beatmap.mode).to_lowercase();
                        row.version = beatmap.version.clone();
                        row.url = beatmap.url.clone();
                        row.status = format!("{:?}", beatmap.status);
                        row.is_scoreable = beatmap.is_scoreable;
                        row.convert = beatmap.convert;
                        row.ar = beatmap.ar;
                        row.cs = beatmap.cs;
                        row.od = beatmap.od;
                        row.hp = beatmap.hp;
                        row.bpm = beatmap.bpm;
                        row.count_circles = beatmap.count_circles;
                        row.count_sliders = beatmap.count_sliders;
                        row.count_spinners = beatmap.count_spinners;
                        row.seconds_drain = beatmap.seconds_drain;
                        row.seconds_total = beatmap.seconds_total;
                        row.playcount = beatmap.playcount;
                        row.passcount = beatmap.passcount;
                        row.max_combo_api = beatmap.max_combo;
                        row.stars_api = beatmap.stars;
                        row.checksum = beatmap.checksum.unwrap_or_default();
                        row.creator_id = beatmap.creator_id;
                        row.last_updated = Some(beatmap.last_updated.unix_timestamp());
                    }
                    Err(e) => {
                        let error_str = format!("{}", e);
                        if error_str.contains("404") || error_str.contains("missing") {
                            failed_ids.lock().unwrap().insert(format!("{}: {}", beatmap_id, e));
                        }
                        pb.println(format!("⚠ Failed to fetch API data for {}: {}", beatmap_id, e));
                    }
                }

                let osu_path = source_dir.join(&folder_id).join(&osu_file);
                if osu_path.exists() {
                    match calculate_difficulty(&osu_path, &mut row) {
                        Ok(_) => {}
                        Err(e) => {
                            row.pp_failed = Some(format!("{}", e));
                            pb.println(format!("⚠ Failed to calculate PP for {}: {}", osu_file, e));
                        }
                    }
                }

                enriched_writer.lock().unwrap_or_else(|e| e.into_inner()).write(row)?;
                pb.inc(1);
                Ok::<(), anyhow::Error>(())
            }
        })
        .buffer_unordered(parallelism);

    while let Some(res) = stream.next().await {
        if let Err(e) = res {
            pb.println(format!("🛑 Critical error in fetch stream: {}", e));
            interrupted = true;
            break;
        }
        if shutdown_requested.load(Ordering::SeqCst) {
            interrupted = true;
            break;
        }
    }
    drop(stream); // Release Arc references

    pb.finish_with_message("Beatmap fetching complete");

    // Get ALL beatmapset_ids from enriched data (including previous runs)
    // Combined with beatmapset_ids from this run
    let all_enriched_beatmapset_ids = read_all_enriched_beatmapset_ids(&args.dataset_dir);
    let current_beatmapset_ids = beatmapset_ids.lock().unwrap_or_else(|e| e.into_inner());
    let all_beatmapset_ids: HashSet<u32> = all_enriched_beatmapset_ids.union(&current_beatmapset_ids).copied().collect();
    drop(current_beatmapset_ids);
    
    // Read already-commented beatmapset_ids (unless --force)
    let existing_commented: HashSet<u32> = if !args.force {
        read_existing_commented_beatmapset_ids(&args.dataset_dir)
    } else {
        HashSet::new()
    };
    
    // Filter to only new beatmapset_ids
    let new_beatmapset_ids: Vec<u32> = all_beatmapset_ids
        .into_iter()
        .filter(|id| !existing_commented.contains(id))
        .collect();
    
    if !existing_commented.is_empty() {
        println!("Skipping {} already-commented beatmapsets", existing_commented.len());
    }

    // Fetch comments for each beatmapset
    println!("Fetching comments for {} new beatmapsets...", new_beatmapset_ids.len());
    let pb2 = ProgressBar::new(new_beatmapset_ids.len() as u64);
    pb2.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("#>-"),
    );

    // Fetch comments for each beatmapset in parallel
    let mut comment_stream = stream::iter(new_beatmapset_ids.iter())
        .map(|beatmapset_id| {
            let pool = Arc::clone(&pool);
            let comments_writer = Arc::clone(&comments_writer);
            let shutdown_requested = Arc::clone(&shutdown_requested);
            let pb2 = pb2.clone();
            let beatmapset_id = *beatmapset_id;
            
            async move {
                if shutdown_requested.load(Ordering::SeqCst) {
                    return Ok(());
                }

                let osu_client = pool.get_next();
                osu_client.rate_limiter.until_ready().await;

                match osu_client.client
                    .comments()
                    .commentable_type("beatmapset")
                    .commentable_id(beatmapset_id)
                    .await
                {
                    Ok(bundle) => {
                        let to_row = |comment: &rosu_v2::model::comments::Comment, mapset_id: u32| CommentRow {
                            beatmapset_id: mapset_id,
                            comment_id: comment.comment_id,
                            parent_id: comment.parent_id,
                            user_id: comment.user_id,
                            legacy_name: comment.legacy_name.as_ref().map(|s| s.to_string()),
                            message: comment.message.clone(),
                            message_html: comment.message_html.clone(),
                            votes_count: comment.votes_count,
                            replies_count: comment.replies_count,
                            pinned: comment.pinned,
                            commentable_type: comment.commentable_type.clone(),
                            created_at: comment.created_at.unix_timestamp(),
                            updated_at: comment.updated_at.unix_timestamp(),
                            edited_at: comment.edited_at.map(|t| t.unix_timestamp()),
                            edited_by_id: comment.edited_by_id,
                            deleted_at: comment.deleted_at.map(|t| t.unix_timestamp()),
                        };

                        let mut writer = comments_writer.lock().unwrap_or_else(|e| e.into_inner());
                        for comment in &bundle.comments {
                            writer.write(to_row(comment, beatmapset_id))?;
                        }
                        for comment in &bundle.included_comments {
                            writer.write(to_row(comment, beatmapset_id))?;
                        }
                        if let Some(pinned) = &bundle.pinned_comments {
                            for comment in pinned {
                                if !bundle.comments.iter().any(|c| c.comment_id == comment.comment_id) {
                                    writer.write(to_row(comment, beatmapset_id))?;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        pb2.println(format!("⚠ Failed to fetch comments for mapset {}: {}", beatmapset_id, e));
                    }
                }

                pb2.inc(1);
                Ok::<(), anyhow::Error>(())
            }
        })
        .buffer_unordered(parallelism);

    while let Some(res) = comment_stream.next().await {
        if let Err(e) = res {
            pb2.println(format!("🛑 Critical error in comment stream: {}", e));
            break;
        }
        if shutdown_requested.load(Ordering::SeqCst) {
            break;
        }
    }
    drop(comment_stream); // Release Arc references

    pb2.finish_with_message("Comment fetching complete");

    // Close batch writers and get totals (handles merge automatically)
    println!("\n=== Writing Parquet Files ===");
    
    let enriched_total = match Arc::try_unwrap(enriched_writer) {
        Ok(mutex) => mutex.into_inner().unwrap_or_else(|e| e.into_inner()).close()?,
        Err(_) => anyhow::bail!("Failed to unwrap enriched_writer: active references remain"),
    };
    println!("  beatmap_enriched.parquet: {} rows", enriched_total);
    
    let comments_total = match Arc::try_unwrap(comments_writer) {
        Ok(mutex) => mutex.into_inner().unwrap_or_else(|e| e.into_inner()).close()?,
        Err(_) => anyhow::bail!("Failed to unwrap comments_writer: active references remain"),
    };
    println!("  beatmap_comments.parquet: {} rows", comments_total);

    // Save failed list if there are new failures
    let final_failed_ids = failed_ids.lock().unwrap_or_else(|e| e.into_inner());
    let new_failures = final_failed_ids.len() - initial_failed_count;
    if new_failures > 0 {
        let content: String = final_failed_ids.iter().map(|s| format!("{}\n", s)).collect();
        let _ = std::fs::write(&args.dataset_dir.join("failed_beatmaps.txt"), content);
        println!("Added {} beatmaps to failed_beatmaps.txt", new_failures);
    }

    if interrupted {
        println!("\n⚠ Run was interrupted by Ctrl+C");
    } else {
        println!("\nEnrichment complete!");
    }
    Ok(())
}

// ============ PP Calculation ============

fn calculate_difficulty(osu_path: &Path, row: &mut BeatmapRow) -> Result<()> {
    let map = PpBeatmap::from_path(osu_path)?;
    
    // Check for suspicious maps
    if let Err(sus) = map.check_suspicion() {
        anyhow::bail!("Suspicious map: {:?}", sus);
    }

    // Calculate difficulty (nomod)
    let diff_attrs = Difficulty::new().calculate(&map);
    row.stars_calc = diff_attrs.stars();
    row.max_combo_calc = diff_attrs.max_combo();

    // Calculate max PP (SS, nomod)
    let perf_attrs = Performance::new(diff_attrs.clone()).calculate();
    row.max_pp = perf_attrs.pp();

    // Extract mode-specific attributes
    match diff_attrs {
        rosu_pp::any::DifficultyAttributes::Osu(attrs) => {
            row.osu_aim = Some(attrs.aim);
            row.osu_speed = Some(attrs.speed);
            row.osu_flashlight = Some(attrs.flashlight);
            row.osu_slider_factor = Some(attrs.slider_factor);
            row.osu_speed_note_count = Some(attrs.speed_note_count);
            row.osu_aim_difficult_slider_count = Some(attrs.aim_difficult_slider_count);
            row.osu_aim_difficult_strain_count = Some(attrs.aim_difficult_strain_count);
            row.osu_speed_difficult_strain_count = Some(attrs.speed_difficult_strain_count);
            row.osu_great_hit_window = Some(attrs.great_hit_window);
            row.osu_ok_hit_window = Some(attrs.ok_hit_window);
            row.osu_meh_hit_window = Some(attrs.meh_hit_window);
            row.osu_n_large_ticks = Some(attrs.n_large_ticks);
        }
        rosu_pp::any::DifficultyAttributes::Taiko(attrs) => {
            row.taiko_stamina = Some(attrs.stamina);
            row.taiko_rhythm = Some(attrs.rhythm);
            row.taiko_color = Some(attrs.color);
            row.taiko_reading = Some(attrs.reading);
            row.taiko_great_hit_window = Some(attrs.great_hit_window);
            row.taiko_ok_hit_window = Some(attrs.ok_hit_window);
            row.taiko_mono_stamina_factor = Some(attrs.mono_stamina_factor);
            row.is_convert = Some(attrs.is_convert);
        }
        rosu_pp::any::DifficultyAttributes::Catch(attrs) => {
            row.catch_ar = Some(attrs.ar);
            row.catch_n_fruits = Some(attrs.n_fruits);
            row.catch_n_droplets = Some(attrs.n_droplets);
            row.catch_n_tiny_droplets = Some(attrs.n_tiny_droplets);
            row.is_convert = Some(attrs.is_convert);
        }
        rosu_pp::any::DifficultyAttributes::Mania(attrs) => {
            row.mania_n_objects = Some(attrs.n_objects);
            row.mania_n_hold_notes = Some(attrs.n_hold_notes);
            row.is_convert = Some(attrs.is_convert);
        }
    }

    Ok(())
}

// ============ Parquet Reading ============

/// Read existing enriched beatmap_ids from beatmap_enriched.parquet
fn read_existing_enriched_ids(dataset_dir: &Path) -> HashSet<u32> {
    let enriched_path = dataset_dir.join("beatmap_enriched.parquet");
    if !enriched_path.exists() {
        return HashSet::new();
    }

    let mut ids = HashSet::new();
    
    if let Ok(file) = File::open(&enriched_path) {
        if let Ok(reader) = ParquetRecordBatchReaderBuilder::try_new(file) {
            if let Ok(reader) = reader.build() {
                for batch in reader.flatten() {
                    if let Some(col) = batch.column_by_name("beatmap_id") {
                        if let Some(arr) = col.as_any().downcast_ref::<arrow::array::UInt32Array>() {
                            for i in 0..arr.len() {
                                if !arr.is_null(i) {
                                    ids.insert(arr.value(i));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    ids
}

/// Read existing commented beatmapset_ids from beatmap_comments.parquet
fn read_existing_commented_beatmapset_ids(dataset_dir: &Path) -> HashSet<u32> {
    let comments_path = dataset_dir.join("beatmap_comments.parquet");
    if !comments_path.exists() {
        return HashSet::new();
    }

    let mut ids = HashSet::new();
    
    if let Ok(file) = File::open(&comments_path) {
        if let Ok(reader) = ParquetRecordBatchReaderBuilder::try_new(file) {
            if let Ok(reader) = reader.build() {
                for batch in reader.flatten() {
                    if let Some(col) = batch.column_by_name("beatmapset_id") {
                        if let Some(arr) = col.as_any().downcast_ref::<arrow::array::UInt32Array>() {
                            for i in 0..arr.len() {
                                if !arr.is_null(i) {
                                    ids.insert(arr.value(i));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    ids
}

/// Read all beatmapset_ids from beatmap_enriched.parquet
fn read_all_enriched_beatmapset_ids(dataset_dir: &Path) -> HashSet<u32> {
    let enriched_path = dataset_dir.join("beatmap_enriched.parquet");
    if !enriched_path.exists() {
        return HashSet::new();
    }

    let mut ids = HashSet::new();
    
    if let Ok(file) = File::open(&enriched_path) {
        if let Ok(reader) = ParquetRecordBatchReaderBuilder::try_new(file) {
            if let Ok(reader) = reader.build() {
                for batch in reader.flatten() {
                    if let Some(col) = batch.column_by_name("beatmapset_id") {
                        if let Some(arr) = col.as_any().downcast_ref::<arrow::array::UInt32Array>() {
                            for i in 0..arr.len() {
                                if !arr.is_null(i) {
                                    ids.insert(arr.value(i));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    ids
}

fn read_beatmap_ids(dataset_dir: &Path) -> Result<Vec<(u32, String, String)>> {
    let beatmaps_path = dataset_dir.join("beatmaps.parquet");
    let file = File::open(&beatmaps_path)
        .with_context(|| format!("Failed to open {}", beatmaps_path.display()))?;

    let reader = ParquetRecordBatchReaderBuilder::try_new(file)?
        .build()?;

    let mut results = Vec::new();

    for batch in reader {
        let batch = batch?;
        
        // Get columns by name
        let beatmap_id_col = batch
            .column_by_name("beatmap_id")
            .context("Missing beatmap_id column")?
            .as_any()
            .downcast_ref::<Int32Array>()
            .context("beatmap_id is not Int32")?;

        let folder_id_col = batch
            .column_by_name("folder_id")
            .context("Missing folder_id column")?
            .as_any()
            .downcast_ref::<StringArray>()
            .context("folder_id is not String")?;

        let osu_file_col = batch
            .column_by_name("osu_file")
            .context("Missing osu_file column")?
            .as_any()
            .downcast_ref::<StringArray>()
            .context("osu_file is not String")?;

        for i in 0..batch.num_rows() {
            let beatmap_id = beatmap_id_col.value(i);
            if beatmap_id > 0 {
                results.push((
                    beatmap_id as u32,
                    folder_id_col.value(i).to_string(),
                    osu_file_col.value(i).to_string(),
                ));
            }
        }
    }

    Ok(results)
}
