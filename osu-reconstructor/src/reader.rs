//! Parquet reader utilities for loading dataset
//! 
//! This module uses Arrow's filter capabilities to only keep rows that match
//! the specified folder_id, significantly reducing memory usage.

use anyhow::{Context, Result};
use arrow::array::{
    Array, AsArray, BooleanArray, Float32Array, Float64Array, Int32Array, RecordBatch, StringArray,
};
use arrow::compute::kernels::cmp::eq;
use arrow::compute::filter_record_batch;
use arrow::datatypes::DataType;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use std::fs::File;
use std::path::Path;

use crate::types::*;

/// Reader for loading parquet files into Dataset
pub struct ParquetReader {
    dataset_path: std::path::PathBuf,
}

impl ParquetReader {
    /// Create a new reader for the given dataset directory
    pub fn new<P: AsRef<Path>>(dataset_path: P) -> Self {
        Self {
            dataset_path: dataset_path.as_ref().to_path_buf(),
        }
    }

    /// Load just the unique folder IDs from beatmaps.parquet
    /// 
    /// This is memory-efficient as it reads in batches
    pub fn load_folder_ids(&self) -> Result<Vec<String>> {
        let path = self.dataset_path.join("beatmaps.parquet");
        let file = File::open(&path).context(format!("Failed to open {}", path.display()))?;
        let builder = ParquetRecordBatchReaderBuilder::try_new(file)?;
        let reader = builder.with_batch_size(8192).build()?;
        
        let mut ids = std::collections::HashSet::new();
        for batch_result in reader {
            let batch = batch_result?;
            if let Some(col) = batch.column_by_name("folder_id") {
                if let Some(arr) = col.as_any().downcast_ref::<StringArray>() {
                    for i in 0..arr.len() {
                        if !arr.is_null(i) {
                            ids.insert(arr.value(i).to_string());
                        }
                    }
                }
            }
        }
        
        let mut sorted: Vec<String> = ids.into_iter().collect();
        sorted.sort();
        Ok(sorted)
    }

    /// Load dataset for a specific folder only using row-level filtering
    /// 
    /// This only loads rows that match the folder_id, using Arrow's filter
    /// capabilities to minimize memory usage.
    pub fn load_dataset_for_folder(&self, folder_id: &str) -> Result<Dataset> {
        let mut dataset = Dataset::default();
        
        dataset.beatmaps = self.load_beatmaps_filtered(folder_id)?;
        dataset.hit_objects = self.load_hit_objects_filtered(folder_id)?;
        dataset.timing_points = self.load_timing_points_filtered(folder_id)?;
        dataset.storyboard_elements = self.load_storyboard_elements_filtered(folder_id)?;
        dataset.storyboard_commands = self.load_storyboard_commands_filtered(folder_id)?;
        dataset.slider_control_points = self.load_slider_control_points_filtered(folder_id)?;
        dataset.slider_data = self.load_slider_data_filtered(folder_id)?;
        dataset.breaks = self.load_breaks_filtered(folder_id)?;
        dataset.combo_colors = self.load_combo_colors_filtered(folder_id)?;
        dataset.hit_samples = self.load_hit_samples_filtered(folder_id)?;
        dataset.storyboard_loops = self.load_storyboard_loops_filtered(folder_id)?;
        dataset.storyboard_triggers = self.load_storyboard_triggers_filtered(folder_id)?;
        
        Ok(dataset)
    }

    // ============ Filtered loading methods ============

