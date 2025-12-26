use anyhow::{Context, Result};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use arrow::array::{Array, StringArray};
use rosu_map::Beatmap;
use rosu_storyboard::Storyboard;
use std::collections::HashSet;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use walkdir::WalkDir;
use rand::seq::SliceRandom;
use rand::rng;

mod batch_writer;

/// Build parquet dataset from osu! beatmap folders
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// Path to source directory containing extracted beatmap folders
    #[arg(long, default_value = r"E:\osu_model\osu_archives_extracted")]
    input_dir: PathBuf,

    /// Path to output directory for parquet files
    #[arg(long, default_value = r"E:\osu_model\dataset")]
    output_dir: PathBuf,

    /// Force rebuild, ignoring existing parquet data
    #[arg(long, short)]
    force: bool,

    /// Test mode: only process 10 random folders
    #[arg(long)]
    test: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    
    let assets_dir = args.output_dir.join("assets");
    fs::create_dir_all(&args.output_dir)?;
    fs::create_dir_all(&assets_dir)?;

    // Read existing processed folder_ids unless --force
    let existing_folder_ids: HashSet<String> = if !args.force {
        read_existing_folder_ids(&args.output_dir)
    } else {
        HashSet::new()
    };

    // Load failed folders list (format: "folder_name: reason")
    let failed_path = args.output_dir.join("failed_folders.txt");
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
    let failed_folder_set: HashSet<String> = failed_lines
        .iter()
        .map(|line| line.split(':').next().unwrap_or("").trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let mut failed_folders: HashSet<String> = failed_lines.into_iter().collect();
    let initial_failed_count = failed_folder_set.len();

    if !existing_folder_ids.is_empty() {
        println!("Found {} already processed folders (use --force to rebuild)", existing_folder_ids.len());
    }
    if initial_failed_count > 0 {
        println!("Skipping {} permanently failed folders", initial_failed_count);
    }

    let mut folders: Vec<PathBuf> = fs::read_dir(&args.input_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.path())
        .filter(|p| {
            // Skip already processed and failed folders
            let folder_name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
            !existing_folder_ids.contains(&folder_name) && !failed_folder_set.contains(&folder_name)
        })
        .collect();

    if args.test {
        let mut rng = rng();
        folders.shuffle(&mut rng);
        folders.truncate(10);
        println!("TEST MODE: Processing 10 random folders");
    }

    if folders.is_empty() {
        println!("No new beatmap folders to process.");
        return Ok(());
    }

    println!("Found {} new beatmap folders to process", folders.len());

    let pb = ProgressBar::new(folders.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("#>-"),
    );

    // Initialize batch writers for memory-efficient parquet writing
    // Append mode: existing parquet files will have new data appended
    let mut writers = batch_writer::DatasetWriters::new(&args.output_dir)?;

    // Set up graceful shutdown
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let shutdown_clone = shutdown_requested.clone();
    ctrlc::set_handler(move || {
        println!("\n⏳ Ctrl+C received! Finishing current folder then stopping...");
        shutdown_clone.store(true, Ordering::SeqCst);
    }).expect("Error setting Ctrl+C handler");

    let mut success_count = 0;
    let mut failure_count = 0;
    let mut interrupted = false;

    for folder in &folders {
        // Check if shutdown was requested
        if shutdown_requested.load(Ordering::SeqCst) {
            pb.println("🛑 Stopping gracefully...");
            interrupted = true;
            break;
        }

        pb.inc(1);
        match process_folder_batch(folder, &mut writers, &assets_dir) {
            Ok(()) => success_count += 1,
            Err(e) => {
                failure_count += 1;
                let folder_name = folder.file_name().unwrap_or_default().to_string_lossy().to_string();
                failed_folders.insert(format!("{}: {}", folder_name, e));
                pb.println(format!("Error: {}: {}", folder.display(), e));
            }
        }
    }

    pb.finish_with_message("Processing complete!");

    println!("\n=== Writing Parquet Files ===");
    let stats = writers.close()?;
    println!("  beatmaps.parquet: {} rows", stats.beatmaps);
    println!("  hit_objects.parquet: {} rows", stats.hit_objects);
    println!("  timing_points.parquet: {} rows", stats.timing_points);
    println!("  storyboard_elements.parquet: {} rows", stats.storyboard_elements);
    println!("  storyboard_commands.parquet: {} rows", stats.storyboard_commands);
    println!("  slider_control_points.parquet: {} rows", stats.slider_control_points);
    println!("  slider_data.parquet: {} rows", stats.slider_data);
    println!("  breaks.parquet: {} rows", stats.breaks);
    println!("  combo_colors.parquet: {} rows", stats.combo_colors);
    println!("  hit_samples.parquet: {} rows", stats.hit_samples);
    println!("  storyboard_loops.parquet: {} rows", stats.storyboard_loops);
    println!("  storyboard_triggers.parquet: {} rows", stats.storyboard_triggers);

    println!("\n=== Results ===");
    println!("Success: {}", success_count);
    println!("Failed: {}", failure_count);
    if interrupted {
        println!("⚠ Run was interrupted by Ctrl+C");
    }

    // Save failed list if there are new failures
    let new_failures = failed_folders.len() - initial_failed_count;
    if new_failures > 0 {
        let content: String = failed_folders.iter().map(|s| format!("{}\n", s)).collect();
        let _ = std::fs::write(&failed_path, content);
        println!("Added {} folders to failed_folders.txt", new_failures);
    }

    // Note: Round-trip verification is not available in batch mode
    // since data is written directly to parquet files.
    // Use osu-reconstructor library to verify data integrity.

    Ok(())
}

