//! osu-enricher: Enrich beatmap data with API metadata and PP calculations
//!
//! Reads beatmap_ids from existing dataset, fetches API data, calculates PP,
//! and writes enriched data to new parquet files.

use anyhow::{Context, Result};
use arrow::array::*;
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use clap::Parser;
use governor::{Quota, RateLimiter};
use indicatif::{ProgressBar, ProgressStyle};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ArrowWriter;
use parquet::file::properties::WriterProperties;
use rosu_pp::{Beatmap as PpBeatmap, Difficulty, Performance};
use rosu_v2::prelude::*;
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Enrich beatmap data with osu! API metadata and PP calculations
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// Path to dataset directory containing beatmaps.parquet
    #[arg(long, default_value = r"E:\osu_model\dataset")]
    dataset_dir: PathBuf,

    /// Path to source directory containing extracted .osu files
    #[arg(long, default_value = r"E:\osu_model\osz_extracted")]
    source_dir: PathBuf,

    /// Path to credentials file (first line: client_id, second line: client_secret)
    #[arg(long, default_value = r"E:\osu_model\osu_credentials.txt")]
    credentials: PathBuf,
}

fn read_credentials(path: &Path) -> Result<(u64, String)> {
    let file = File::open(path)
        .with_context(|| format!("Failed to open credentials file: {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();
    
    let client_id = lines
        .next()
        .context("Credentials file is empty")?
        .context("Failed to read client_id line")?
        .trim()
        .parse::<u64>()
        .context("client_id must be a number")?;
    
    let client_secret = lines
        .next()
        .context("Credentials file missing client_secret")?
        .context("Failed to read client_secret line")?
        .trim()
        .to_string();
    
    Ok((client_id, client_secret))
}

// ============ Data Structures ============

/// Comprehensive beatmap row combining API metadata and PP calculations
#[derive(Default)]
struct BeatmapRow {
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
}

struct CommentRow {
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
    let args = Args::parse();

    // Load API credentials from file
    println!("Reading credentials from {}...", args.credentials.display());
    let (client_id, client_secret) = read_credentials(&args.credentials)?;

    println!("Initializing osu! API client...");
    let osu = Osu::new(client_id, client_secret).await?;

    // Rate limiter: 60 requests per minute
    let rate_limiter = RateLimiter::direct(Quota::per_minute(NonZeroU32::new(60).unwrap()));

    // Read existing beatmap IDs from dataset
    println!("Reading existing beatmap IDs from dataset...");
    let beatmap_ids = read_beatmap_ids(&args.dataset_dir)?;
    println!("Found {} beatmaps with valid IDs", beatmap_ids.len());

    if beatmap_ids.is_empty() {
        println!("No beatmap IDs found. Exiting.");
        return Ok(());
    }

    // Collect unique beatmapset IDs for comments
    let mut beatmapset_ids: HashSet<u32> = HashSet::new();

    // Prepare output paths
    let enriched_path = args.dataset_dir.join("beatmap_enriched.parquet");
    let comments_path = args.dataset_dir.join("beatmap_comments.parquet");

    let mut beatmap_rows: Vec<BeatmapRow> = Vec::new();
    let mut comment_rows: Vec<CommentRow> = Vec::new();

    let pb = ProgressBar::new(beatmap_ids.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")
            .unwrap()
            .progress_chars("#>-"),
    );

    // Fetch metadata for each beatmap
    for (beatmap_id, folder_id, osu_file) in &beatmap_ids {
        pb.set_message(format!("Fetching {}", beatmap_id));
        
        // Rate limit
        rate_limiter.until_ready().await;

        let mut row = BeatmapRow {
            beatmap_id: *beatmap_id,
            folder_id: folder_id.clone(),
            osu_file: osu_file.clone(),
            ..Default::default()
        };

        // Fetch from API
        match osu.beatmap().map_id(*beatmap_id).await {
            Ok(beatmap) => {
                beatmapset_ids.insert(beatmap.mapset_id);
                
                // API Metadata
                row.beatmapset_id = beatmap.mapset_id;
                row.mode = format!("{:?}", beatmap.mode).to_lowercase();
                row.version = beatmap.version.clone();
                row.url = beatmap.url.clone();
                row.status = format!("{:?}", beatmap.status);
                row.is_scoreable = beatmap.is_scoreable;
                row.convert = beatmap.convert;
                
                // Difficulty settings
                row.ar = beatmap.ar;
                row.cs = beatmap.cs;
                row.od = beatmap.od;
                row.hp = beatmap.hp;
                row.bpm = beatmap.bpm;
                
                // Counts
                row.count_circles = beatmap.count_circles;
                row.count_sliders = beatmap.count_sliders;
                row.count_spinners = beatmap.count_spinners;
                
                // Length
                row.seconds_drain = beatmap.seconds_drain;
                row.seconds_total = beatmap.seconds_total;
                
                // Stats
                row.playcount = beatmap.playcount;
                row.passcount = beatmap.passcount;
                row.max_combo_api = beatmap.max_combo;
                row.stars_api = beatmap.stars;
                
                // Other
                row.checksum = beatmap.checksum.unwrap_or_default();
                row.creator_id = beatmap.creator_id;
                row.last_updated = Some(beatmap.last_updated.unix_timestamp());
            }
            Err(e) => {
                pb.println(format!("⚠ Failed to fetch API data for {}: {}", beatmap_id, e));
            }
        }

        // Calculate PP from local file
        let osu_path = args.source_dir.join(&folder_id).join(&osu_file);
        if osu_path.exists() {
            match calculate_difficulty(&osu_path, &mut row) {
                Ok(_) => {}
                Err(e) => {
                    pb.println(format!("⚠ Failed to calculate PP for {}: {}", osu_file, e));
                }
            }
        }

        beatmap_rows.push(row);
        pb.inc(1);
    }

    pb.finish_with_message("Beatmap fetching complete");

    // Fetch comments for each beatmapset
    println!("\nFetching comments for {} beatmapsets...", beatmapset_ids.len());
    let pb2 = ProgressBar::new(beatmapset_ids.len() as u64);
    pb2.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("#>-"),
    );

    for beatmapset_id in &beatmapset_ids {
        rate_limiter.until_ready().await;

        match osu
            .comments()
            .commentable_type("beatmapset")
            .commentable_id(*beatmapset_id)
            .await
        {
            Ok(bundle) => {
                // Helper to convert Comment to CommentRow
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

                // Add main comments
                for comment in &bundle.comments {
                    comment_rows.push(to_row(comment, *beatmapset_id));
                }

                // Add included comments (replies, parents)
                for comment in &bundle.included_comments {
                    comment_rows.push(to_row(comment, *beatmapset_id));
                }

                // Add pinned comments if present
                if let Some(pinned) = &bundle.pinned_comments {
                    for comment in pinned {
                        // Only add if not already in comments
                        if !bundle.comments.iter().any(|c| c.comment_id == comment.comment_id) {
                            comment_rows.push(to_row(comment, *beatmapset_id));
                        }
                    }
                }
            }
            Err(e) => {
                pb2.println(format!("⚠ Failed to fetch comments for mapset {}: {}", beatmapset_id, e));
            }
        }

        pb2.inc(1);
    }

    pb2.finish_with_message("Comment fetching complete");

    // Write parquet files
    println!("\nWriting parquet files...");
    
    if !beatmap_rows.is_empty() {
        write_enriched_parquet(&enriched_path, &beatmap_rows)?;
        println!("  beatmap_enriched.parquet: {} rows", beatmap_rows.len());
    }

    if !comment_rows.is_empty() {
        write_comments_parquet(&comments_path, &comment_rows)?;
        println!("  beatmap_comments.parquet: {} rows", comment_rows.len());
    }

    println!("\nEnrichment complete!");
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

// ============ Parquet Writers ============

fn enriched_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        // Identifiers
        Field::new("beatmap_id", DataType::UInt32, false),
        Field::new("beatmapset_id", DataType::UInt32, false),
        Field::new("folder_id", DataType::Utf8, false),
        Field::new("osu_file", DataType::Utf8, false),
        
        // API Metadata
        Field::new("mode", DataType::Utf8, false),
        Field::new("version", DataType::Utf8, false),
        Field::new("url", DataType::Utf8, false),
        Field::new("status", DataType::Utf8, false),
        Field::new("is_scoreable", DataType::Boolean, false),
        Field::new("convert", DataType::Boolean, false),
        
        // Difficulty settings
        Field::new("ar", DataType::Float32, false),
        Field::new("cs", DataType::Float32, false),
        Field::new("od", DataType::Float32, false),
        Field::new("hp", DataType::Float32, false),
        Field::new("bpm", DataType::Float32, false),
        
        // Counts
        Field::new("count_circles", DataType::UInt32, false),
        Field::new("count_sliders", DataType::UInt32, false),
        Field::new("count_spinners", DataType::UInt32, false),
        
        // Length
        Field::new("seconds_drain", DataType::UInt32, false),
        Field::new("seconds_total", DataType::UInt32, false),
        
        // Stats
        Field::new("playcount", DataType::UInt32, false),
        Field::new("passcount", DataType::UInt32, false),
        Field::new("max_combo_api", DataType::UInt32, true),
        Field::new("stars_api", DataType::Float32, false),
        
        // Other
        Field::new("checksum", DataType::Utf8, false),
        Field::new("creator_id", DataType::UInt32, false),
        Field::new("last_updated", DataType::Int64, true),
        
        // PP Calculation
        Field::new("stars_calc", DataType::Float64, false),
        Field::new("max_pp", DataType::Float64, false),
        Field::new("max_combo_calc", DataType::UInt32, false),
        
        // osu! specific
        Field::new("osu_aim", DataType::Float64, true),
        Field::new("osu_speed", DataType::Float64, true),
        Field::new("osu_flashlight", DataType::Float64, true),
        Field::new("osu_slider_factor", DataType::Float64, true),
        Field::new("osu_speed_note_count", DataType::Float64, true),
        Field::new("osu_aim_difficult_slider_count", DataType::Float64, true),
        Field::new("osu_aim_difficult_strain_count", DataType::Float64, true),
        Field::new("osu_speed_difficult_strain_count", DataType::Float64, true),
        Field::new("osu_great_hit_window", DataType::Float64, true),
        Field::new("osu_ok_hit_window", DataType::Float64, true),
        Field::new("osu_meh_hit_window", DataType::Float64, true),
        Field::new("osu_n_large_ticks", DataType::UInt32, true),
        
        // taiko specific
        Field::new("taiko_stamina", DataType::Float64, true),
        Field::new("taiko_rhythm", DataType::Float64, true),
        Field::new("taiko_color", DataType::Float64, true),
        Field::new("taiko_reading", DataType::Float64, true),
        Field::new("taiko_great_hit_window", DataType::Float64, true),
        Field::new("taiko_ok_hit_window", DataType::Float64, true),
        Field::new("taiko_mono_stamina_factor", DataType::Float64, true),
        
        // catch specific
        Field::new("catch_ar", DataType::Float64, true),
        Field::new("catch_n_fruits", DataType::UInt32, true),
        Field::new("catch_n_droplets", DataType::UInt32, true),
        Field::new("catch_n_tiny_droplets", DataType::UInt32, true),
        
        // mania specific
        Field::new("mania_n_objects", DataType::UInt32, true),
        Field::new("mania_n_hold_notes", DataType::UInt32, true),
        
        // Common
        Field::new("is_convert", DataType::Boolean, true),
    ]))
}