    fn load_beatmaps_filtered(&self, target_folder: &str) -> Result<Vec<BeatmapRow>> {
        let path = self.dataset_path.join("beatmaps.parquet");
        let mut rows = Vec::new();
        
        for batch in read_filtered_batches(&path, "folder_id", target_folder)? {
            let folder_id = get_string_array(&batch, "folder_id")?;
            let osu_file = get_string_array(&batch, "osu_file")?;
            let format_version = get_i32_array(&batch, "format_version")?;
            let audio_file = get_string_array(&batch, "audio_file")?;
            let audio_lead_in = get_f64_array(&batch, "audio_lead_in")?;
            let preview_time = get_i32_array(&batch, "preview_time")?;
            let default_sample_bank = get_i32_array(&batch, "default_sample_bank")?;
            let default_sample_volume = get_i32_array(&batch, "default_sample_volume")?;
            let stack_leniency = get_f32_array(&batch, "stack_leniency")?;
            let mode = get_i32_array(&batch, "mode")?;
            let letterbox_in_breaks = get_bool_array(&batch, "letterbox_in_breaks")?;
            let special_style = get_bool_array(&batch, "special_style")?;
            let widescreen_storyboard = get_bool_array(&batch, "widescreen_storyboard")?;
            let epilepsy_warning = get_bool_array(&batch, "epilepsy_warning")?;
            let samples_match_playback_rate = get_bool_array(&batch, "samples_match_playback_rate")?;
            let countdown = get_i32_array(&batch, "countdown")?;
            let countdown_offset = get_i32_array(&batch, "countdown_offset")?;
            let bookmarks = get_string_array(&batch, "bookmarks")?;
            let distance_spacing = get_f64_array(&batch, "distance_spacing")?;
            let beat_divisor = get_i32_array(&batch, "beat_divisor")?;
            let grid_size = get_i32_array(&batch, "grid_size")?;
            let timeline_zoom = get_f64_array(&batch, "timeline_zoom")?;
            let title = get_string_array(&batch, "title")?;
            let title_unicode = get_string_array(&batch, "title_unicode")?;
            let artist = get_string_array(&batch, "artist")?;
            let artist_unicode = get_string_array(&batch, "artist_unicode")?;
            let creator = get_string_array(&batch, "creator")?;
            let version = get_string_array(&batch, "version")?;
            let source = get_string_array(&batch, "source")?;
            let tags = get_string_array(&batch, "tags")?;
            let beatmap_id = get_i32_array(&batch, "beatmap_id")?;
            let beatmap_set_id = get_i32_array(&batch, "beatmap_set_id")?;
            let hp_drain_rate = get_f32_array(&batch, "hp_drain_rate")?;
            let circle_size = get_f32_array(&batch, "circle_size")?;
            let overall_difficulty = get_f32_array(&batch, "overall_difficulty")?;
            let approach_rate = get_f32_array(&batch, "approach_rate")?;
            let slider_multiplier = get_f64_array(&batch, "slider_multiplier")?;
            let slider_tick_rate = get_f64_array(&batch, "slider_tick_rate")?;
            let background_file = get_string_array(&batch, "background_file")?;
            let audio_path = get_string_array(&batch, "audio_path")?;
            let background_path = get_string_array(&batch, "background_path")?;
            
            for i in 0..batch.num_rows() {
                rows.push(BeatmapRow {
                    folder_id: folder_id.value(i).to_string(),
                    osu_file: osu_file.value(i).to_string(),
                    format_version: format_version.value(i),
                    audio_file: audio_file.value(i).to_string(),
                    audio_lead_in: audio_lead_in.value(i),
                    preview_time: preview_time.value(i),
                    default_sample_bank: default_sample_bank.value(i),
                    default_sample_volume: default_sample_volume.value(i),
                    stack_leniency: stack_leniency.value(i),
                    mode: mode.value(i),
                    letterbox_in_breaks: letterbox_in_breaks.value(i),
                    special_style: special_style.value(i),
                    widescreen_storyboard: widescreen_storyboard.value(i),
                    epilepsy_warning: epilepsy_warning.value(i),
                    samples_match_playback_rate: samples_match_playback_rate.value(i),
                    countdown: countdown.value(i),
                    countdown_offset: countdown_offset.value(i),
                    bookmarks: bookmarks.value(i).to_string(),
                    distance_spacing: distance_spacing.value(i),
                    beat_divisor: beat_divisor.value(i),
                    grid_size: grid_size.value(i),
                    timeline_zoom: timeline_zoom.value(i),
                    title: title.value(i).to_string(),
                    title_unicode: title_unicode.value(i).to_string(),
                    artist: artist.value(i).to_string(),
                    artist_unicode: artist_unicode.value(i).to_string(),
                    creator: creator.value(i).to_string(),
                    version: version.value(i).to_string(),
                    source: source.value(i).to_string(),
                    tags: tags.value(i).to_string(),
                    beatmap_id: beatmap_id.value(i),
                    beatmap_set_id: beatmap_set_id.value(i),
                    hp_drain_rate: hp_drain_rate.value(i),
                    circle_size: circle_size.value(i),
                    overall_difficulty: overall_difficulty.value(i),
                    approach_rate: approach_rate.value(i),
                    slider_multiplier: slider_multiplier.value(i),
                    slider_tick_rate: slider_tick_rate.value(i),
                    background_file: background_file.value(i).to_string(),
                    audio_path: audio_path.value(i).to_string(),
                    background_path: background_path.value(i).to_string(),
                });
            }
        }
        Ok(rows)
    }