// ============ Data Structures ============

struct BeatmapRow {
    folder_id: String,
    osu_file: String,
    format_version: i32,
    audio_file: String,
    audio_lead_in: f64,
    preview_time: i32,
    // General section - new fields
    default_sample_bank: i32,  // SampleBank enum: 0=Auto, 1=Normal, 2=Soft, 3=Drum
    default_sample_volume: i32,
    stack_leniency: f32,
    mode: i32,
    letterbox_in_breaks: bool,
    special_style: bool,  // Mania-specific
    widescreen_storyboard: bool,
    epilepsy_warning: bool,
    samples_match_playback_rate: bool,
    countdown: i32,
    countdown_offset: i32,
    // Editor section
    bookmarks: String,  // JSON array of i32
    distance_spacing: f64,
    beat_divisor: i32,
    grid_size: i32,
    timeline_zoom: f64,
    // Metadata section
    title: String,
    title_unicode: String,
    artist: String,
    artist_unicode: String,
    creator: String,
    version: String,
    source: String,
    tags: String,
    beatmap_id: i32,
    beatmap_set_id: i32,
    // Difficulty section
    hp_drain_rate: f32,
    circle_size: f32,
    overall_difficulty: f32,
    approach_rate: f32,
    slider_multiplier: f64,
    slider_tick_rate: f64,
    // Events section
    background_file: String,
    audio_path: String,
    background_path: String,
}

struct HitObjectRow {
    folder_id: String,
    osu_file: String,
    index: i32,
    start_time: f64,
    object_type: String,
    // Circle/Slider/Spinner specific
    pos_x: Option<i32>,
    pos_y: Option<i32>,
    new_combo: bool,
    combo_offset: i32,  // How many combo colors to skip
    // Slider specific
    curve_type: Option<String>,
    slides: Option<i32>,
    length: Option<f64>,
    // Spinner specific
    end_time: Option<f64>,
}

struct TimingPointRow {
    folder_id: String,
    osu_file: String,
    time: f64,
    point_type: String,
    beat_length: Option<f64>,
    time_signature: Option<String>,
    slider_velocity: Option<f64>,
    kiai: Option<bool>,
    // Sample settings (from SamplePoint)
    sample_bank: Option<String>,
    sample_volume: Option<i32>,
}

struct StoryboardElementRow {
    folder_id: String,
    source_file: String,
    element_index: i32,
    layer_name: String,
    element_path: String,
    element_type: String,
    // Sprite data
    origin: String,  // "TopLeft", "Centre", etc.
    initial_pos_x: f32,
    initial_pos_y: f32,
    // Animation specific
    frame_count: Option<i32>,
    frame_delay: Option<f64>,
    loop_type: Option<String>,
    // True if storyboard was embedded in .osu file, false if from standalone .osb
    is_embedded: bool,
}

// Store storyboard commands (one row per command)
struct StoryboardCommandRow {
    folder_id: String,
    source_file: String,
    element_index: i32,
    command_type: String,  // "x", "y", "scale", "rotation", "alpha", "color", "flip_h", "flip_v", "blending"
    start_time: f64,
    end_time: f64,
    // Values stored as strings for flexibility (color is RGB, pos is x,y, etc.)
    start_value: String,
    end_value: String,
    easing: i32,  // Easing function index
    // True if storyboard was embedded in .osu file, false if from standalone .osb
    is_embedded: bool,
}