fn write_enriched_parquet(path: &Path, rows: &[BeatmapRow]) -> Result<()> {
    let schema = enriched_schema();
    let file = File::create(path)?;
    let props = WriterProperties::builder()
        .set_compression(parquet::basic::Compression::SNAPPY)
        .build();
    let mut writer = ArrowWriter::try_new(file, schema.clone(), Some(props))?;

    let batch = RecordBatch::try_new(
        schema,
        vec![
            // Identifiers
            Arc::new(UInt32Array::from_iter_values(rows.iter().map(|r| r.beatmap_id))),
            Arc::new(UInt32Array::from_iter_values(rows.iter().map(|r| r.beatmapset_id))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.folder_id.as_str()))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.osu_file.as_str()))),
            
            // API Metadata
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.mode.as_str()))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.version.as_str()))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.url.as_str()))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.status.as_str()))),
            Arc::new(BooleanArray::from_iter(rows.iter().map(|r| Some(r.is_scoreable)))),
            Arc::new(BooleanArray::from_iter(rows.iter().map(|r| Some(r.convert)))),
            
            // Difficulty settings
            Arc::new(Float32Array::from_iter_values(rows.iter().map(|r| r.ar))),
            Arc::new(Float32Array::from_iter_values(rows.iter().map(|r| r.cs))),
            Arc::new(Float32Array::from_iter_values(rows.iter().map(|r| r.od))),
            Arc::new(Float32Array::from_iter_values(rows.iter().map(|r| r.hp))),
            Arc::new(Float32Array::from_iter_values(rows.iter().map(|r| r.bpm))),
            
            // Counts
            Arc::new(UInt32Array::from_iter_values(rows.iter().map(|r| r.count_circles))),
            Arc::new(UInt32Array::from_iter_values(rows.iter().map(|r| r.count_sliders))),
            Arc::new(UInt32Array::from_iter_values(rows.iter().map(|r| r.count_spinners))),
            
            // Length
            Arc::new(UInt32Array::from_iter_values(rows.iter().map(|r| r.seconds_drain))),
            Arc::new(UInt32Array::from_iter_values(rows.iter().map(|r| r.seconds_total))),
            
            // Stats
            Arc::new(UInt32Array::from_iter_values(rows.iter().map(|r| r.playcount))),
            Arc::new(UInt32Array::from_iter_values(rows.iter().map(|r| r.passcount))),
            Arc::new(UInt32Array::from(rows.iter().map(|r| r.max_combo_api).collect::<Vec<_>>())),
            Arc::new(Float32Array::from_iter_values(rows.iter().map(|r| r.stars_api))),
            
            // Other
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.checksum.as_str()))),
            Arc::new(UInt32Array::from_iter_values(rows.iter().map(|r| r.creator_id))),
            Arc::new(Int64Array::from(rows.iter().map(|r| r.last_updated).collect::<Vec<_>>())),
            
            // PP Calculation
            Arc::new(Float64Array::from_iter_values(rows.iter().map(|r| r.stars_calc))),
            Arc::new(Float64Array::from_iter_values(rows.iter().map(|r| r.max_pp))),
            Arc::new(UInt32Array::from_iter_values(rows.iter().map(|r| r.max_combo_calc))),
            
            // osu! specific
            Arc::new(Float64Array::from(rows.iter().map(|r| r.osu_aim).collect::<Vec<_>>())),
            Arc::new(Float64Array::from(rows.iter().map(|r| r.osu_speed).collect::<Vec<_>>())),
            Arc::new(Float64Array::from(rows.iter().map(|r| r.osu_flashlight).collect::<Vec<_>>())),
            Arc::new(Float64Array::from(rows.iter().map(|r| r.osu_slider_factor).collect::<Vec<_>>())),
            Arc::new(Float64Array::from(rows.iter().map(|r| r.osu_speed_note_count).collect::<Vec<_>>())),
            Arc::new(Float64Array::from(rows.iter().map(|r| r.osu_aim_difficult_slider_count).collect::<Vec<_>>())),
            Arc::new(Float64Array::from(rows.iter().map(|r| r.osu_aim_difficult_strain_count).collect::<Vec<_>>())),
            Arc::new(Float64Array::from(rows.iter().map(|r| r.osu_speed_difficult_strain_count).collect::<Vec<_>>())),
            Arc::new(Float64Array::from(rows.iter().map(|r| r.osu_great_hit_window).collect::<Vec<_>>())),
            Arc::new(Float64Array::from(rows.iter().map(|r| r.osu_ok_hit_window).collect::<Vec<_>>())),
            Arc::new(Float64Array::from(rows.iter().map(|r| r.osu_meh_hit_window).collect::<Vec<_>>())),
            Arc::new(UInt32Array::from(rows.iter().map(|r| r.osu_n_large_ticks).collect::<Vec<_>>())),
            
            // taiko specific
            Arc::new(Float64Array::from(rows.iter().map(|r| r.taiko_stamina).collect::<Vec<_>>())),
            Arc::new(Float64Array::from(rows.iter().map(|r| r.taiko_rhythm).collect::<Vec<_>>())),
            Arc::new(Float64Array::from(rows.iter().map(|r| r.taiko_color).collect::<Vec<_>>())),
            Arc::new(Float64Array::from(rows.iter().map(|r| r.taiko_reading).collect::<Vec<_>>())),
            Arc::new(Float64Array::from(rows.iter().map(|r| r.taiko_great_hit_window).collect::<Vec<_>>())),
            Arc::new(Float64Array::from(rows.iter().map(|r| r.taiko_ok_hit_window).collect::<Vec<_>>())),
            Arc::new(Float64Array::from(rows.iter().map(|r| r.taiko_mono_stamina_factor).collect::<Vec<_>>())),
            
            // catch specific
            Arc::new(Float64Array::from(rows.iter().map(|r| r.catch_ar).collect::<Vec<_>>())),
            Arc::new(UInt32Array::from(rows.iter().map(|r| r.catch_n_fruits).collect::<Vec<_>>())),
            Arc::new(UInt32Array::from(rows.iter().map(|r| r.catch_n_droplets).collect::<Vec<_>>())),
            Arc::new(UInt32Array::from(rows.iter().map(|r| r.catch_n_tiny_droplets).collect::<Vec<_>>())),
            
            // mania specific
            Arc::new(UInt32Array::from(rows.iter().map(|r| r.mania_n_objects).collect::<Vec<_>>())),
            Arc::new(UInt32Array::from(rows.iter().map(|r| r.mania_n_hold_notes).collect::<Vec<_>>())),
            
            // Common
            Arc::new(BooleanArray::from(rows.iter().map(|r| r.is_convert).collect::<Vec<_>>())),
        ],
    )?;

    writer.write(&batch)?;
    writer.close()?;
    Ok(())
}