    fn load_hit_objects_filtered(&self, target_folder: &str) -> Result<Vec<HitObjectRow>> {
        let path = self.dataset_path.join("hit_objects.parquet");
        let mut rows = Vec::new();
        
        for batch in read_filtered_batches(&path, "folder_id", target_folder)? {
            let folder_id = get_string_array(&batch, "folder_id")?;
            let osu_file = get_string_array(&batch, "osu_file")?;
            let index = get_i32_array(&batch, "index")?;
            let start_time = get_f64_array(&batch, "start_time")?;
            let object_type = get_string_array(&batch, "object_type")?;
            let pos_x = get_nullable_i32_array(&batch, "pos_x")?;
            let pos_y = get_nullable_i32_array(&batch, "pos_y")?;
            let new_combo = get_bool_array(&batch, "new_combo")?;
            let combo_offset = get_i32_array(&batch, "combo_offset")?;
            let curve_type = get_nullable_string_array(&batch, "curve_type")?;
            let slides = get_nullable_i32_array(&batch, "slides")?;
            let length = get_nullable_f64_array(&batch, "length")?;
            let end_time = get_nullable_f64_array(&batch, "end_time")?;
            
            for i in 0..batch.num_rows() {
                rows.push(HitObjectRow {
                    folder_id: folder_id.value(i).to_string(),
                    osu_file: osu_file.value(i).to_string(),
                    index: index.value(i),
                    start_time: start_time.value(i),
                    object_type: object_type.value(i).to_string(),
                    pos_x: pos_x.get(i),
                    pos_y: pos_y.get(i),
                    new_combo: new_combo.value(i),
                    combo_offset: combo_offset.value(i),
                    curve_type: curve_type.get(i),
                    slides: slides.get(i),
                    length: length.get(i),
                    end_time: end_time.get(i),
                });
            }
        }
        Ok(rows)
    }

    fn load_timing_points_filtered(&self, target_folder: &str) -> Result<Vec<TimingPointRow>> {
        let path = self.dataset_path.join("timing_points.parquet");
        let mut rows = Vec::new();
        
        for batch in read_filtered_batches(&path, "folder_id", target_folder)? {
            let folder_id = get_string_array(&batch, "folder_id")?;
            let osu_file = get_string_array(&batch, "osu_file")?;
            let time = get_f64_array(&batch, "time")?;
            let point_type = get_string_array(&batch, "point_type")?;
            let beat_length = get_nullable_f64_array(&batch, "beat_length")?;
            let time_signature = get_nullable_string_array(&batch, "time_signature")?;
            let slider_velocity = get_nullable_f64_array(&batch, "slider_velocity")?;
            let kiai = get_nullable_bool_array(&batch, "kiai")?;
            let sample_bank = get_nullable_string_array(&batch, "sample_bank")?;
            let sample_volume = get_nullable_i32_array(&batch, "sample_volume")?;
            
            for i in 0..batch.num_rows() {
                rows.push(TimingPointRow {
                    folder_id: folder_id.value(i).to_string(),
                    osu_file: osu_file.value(i).to_string(),
                    time: time.value(i),
                    point_type: point_type.value(i).to_string(),
                    beat_length: beat_length.get(i),
                    time_signature: time_signature.get(i),
                    slider_velocity: slider_velocity.get(i),
                    kiai: kiai.get(i),
                    sample_bank: sample_bank.get(i),
                    sample_volume: sample_volume.get(i),
                });
            }
        }
        Ok(rows)
    }