// Separate table for slider control points (one row per control point)
struct SliderControlPointRow {
    folder_id: String,
    osu_file: String,
    hit_object_index: i32,
    point_index: i32,
    pos_x: f32,
    pos_y: f32,
    path_type: Option<String>,  // "Bezier", "Linear", "Catmull", "PerfectCurve"
}

// Additional slider data stored in hit_objects extended fields
struct SliderDataRow {
    folder_id: String,
    osu_file: String,
    hit_object_index: i32,
    repeat_count: i32,
    velocity: f64,
    expected_dist: Option<f64>,
}

// Break periods during gameplay
struct BreakRow {
    folder_id: String,
    osu_file: String,
    start_time: f64,
    end_time: f64,
}

// Combo colors
struct ComboColorRow {
    folder_id: String,
    osu_file: String,
    color_index: i32,
    color_type: String,  // "combo" or "custom"
    custom_name: Option<String>,  // For custom colors like "SliderTrackOverride"
    red: i32,
    green: i32,
    blue: i32,
}

// HitSound samples per hit object
struct HitSampleRow {
    folder_id: String,
    osu_file: String,
    hit_object_index: i32,
    sample_index: i32,
    name: String,  // "normal", "whistle", "finish", "clap"
    bank: String,  // "Normal", "Soft", "Drum"
    suffix: Option<String>,  // Custom sample suffix
    volume: i32,
}

// Storyboard loops
struct StoryboardLoopRow {
    folder_id: String,
    source_file: String,
    element_index: i32,
    loop_index: i32,
    loop_start_time: f64,
    loop_count: i32,
    is_embedded: bool,
}

// Storyboard triggers
struct StoryboardTriggerRow {
    folder_id: String,
    source_file: String,
    element_index: i32,
    trigger_index: i32,
    trigger_name: String,
    trigger_start_time: f64,
    trigger_end_time: f64,
    group_number: i32,
    is_embedded: bool,
}

// ============ Processing ============