fn comments_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("beatmapset_id", DataType::UInt32, false),
        Field::new("comment_id", DataType::UInt32, false),
        Field::new("parent_id", DataType::UInt32, true),
        Field::new("user_id", DataType::UInt32, true),
        Field::new("legacy_name", DataType::Utf8, true),
        Field::new("message", DataType::Utf8, true),
        Field::new("message_html", DataType::Utf8, true),
        Field::new("votes_count", DataType::UInt32, false),
        Field::new("replies_count", DataType::UInt32, false),
        Field::new("pinned", DataType::Boolean, false),
        Field::new("commentable_type", DataType::Utf8, false),
        Field::new("created_at", DataType::Int64, false),
        Field::new("updated_at", DataType::Int64, false),
        Field::new("edited_at", DataType::Int64, true),
        Field::new("edited_by_id", DataType::UInt32, true),
        Field::new("deleted_at", DataType::Int64, true),
    ]))
}

fn write_comments_parquet(path: &Path, rows: &[CommentRow]) -> Result<()> {
    let schema = comments_schema();
    let file = File::create(path)?;
    let props = WriterProperties::builder()
        .set_compression(parquet::basic::Compression::SNAPPY)
        .build();
    let mut writer = ArrowWriter::try_new(file, schema.clone(), Some(props))?;

    let batch = RecordBatch::try_new(
        schema,
        vec![
            Arc::new(UInt32Array::from_iter_values(rows.iter().map(|r| r.beatmapset_id))),
            Arc::new(UInt32Array::from_iter_values(rows.iter().map(|r| r.comment_id))),
            Arc::new(UInt32Array::from(rows.iter().map(|r| r.parent_id).collect::<Vec<_>>())),
            Arc::new(UInt32Array::from(rows.iter().map(|r| r.user_id).collect::<Vec<_>>())),
            Arc::new(StringArray::from(rows.iter().map(|r| r.legacy_name.as_deref()).collect::<Vec<_>>())),
            Arc::new(StringArray::from(rows.iter().map(|r| r.message.as_deref()).collect::<Vec<_>>())),
            Arc::new(StringArray::from(rows.iter().map(|r| r.message_html.as_deref()).collect::<Vec<_>>())),
            Arc::new(UInt32Array::from_iter_values(rows.iter().map(|r| r.votes_count))),
            Arc::new(UInt32Array::from_iter_values(rows.iter().map(|r| r.replies_count))),
            Arc::new(BooleanArray::from_iter(rows.iter().map(|r| Some(r.pinned)))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.commentable_type.as_str()))),
            Arc::new(Int64Array::from_iter_values(rows.iter().map(|r| r.created_at))),
            Arc::new(Int64Array::from_iter_values(rows.iter().map(|r| r.updated_at))),
            Arc::new(Int64Array::from(rows.iter().map(|r| r.edited_at).collect::<Vec<_>>())),
            Arc::new(UInt32Array::from(rows.iter().map(|r| r.edited_by_id).collect::<Vec<_>>())),
            Arc::new(Int64Array::from(rows.iter().map(|r| r.deleted_at).collect::<Vec<_>>())),
        ],
    )?;

    writer.write(&batch)?;
    writer.close()?;
    Ok(())
}