    fn load_storyboard_elements_filtered(&self, target_folder: &str) -> Result<Vec<StoryboardElementRow>> {
        let path = self.dataset_path.join("storyboard_elements.parquet");
        let mut rows = Vec::new();
        
        for batch in read_filtered_batches(&path, "folder_id", target_folder)? {
            let folder_id = get_string_array(&batch, "folder_id")?;
            let source_file = get_string_array(&batch, "source_file")?;
            let element_index = get_i32_array(&batch, "element_index")?;
            let layer_name = get_string_array(&batch, "layer_name")?;
            let element_path = get_string_array(&batch, "element_path")?;
            let element_type = get_string_array(&batch, "element_type")?;
            let origin = get_string_array(&batch, "origin")?;
            let initial_pos_x = get_f32_array(&batch, "initial_pos_x")?;
            let initial_pos_y = get_f32_array(&batch, "initial_pos_y")?;
            let frame_count = get_nullable_i32_array(&batch, "frame_count")?;
            let frame_delay = get_nullable_f64_array(&batch, "frame_delay")?;
            let loop_type = get_nullable_string_array(&batch, "loop_type")?;
            let is_embedded = get_bool_array(&batch, "is_embedded")?;
            
            for i in 0..batch.num_rows() {
                rows.push(StoryboardElementRow {
                    folder_id: folder_id.value(i).to_string(),
                    source_file: source_file.value(i).to_string(),
                    element_index: element_index.value(i),
                    layer_name: layer_name.value(i).to_string(),
                    element_path: element_path.value(i).to_string(),
                    element_type: element_type.value(i).to_string(),
                    origin: origin.value(i).to_string(),
                    initial_pos_x: initial_pos_x.value(i),
                    initial_pos_y: initial_pos_y.value(i),
                    frame_count: frame_count.get(i),
                    frame_delay: frame_delay.get(i),
                    loop_type: loop_type.get(i),
                    is_embedded: is_embedded.value(i),
                });
            }
        }
        Ok(rows)
    }

    fn load_storyboard_commands_filtered(&self, target_folder: &str) -> Result<Vec<StoryboardCommandRow>> {
        let path = self.dataset_path.join("storyboard_commands.parquet");
        let mut rows = Vec::new();
        
        for batch in read_filtered_batches(&path, "folder_id", target_folder)? {
            let folder_id = get_string_array(&batch, "folder_id")?;
            let source_file = get_string_array(&batch, "source_file")?;
            let element_index = get_i32_array(&batch, "element_index")?;
            let command_type = get_string_array(&batch, "command_type")?;
            let start_time = get_f64_array(&batch, "start_time")?;
            let end_time = get_f64_array(&batch, "end_time")?;
            let start_value = get_string_array(&batch, "start_value")?;
            let end_value = get_string_array(&batch, "end_value")?;
            let easing = get_i32_array(&batch, "easing")?;
            let is_embedded = get_bool_array(&batch, "is_embedded")?;
            
            for i in 0..batch.num_rows() {
                rows.push(StoryboardCommandRow {
                    folder_id: folder_id.value(i).to_string(),
                    source_file: source_file.value(i).to_string(),
                    element_index: element_index.value(i),
                    command_type: command_type.value(i).to_string(),
                    start_time: start_time.value(i),
                    end_time: end_time.value(i),
                    start_value: start_value.value(i).to_string(),
                    end_value: end_value.value(i).to_string(),
                    easing: easing.value(i),
                    is_embedded: is_embedded.value(i),
                });
            }
        }
        Ok(rows)
    }

    fn load_slider_control_points_filtered(&self, target_folder: &str) -> Result<Vec<SliderControlPointRow>> {
        let path = self.dataset_path.join("slider_control_points.parquet");
        let mut rows = Vec::new();
        
        for batch in read_filtered_batches(&path, "folder_id", target_folder)? {
            let folder_id = get_string_array(&batch, "folder_id")?;
            let osu_file = get_string_array(&batch, "osu_file")?;
            let hit_object_index = get_i32_array(&batch, "hit_object_index")?;
            let point_index = get_i32_array(&batch, "point_index")?;
            let pos_x = get_f32_array(&batch, "pos_x")?;
            let pos_y = get_f32_array(&batch, "pos_y")?;
            let path_type = get_nullable_string_array(&batch, "path_type")?;
            
            for i in 0..batch.num_rows() {
                rows.push(SliderControlPointRow {
                    folder_id: folder_id.value(i).to_string(),
                    osu_file: osu_file.value(i).to_string(),
                    hit_object_index: hit_object_index.value(i),
                    point_index: point_index.value(i),
                    pos_x: pos_x.value(i),
                    pos_y: pos_y.value(i),
                    path_type: path_type.get(i),
                });
            }
        }
        Ok(rows)
    }