/// Read existing folder_ids from beatmaps.parquet
fn read_existing_folder_ids(output_dir: &Path) -> HashSet<String> {
    let beatmaps_path = output_dir.join("beatmaps.parquet");
    if !beatmaps_path.exists() {
        return HashSet::new();
    }

    let mut folder_ids = HashSet::new();
    
    if let Ok(file) = File::open(&beatmaps_path) {
        if let Ok(reader) = ParquetRecordBatchReaderBuilder::try_new(file) {
            if let Ok(reader) = reader.build() {
                for batch in reader.flatten() {
                    if let Some(col) = batch.column_by_name("folder_id") {
                        if let Some(arr) = col.as_any().downcast_ref::<StringArray>() {
                            for i in 0..arr.len() {
                                if !arr.is_null(i) {
                                    folder_ids.insert(arr.value(i).to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    folder_ids
}

/// Batch version of process_folder that writes directly to parquet writers
fn process_folder_batch(
    source_folder: &Path,
    writers: &mut batch_writer::DatasetWriters,
    assets_dir: &Path,
) -> Result<()> {
    let folder_id = source_folder
        .file_name()
        .context("Invalid folder name")?
        .to_string_lossy()
        .to_string();

    let assets_folder = assets_dir.join(&folder_id);
    let mut assets: HashSet<String> = HashSet::new();

    // Find all .osu files
    let mut osu_files: Vec<PathBuf> = Vec::new();
    for entry in WalkDir::new(source_folder).max_depth(1) {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext.to_string_lossy().to_lowercase() == "osu" {
                    osu_files.push(path.to_path_buf());
                }
            }
        }
    }

    if osu_files.is_empty() {
        anyhow::bail!("No .osu files found");
    }

    // Process each .osu file
    for osu_path in &osu_files {
        let osu_filename = osu_path.file_name().unwrap().to_string_lossy().to_string();

        // Parse beatmap
        let beatmap: Beatmap = rosu_map::from_path(osu_path)
            .with_context(|| format!("Failed to parse: {}", osu_path.display()))?;

        // Collect assets
        if !beatmap.audio_file.is_empty() {
            assets.insert(beatmap.audio_file.clone());
        }
        if !beatmap.background_file.is_empty() {
            assets.insert(beatmap.background_file.clone());
        }

        // Build asset paths
        let audio_path = if !beatmap.audio_file.is_empty() {
            format!("assets/{}/{}", folder_id, beatmap.audio_file)
        } else {
            String::new()
        };
        let background_path = if !beatmap.background_file.is_empty() {
            format!("assets/{}/{}", folder_id, beatmap.background_file)
        } else {
            String::new()
        };

        // Write beatmap row
        writers.beatmaps.write(BeatmapRow {
            folder_id: folder_id.clone(),
            osu_file: osu_filename.clone(),
            format_version: beatmap.format_version,
            audio_file: beatmap.audio_file.clone(),
            audio_lead_in: beatmap.audio_lead_in,
            preview_time: beatmap.preview_time,
            // General section - new fields
            default_sample_bank: beatmap.default_sample_bank as i32,
            default_sample_volume: beatmap.default_sample_volume,
            stack_leniency: beatmap.stack_leniency,
            mode: beatmap.mode as i32,
            letterbox_in_breaks: beatmap.letterbox_in_breaks,
            special_style: beatmap.special_style,
            widescreen_storyboard: beatmap.widescreen_storyboard,
            epilepsy_warning: beatmap.epilepsy_warning,
            samples_match_playback_rate: beatmap.samples_match_playback_rate,
            countdown: beatmap.countdown as i32,
            countdown_offset: beatmap.countdown_offset,
            // Editor section
            bookmarks: beatmap.bookmarks.iter().map(|b| b.to_string()).collect::<Vec<_>>().join(","),
            distance_spacing: beatmap.distance_spacing,
            beat_divisor: beatmap.beat_divisor,
            grid_size: beatmap.grid_size,
            timeline_zoom: beatmap.timeline_zoom,
            // Metadata section
            title: beatmap.title.clone(),
            title_unicode: beatmap.title_unicode.clone(),
            artist: beatmap.artist.clone(),
            artist_unicode: beatmap.artist_unicode.clone(),
            creator: beatmap.creator.clone(),
            version: beatmap.version.clone(),
            source: beatmap.source.clone(),
            tags: beatmap.tags.clone(),
            beatmap_id: beatmap.beatmap_id,
            beatmap_set_id: beatmap.beatmap_set_id,
            // Difficulty section
            hp_drain_rate: beatmap.hp_drain_rate,
            circle_size: beatmap.circle_size,
            overall_difficulty: beatmap.overall_difficulty,
            approach_rate: beatmap.approach_rate,
            slider_multiplier: beatmap.slider_multiplier,
            slider_tick_rate: beatmap.slider_tick_rate,
            // Events section
            background_file: beatmap.background_file.clone(),
            audio_path,
            background_path,
        })?;

        // Write hit objects
        for (idx, ho) in beatmap.hit_objects.iter().enumerate() {
            let (obj_type, pos_x, pos_y, new_combo, curve_type, slides, length, end_time) =
                extract_hit_object_info(ho);

            writers.hit_objects.write(HitObjectRow {
                folder_id: folder_id.clone(),
                osu_file: osu_filename.clone(),
                index: idx as i32,
                start_time: ho.start_time,
                object_type: obj_type,
                pos_x,
                pos_y,
                new_combo,
                combo_offset: extract_combo_offset(ho),
                curve_type: curve_type.clone(),
                slides,
                length,
                end_time,
            })?;

            // Write slider data if applicable
            if let rosu_map::section::hit_objects::HitObjectKind::Slider(s) = &ho.kind {
                writers.slider_data.write(SliderDataRow {
                    folder_id: folder_id.clone(),
                    osu_file: osu_filename.clone(),
                    hit_object_index: idx as i32,
                    repeat_count: s.repeat_count,
                    velocity: s.velocity,
                    expected_dist: s.path.expected_dist(),
                })?;

                for (cp_idx, cp) in s.path.control_points().iter().enumerate() {
                    writers.slider_control_points.write(SliderControlPointRow {
                        folder_id: folder_id.clone(),
                        osu_file: osu_filename.clone(),
                        hit_object_index: idx as i32,
                        point_index: cp_idx as i32,
                        pos_x: cp.pos.x,
                        pos_y: cp.pos.y,
                        path_type: cp.path_type.map(|pt| format!("{:?}", pt)),
                    })?;
                }
            }
        }

        // Write timing points
        for tp in &beatmap.control_points.timing_points {
            writers.timing_points.write(TimingPointRow {
                folder_id: folder_id.clone(),
                osu_file: osu_filename.clone(),
                time: tp.time,
                point_type: "timing".to_string(),
                beat_length: Some(tp.beat_len),
                time_signature: Some(format!("{:?}", tp.time_signature)),
                slider_velocity: None,
                kiai: None,
                sample_bank: None,
                sample_volume: None,
            })?;
        }

        for dp in &beatmap.control_points.difficulty_points {
            writers.timing_points.write(TimingPointRow {
                folder_id: folder_id.clone(),
                osu_file: osu_filename.clone(),
                time: dp.time,
                point_type: "difficulty".to_string(),
                beat_length: None,
                time_signature: None,
                slider_velocity: Some(dp.slider_velocity),
                kiai: None,
                sample_bank: None,
                sample_volume: None,
            })?;
        }

        for ep in &beatmap.control_points.effect_points {
            writers.timing_points.write(TimingPointRow {
                folder_id: folder_id.clone(),
                osu_file: osu_filename.clone(),
                time: ep.time,
                point_type: "effect".to_string(),
                beat_length: None,
                time_signature: None,
                slider_velocity: None,
                kiai: Some(ep.kiai),
                sample_bank: None,
                sample_volume: None,
            })?;
        }

        // Write break periods
        for break_period in &beatmap.breaks {
            writers.breaks.write(BreakRow {
                folder_id: folder_id.clone(),
                osu_file: osu_filename.clone(),
                start_time: break_period.start_time,
                end_time: break_period.end_time,
            })?;
        }

        // Write combo colors
        for (idx, color) in beatmap.custom_combo_colors.iter().enumerate() {
            writers.combo_colors.write(ComboColorRow {
                folder_id: folder_id.clone(),
                osu_file: osu_filename.clone(),
                color_index: idx as i32,
                color_type: "combo".to_string(),
                custom_name: None,
                red: color.red() as i32,
                green: color.green() as i32,
                blue: color.blue() as i32,
            })?;
        }

        // Write custom colors (slider track, etc.)
        for (idx, custom_color) in beatmap.custom_colors.iter().enumerate() {
            writers.combo_colors.write(ComboColorRow {
                folder_id: folder_id.clone(),
                osu_file: osu_filename.clone(),
                color_index: idx as i32,
                color_type: "custom".to_string(),
                custom_name: Some(custom_color.name.clone()),
                red: custom_color.color.red() as i32,
                green: custom_color.color.green() as i32,
                blue: custom_color.color.blue() as i32,
            })?;
        }

        // Write hit samples for each hit object
        for (ho_idx, ho) in beatmap.hit_objects.iter().enumerate() {
            for (sample_idx, sample) in ho.samples.iter().enumerate() {
                writers.hit_samples.write(HitSampleRow {
                    folder_id: folder_id.clone(),
                    osu_file: osu_filename.clone(),
                    hit_object_index: ho_idx as i32,
                    sample_index: sample_idx as i32,
                    name: format!("{:?}", sample.name),
                    bank: format!("{:?}", sample.bank),
                    suffix: sample.suffix.map(|s| s.get().to_string()),
                    volume: sample.volume,
                })?;
            }
        }

        // Parse storyboard from .osu file (storyboards are often embedded in .osu files)
        if let Ok(storyboard) = Storyboard::from_path(osu_path) {
            let mut element_index = 0i32;
            
            use rosu_storyboard::element::ElementKind;
            
            for (layer_name, layer) in &storyboard.layers {
                for element in &layer.elements {
                    let (element_type, origin, initial_pos_x, initial_pos_y, 
                         frame_count, frame_delay, loop_type, tg) = match &element.kind {
                        ElementKind::Sprite(s) => {
                            (
                                "sprite",
                                format!("{:?}", s.origin),
                                s.initial_pos.x,
                                s.initial_pos.y,
                                None, None, None,
                                Some(&s.timeline_group),
                            )
                        }
                        ElementKind::Animation(a) => {
                            (
                                "animation",
                                format!("{:?}", a.sprite.origin),
                                a.sprite.initial_pos.x,
                                a.sprite.initial_pos.y,
                                Some(a.frame_count as i32),
                                Some(a.frame_delay),
                                Some(format!("{:?}", a.loop_kind)),
                                Some(&a.sprite.timeline_group),
                            )
                        }
                        ElementKind::Sample(_) => {
                            ("sample", String::new(), 0.0, 0.0, 
                             None, None, None, None)
                        }
                        ElementKind::Video(_) => {
                            ("video", String::new(), 0.0, 0.0,
                             None, None, None, None)
                        }
                    };
                    
                    // Add asset path for sprites/animations/videos
                    if !element.path.is_empty() {
                        assets.insert(element.path.clone());
                    }

                    writers.storyboard_elements.write(StoryboardElementRow {
                        folder_id: folder_id.clone(),
                        source_file: osu_filename.clone(),
                        element_index,
                        layer_name: layer_name.to_string(),
                        element_path: element.path.clone(),
                        element_type: element_type.to_string(),
                        origin,
                        initial_pos_x,
                        initial_pos_y,
                        frame_count,
                        frame_delay,
                        loop_type,
                        is_embedded: true,
                    })?;

                    // Write commands for this element
                    if let Some(tg) = tg {
                        macro_rules! add_commands {
                            ($cmd_type:expr, $timeline:expr, $format_fn:expr) => {
                                for cmd in $timeline.commands() {
                                    writers.storyboard_commands.write(StoryboardCommandRow {
                                        folder_id: folder_id.clone(),
                                        source_file: osu_filename.clone(),
                                        element_index,
                                        command_type: $cmd_type.to_string(),
                                        start_time: cmd.start_time,
                                        end_time: cmd.end_time,
                                        start_value: $format_fn(&cmd.start_value),
                                        end_value: $format_fn(&cmd.end_value),
                                        easing: cmd.easing as i32,
                                        is_embedded: true,
                                    })?;
                                }
                            };
                        }

                        add_commands!("x", tg.x, |v: &f32| v.to_string());
                        add_commands!("y", tg.y, |v: &f32| v.to_string());
                        add_commands!("scale", tg.scale, |v: &f32| v.to_string());
                        add_commands!("rotation", tg.rotation, |v: &f32| v.to_string());
                        add_commands!("alpha", tg.alpha, |v: &f32| v.to_string());
                        add_commands!("color", tg.color, |v: &rosu_storyboard::reexport::Color| format!("{},{},{}", v[0], v[1], v[2]));
                        add_commands!("flip_h", tg.flip_h, |v: &bool| v.to_string());
                        add_commands!("flip_v", tg.flip_v, |v: &bool| v.to_string());
                        add_commands!("vector_scale", tg.vector_scale, |v: &rosu_storyboard::reexport::Pos| format!("{},{}", v.x, v.y));
                        add_commands!("blending", tg.blending_parameters, |_: &rosu_storyboard::visual::BlendingParameters| "A".to_string());
                    }

                    // Write loops and triggers for sprites/animations
                    match &element.kind {
                        ElementKind::Sprite(s) => {
                            for (loop_idx, cmd_loop) in s.loops.iter().enumerate() {
                                writers.storyboard_loops.write(StoryboardLoopRow {
                                    folder_id: folder_id.clone(),
                                    source_file: osu_filename.clone(),
                                    element_index,
                                    loop_index: loop_idx as i32,
                                    loop_start_time: cmd_loop.loop_start_time,
                                    loop_count: cmd_loop.total_iterations as i32,
                                    is_embedded: true,
                                })?;
                            }
                            for (trigger_idx, trigger) in s.triggers.iter().enumerate() {
                                writers.storyboard_triggers.write(StoryboardTriggerRow {
                                    folder_id: folder_id.clone(),
                                    source_file: osu_filename.clone(),
                                    element_index,
                                    trigger_index: trigger_idx as i32,
                                    trigger_name: trigger.name.clone(),
                                    trigger_start_time: trigger.start_time,
                                    trigger_end_time: trigger.end_time,
                                    group_number: trigger.group_num,
                                    is_embedded: true,
                                })?;
                            }
                        }
                        ElementKind::Animation(a) => {
                            for (loop_idx, cmd_loop) in a.sprite.loops.iter().enumerate() {
                                writers.storyboard_loops.write(StoryboardLoopRow {
                                    folder_id: folder_id.clone(),
                                    source_file: osu_filename.clone(),
                                    element_index,
                                    loop_index: loop_idx as i32,
                                    loop_start_time: cmd_loop.loop_start_time,
                                    loop_count: cmd_loop.total_iterations as i32,
                                    is_embedded: true,
                                })?;
                            }
                            for (trigger_idx, trigger) in a.sprite.triggers.iter().enumerate() {
                                writers.storyboard_triggers.write(StoryboardTriggerRow {
                                    folder_id: folder_id.clone(),
                                    source_file: osu_filename.clone(),
                                    element_index,
                                    trigger_index: trigger_idx as i32,
                                    trigger_name: trigger.name.clone(),
                                    trigger_start_time: trigger.start_time,
                                    trigger_end_time: trigger.end_time,
                                    group_number: trigger.group_num,
                                    is_embedded: true,
                                })?;
                            }
                        }
                        _ => {}
                    }

                    element_index += 1;
                }
            }
        }
    }

    // Process standalone .osb storyboard files
    for entry in WalkDir::new(source_folder).max_depth(1) {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext.to_string_lossy().to_lowercase() == "osb" {
                    if let Ok(storyboard) = Storyboard::from_path(path) {
                        let source_file = path.file_name().unwrap().to_string_lossy().to_string();
                        let mut element_index = 0i32;

                        use rosu_storyboard::element::ElementKind;
                        
                        for (layer_name, layer) in &storyboard.layers {
                            for element in &layer.elements {
                                let (element_type, origin, initial_pos_x, initial_pos_y, 
                                     frame_count, frame_delay, loop_type, tg) = match &element.kind {
                                    ElementKind::Sprite(s) => {
                                        (
                                            "sprite",
                                            format!("{:?}", s.origin),
                                            s.initial_pos.x,
                                            s.initial_pos.y,
                                            None, None, None,
                                            Some(&s.timeline_group),
                                        )
                                    }
                                    ElementKind::Animation(a) => {
                                        (
                                            "animation",
                                            format!("{:?}", a.sprite.origin),
                                            a.sprite.initial_pos.x,
                                            a.sprite.initial_pos.y,
                                            Some(a.frame_count as i32),
                                            Some(a.frame_delay),
                                            Some(format!("{:?}", a.loop_kind)),
                                            Some(&a.sprite.timeline_group),
                                        )
                                    }
                                    ElementKind::Sample(_) => {
                                        ("sample", String::new(), 0.0, 0.0, 
                                         None, None, None, None)
                                    }
                                    ElementKind::Video(_) => {
                                        ("video", String::new(), 0.0, 0.0,
                                         None, None, None, None)
                                    }
                                };
                                
                                // Add asset path for sprites/animations/videos
                                if !element.path.is_empty() {
                                    assets.insert(element.path.clone());
                                }

                                writers.storyboard_elements.write(StoryboardElementRow {
                                    folder_id: folder_id.clone(),
                                    source_file: source_file.clone(),
                                    element_index,
                                    layer_name: layer_name.to_string(),
                                    element_path: element.path.clone(),
                                    element_type: element_type.to_string(),
                                    origin,
                                    initial_pos_x,
                                    initial_pos_y,
                                    frame_count,
                                    frame_delay,
                                    loop_type,
                                    is_embedded: false,
                                })?;

                                // Write commands for this element
                                if let Some(tg) = tg {
                                    macro_rules! add_commands {
                                        ($cmd_type:expr, $timeline:expr, $format_fn:expr) => {
                                            for cmd in $timeline.commands() {
                                                writers.storyboard_commands.write(StoryboardCommandRow {
                                                    folder_id: folder_id.clone(),
                                                    source_file: source_file.clone(),
                                                    element_index,
                                                    command_type: $cmd_type.to_string(),
                                                    start_time: cmd.start_time,
                                                    end_time: cmd.end_time,
                                                    start_value: $format_fn(&cmd.start_value),
                                                    end_value: $format_fn(&cmd.end_value),
                                                    easing: cmd.easing as i32,
                                                    is_embedded: false,
                                                })?;
                                            }
                                        };
                                    }

                                    add_commands!("x", tg.x, |v: &f32| v.to_string());
                                    add_commands!("y", tg.y, |v: &f32| v.to_string());
                                    add_commands!("scale", tg.scale, |v: &f32| v.to_string());
                                    add_commands!("rotation", tg.rotation, |v: &f32| v.to_string());
                                    add_commands!("alpha", tg.alpha, |v: &f32| v.to_string());
                                    add_commands!("color", tg.color, |v: &rosu_storyboard::reexport::Color| format!("{},{},{}", v[0], v[1], v[2]));
                                    add_commands!("flip_h", tg.flip_h, |v: &bool| v.to_string());
                                    add_commands!("flip_v", tg.flip_v, |v: &bool| v.to_string());
                                    add_commands!("vector_scale", tg.vector_scale, |v: &rosu_storyboard::reexport::Pos| format!("{},{}", v.x, v.y));
                                    add_commands!("blending", tg.blending_parameters, |_: &rosu_storyboard::visual::BlendingParameters| "A".to_string());
                                }

                                // Write loops and triggers for sprites/animations
                                match &element.kind {
                                    ElementKind::Sprite(s) => {
                                        for (loop_idx, cmd_loop) in s.loops.iter().enumerate() {
                                            writers.storyboard_loops.write(StoryboardLoopRow {
                                                folder_id: folder_id.clone(),
                                                source_file: source_file.clone(),
                                                element_index,
                                                loop_index: loop_idx as i32,
                                                loop_start_time: cmd_loop.loop_start_time,
                                                loop_count: cmd_loop.total_iterations as i32,
                                                is_embedded: false,
                                            })?;
                                        }
                                        for (trigger_idx, trigger) in s.triggers.iter().enumerate() {
                                            writers.storyboard_triggers.write(StoryboardTriggerRow {
                                                folder_id: folder_id.clone(),
                                                source_file: source_file.clone(),
                                                element_index,
                                                trigger_index: trigger_idx as i32,
                                                trigger_name: trigger.name.clone(),
                                                trigger_start_time: trigger.start_time,
                                                trigger_end_time: trigger.end_time,
                                                group_number: trigger.group_num,
                                                is_embedded: false,
                                            })?;
                                        }
                                    }
                                    ElementKind::Animation(a) => {
                                        for (loop_idx, cmd_loop) in a.sprite.loops.iter().enumerate() {
                                            writers.storyboard_loops.write(StoryboardLoopRow {
                                                folder_id: folder_id.clone(),
                                                source_file: source_file.clone(),
                                                element_index,
                                                loop_index: loop_idx as i32,
                                                loop_start_time: cmd_loop.loop_start_time,
                                                loop_count: cmd_loop.total_iterations as i32,
                                                is_embedded: false,
                                            })?;
                                        }
                                        for (trigger_idx, trigger) in a.sprite.triggers.iter().enumerate() {
                                            writers.storyboard_triggers.write(StoryboardTriggerRow {
                                                folder_id: folder_id.clone(),
                                                source_file: source_file.clone(),
                                                element_index,
                                                trigger_index: trigger_idx as i32,
                                                trigger_name: trigger.name.clone(),
                                                trigger_start_time: trigger.start_time,
                                                trigger_end_time: trigger.end_time,
                                                group_number: trigger.group_num,
                                                is_embedded: false,
                                            })?;
                                        }
                                    }
                                    _ => {}
                                }

                                element_index += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    // Copy assets
    fs::create_dir_all(&assets_folder)?;
    for asset in &assets {
        let source_path = source_folder.join(asset);
        let dest_path = assets_folder.join(asset);
        
        if source_path.exists() {
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&source_path, &dest_path)?;
        }
    }

    Ok(())
}


fn extract_hit_object_info(
    ho: &rosu_map::section::hit_objects::HitObject,
) -> (String, Option<i32>, Option<i32>, bool, Option<String>, Option<i32>, Option<f64>, Option<f64>) {
    use rosu_map::section::hit_objects::HitObjectKind;

    match &ho.kind {
        HitObjectKind::Circle(c) => (
            "circle".to_string(),
            Some(c.pos.x as i32),
            Some(c.pos.y as i32),
            c.new_combo,
            None, None, None, None,
        ),
        HitObjectKind::Slider(s) => (
            "slider".to_string(),
            Some(s.pos.x as i32),
            Some(s.pos.y as i32),
            s.new_combo,
            None,  // curve_type not directly accessible
            Some(s.repeat_count),
            s.path.expected_dist().or(Some(0.0)),
            None,
        ),
        HitObjectKind::Spinner(sp) => (
            "spinner".to_string(),
            Some(sp.pos.x as i32),
            Some(sp.pos.y as i32),
            sp.new_combo,
            None, None, None,
            Some(sp.duration),
        ),
        HitObjectKind::Hold(h) => (
            "hold".to_string(),
            Some(h.pos_x as i32),
            None,  // Hold only has pos_x, no y
            false, // Hold has no new_combo
            None, None, None,
            Some(h.duration),
        ),
    }
}

fn extract_combo_offset(ho: &rosu_map::section::hit_objects::HitObject) -> i32 {
    use rosu_map::section::hit_objects::HitObjectKind;
    
    match &ho.kind {
        HitObjectKind::Circle(c) => c.combo_offset as i32,
        HitObjectKind::Slider(s) => s.combo_offset as i32,
        HitObjectKind::Spinner(_) => 0,  // Spinners don't have combo offset
        HitObjectKind::Hold(_) => 0,  // Hold notes don't have combo offset
    }
}