    fn load_slider_data_filtered(&self, target_folder: &str) -> Result<Vec<SliderDataRow>> {
        let path = self.dataset_path.join("slider_data.parquet");
        let mut rows = Vec::new();
        
        for batch in read_filtered_batches(&path, "folder_id", target_folder)? {
            let folder_id = get_string_array(&batch, "folder_id")?;
            let osu_file = get_string_array(&batch, "osu_file")?;
            let hit_object_index = get_i32_array(&batch, "hit_object_index")?;
            let repeat_count = get_i32_array(&batch, "repeat_count")?;
            let velocity = get_f64_array(&batch, "velocity")?;
            let expected_dist = get_nullable_f64_array(&batch, "expected_dist")?;
            
            for i in 0..batch.num_rows() {
                rows.push(SliderDataRow {
                    folder_id: folder_id.value(i).to_string(),
                    osu_file: osu_file.value(i).to_string(),
                    hit_object_index: hit_object_index.value(i),
                    repeat_count: repeat_count.value(i),
                    velocity: velocity.value(i),
                    expected_dist: expected_dist.get(i),
                });
            }
        }
        Ok(rows)
    }

    fn load_breaks_filtered(&self, target_folder: &str) -> Result<Vec<BreakRow>> {
        let path = self.dataset_path.join("breaks.parquet");
        let mut rows = Vec::new();
        
        for batch in read_filtered_batches(&path, "folder_id", target_folder)? {
            let folder_id = get_string_array(&batch, "folder_id")?;
            let osu_file = get_string_array(&batch, "osu_file")?;
            let start_time = get_f64_array(&batch, "start_time")?;
            let end_time = get_f64_array(&batch, "end_time")?;
            
            for i in 0..batch.num_rows() {
                rows.push(BreakRow {
                    folder_id: folder_id.value(i).to_string(),
                    osu_file: osu_file.value(i).to_string(),
                    start_time: start_time.value(i),
                    end_time: end_time.value(i),
                });
            }
        }
        Ok(rows)
    }

    fn load_combo_colors_filtered(&self, target_folder: &str) -> Result<Vec<ComboColorRow>> {
        let path = self.dataset_path.join("combo_colors.parquet");
        let mut rows = Vec::new();
        
        for batch in read_filtered_batches(&path, "folder_id", target_folder)? {
            let folder_id = get_string_array(&batch, "folder_id")?;
            let osu_file = get_string_array(&batch, "osu_file")?;
            let color_index = get_i32_array(&batch, "color_index")?;
            let color_type = get_string_array(&batch, "color_type")?;
            let custom_name = get_nullable_string_array(&batch, "custom_name")?;
            let red = get_i32_array(&batch, "red")?;
            let green = get_i32_array(&batch, "green")?;
            let blue = get_i32_array(&batch, "blue")?;
            
            for i in 0..batch.num_rows() {
                rows.push(ComboColorRow {
                    folder_id: folder_id.value(i).to_string(),
                    osu_file: osu_file.value(i).to_string(),
                    color_index: color_index.value(i),
                    color_type: color_type.value(i).to_string(),
                    custom_name: custom_name.get(i),
                    red: red.value(i),
                    green: green.value(i),
                    blue: blue.value(i),
                });
            }
        }
        Ok(rows)
    }

    fn load_hit_samples_filtered(&self, target_folder: &str) -> Result<Vec<HitSampleRow>> {
        let path = self.dataset_path.join("hit_samples.parquet");
        let mut rows = Vec::new();
        
        for batch in read_filtered_batches(&path, "folder_id", target_folder)? {
            let folder_id = get_string_array(&batch, "folder_id")?;
            let osu_file = get_string_array(&batch, "osu_file")?;
            let hit_object_index = get_i32_array(&batch, "hit_object_index")?;
            let sample_index = get_i32_array(&batch, "sample_index")?;
            let name = get_string_array(&batch, "name")?;
            let bank = get_string_array(&batch, "bank")?;
            let suffix = get_nullable_string_array(&batch, "suffix")?;
            let volume = get_i32_array(&batch, "volume")?;
            
            for i in 0..batch.num_rows() {
                rows.push(HitSampleRow {
                    folder_id: folder_id.value(i).to_string(),
                    osu_file: osu_file.value(i).to_string(),
                    hit_object_index: hit_object_index.value(i),
                    sample_index: sample_index.value(i),
                    name: name.value(i).to_string(),
                    bank: bank.value(i).to_string(),
                    suffix: suffix.get(i),
                    volume: volume.value(i),
                });
            }
        }
        Ok(rows)
    }

    fn load_storyboard_loops_filtered(&self, target_folder: &str) -> Result<Vec<StoryboardLoopRow>> {
        let path = self.dataset_path.join("storyboard_loops.parquet");
        let mut rows = Vec::new();
        
        for batch in read_filtered_batches(&path, "folder_id", target_folder)? {
            let folder_id = get_string_array(&batch, "folder_id")?;
            let source_file = get_string_array(&batch, "source_file")?;
            let element_index = get_i32_array(&batch, "element_index")?;
            let loop_index = get_i32_array(&batch, "loop_index")?;
            let loop_start_time = get_f64_array(&batch, "loop_start_time")?;
            let loop_count = get_i32_array(&batch, "loop_count")?;
            let is_embedded = get_bool_array(&batch, "is_embedded")?;
            
            for i in 0..batch.num_rows() {
                rows.push(StoryboardLoopRow {
                    folder_id: folder_id.value(i).to_string(),
                    source_file: source_file.value(i).to_string(),
                    element_index: element_index.value(i),
                    loop_index: loop_index.value(i),
                    loop_start_time: loop_start_time.value(i),
                    loop_count: loop_count.value(i),
                    is_embedded: is_embedded.value(i),
                });
            }
        }
        Ok(rows)
    }

    fn load_storyboard_triggers_filtered(&self, target_folder: &str) -> Result<Vec<StoryboardTriggerRow>> {
        let path = self.dataset_path.join("storyboard_triggers.parquet");
        let mut rows = Vec::new();
        
        for batch in read_filtered_batches(&path, "folder_id", target_folder)? {
            let folder_id = get_string_array(&batch, "folder_id")?;
            let source_file = get_string_array(&batch, "source_file")?;
            let element_index = get_i32_array(&batch, "element_index")?;
            let trigger_index = get_i32_array(&batch, "trigger_index")?;
            let trigger_name = get_string_array(&batch, "trigger_name")?;
            let trigger_start_time = get_f64_array(&batch, "trigger_start_time")?;
            let trigger_end_time = get_f64_array(&batch, "trigger_end_time")?;
            let group_number = get_i32_array(&batch, "group_number")?;
            let is_embedded = get_bool_array(&batch, "is_embedded")?;
            
            for i in 0..batch.num_rows() {
                rows.push(StoryboardTriggerRow {
                    folder_id: folder_id.value(i).to_string(),
                    source_file: source_file.value(i).to_string(),
                    element_index: element_index.value(i),
                    trigger_index: trigger_index.value(i),
                    trigger_name: trigger_name.value(i).to_string(),
                    trigger_start_time: trigger_start_time.value(i),
                    trigger_end_time: trigger_end_time.value(i),
                    group_number: group_number.value(i),
                    is_embedded: is_embedded.value(i),
                });
            }
        }
        Ok(rows)
    }
}

// ============ Helper functions with filtering ============

/// Read parquet file with row-level filtering using Arrow compute
/// 
/// This reads the file in batches and filters each batch to only include
/// rows where the filter_column equals filter_value. This significantly
/// reduces memory usage compared to loading all rows.
fn read_filtered_batches(
    path: &Path,
    filter_column: &str,
    filter_value: &str,
) -> Result<Vec<RecordBatch>> {
    let file = File::open(path).context(format!("Failed to open {}", path.display()))?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file)?;
    
    // Use smaller batch size to reduce peak memory
    let reader = builder.with_batch_size(8192).build()?;
    
    let mut filtered_batches = Vec::new();
    
    for batch_result in reader {
        let batch = batch_result.context("Failed to read batch")?;
        
        // Get the filter column
        let col = batch
            .column_by_name(filter_column)
            .context(format!("Missing column: {}", filter_column))?;
        
        // Create filter mask: true where column == filter_value
        let filter_mask = create_string_eq_filter(col.as_ref(), filter_value)?;
        
        // Apply filter - only keep rows where filter_mask is true
        let filtered = filter_record_batch(&batch, &filter_mask)?;
        
        // Only add non-empty batches
        if filtered.num_rows() > 0 {
            filtered_batches.push(filtered);
        }
    }
    
    Ok(filtered_batches)
}

/// Create a boolean filter mask for string equality comparison
fn create_string_eq_filter(array: &dyn Array, value: &str) -> Result<BooleanArray> {
    match array.data_type() {
        DataType::Utf8 => {
            let arr = array.as_string::<i32>();
            // Create a scalar array filled with the comparison value
            let scalar = StringArray::from(vec![value; arr.len()]);
            Ok(eq(arr, &scalar)?)
        }
        DataType::LargeUtf8 => {
            let arr = array.as_string::<i64>();
            let scalar = arrow::array::LargeStringArray::from(vec![value; arr.len()]);
            Ok(eq(arr, &scalar)?)
        }
        _ => {
            anyhow::bail!("Unsupported column type for filtering: {:?}", array.data_type());
        }
    }
}

fn get_string_array<'a>(batch: &'a RecordBatch, name: &str) -> Result<&'a StringArray> {
    batch
        .column_by_name(name)
        .context(format!("Missing column: {}", name))?
        .as_any()
        .downcast_ref::<StringArray>()
        .context(format!("Column {} is not StringArray", name))
}

fn get_i32_array<'a>(batch: &'a RecordBatch, name: &str) -> Result<&'a Int32Array> {
    batch
        .column_by_name(name)
        .context(format!("Missing column: {}", name))?
        .as_any()
        .downcast_ref::<Int32Array>()
        .context(format!("Column {} is not Int32Array", name))
}

fn get_f32_array<'a>(batch: &'a RecordBatch, name: &str) -> Result<&'a Float32Array> {
    batch
        .column_by_name(name)
        .context(format!("Missing column: {}", name))?
        .as_any()
        .downcast_ref::<Float32Array>()
        .context(format!("Column {} is not Float32Array", name))
}

fn get_f64_array<'a>(batch: &'a RecordBatch, name: &str) -> Result<&'a Float64Array> {
    batch
        .column_by_name(name)
        .context(format!("Missing column: {}", name))?
        .as_any()
        .downcast_ref::<Float64Array>()
        .context(format!("Column {} is not Float64Array", name))
}

fn get_bool_array<'a>(batch: &'a RecordBatch, name: &str) -> Result<&'a BooleanArray> {
    batch
        .column_by_name(name)
        .context(format!("Missing column: {}", name))?
        .as_any()
        .downcast_ref::<BooleanArray>()
        .context(format!("Column {} is not BooleanArray", name))
}

/// Wrapper for nullable i32 values
struct NullableI32Array<'a>(&'a Int32Array);
impl<'a> NullableI32Array<'a> {
    fn get(&self, i: usize) -> Option<i32> {
        if self.0.is_null(i) { None } else { Some(self.0.value(i)) }
    }
}

fn get_nullable_i32_array<'a>(batch: &'a RecordBatch, name: &str) -> Result<NullableI32Array<'a>> {
    Ok(NullableI32Array(get_i32_array(batch, name)?))
}

/// Wrapper for nullable f64 values
struct NullableF64Array<'a>(&'a Float64Array);
impl<'a> NullableF64Array<'a> {
    fn get(&self, i: usize) -> Option<f64> {
        if self.0.is_null(i) { None } else { Some(self.0.value(i)) }
    }
}

fn get_nullable_f64_array<'a>(batch: &'a RecordBatch, name: &str) -> Result<NullableF64Array<'a>> {
    Ok(NullableF64Array(get_f64_array(batch, name)?))
}

/// Wrapper for nullable string values
struct NullableStringArray<'a>(&'a StringArray);
impl<'a> NullableStringArray<'a> {
    fn get(&self, i: usize) -> Option<String> {
        if self.0.is_null(i) { None } else { Some(self.0.value(i).to_string()) }
    }
}

fn get_nullable_string_array<'a>(batch: &'a RecordBatch, name: &str) -> Result<NullableStringArray<'a>> {
    Ok(NullableStringArray(get_string_array(batch, name)?))
}

/// Wrapper for nullable bool values
struct NullableBoolArray<'a>(&'a BooleanArray);
impl<'a> NullableBoolArray<'a> {
    fn get(&self, i: usize) -> Option<bool> {
        if self.0.is_null(i) { None } else { Some(self.0.value(i)) }
    }
}

fn get_nullable_bool_array<'a>(batch: &'a RecordBatch, name: &str) -> Result<NullableBoolArray<'a>> {
    Ok(NullableBoolArray(get_bool_array(batch, name)?))
}
