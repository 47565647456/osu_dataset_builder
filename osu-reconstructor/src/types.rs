//! Core types for representing parquet row data

/// Beatmap metadata row from beatmaps.parquet
#[derive(Debug, Clone)]
pub struct BeatmapRow {
    pub folder_id: String,
    pub osu_file: String,
    pub format_version: i32,
    pub audio_file: String,
    pub audio_lead_in: f64,
    pub preview_time: i32,
    // General section
    pub default_sample_bank: i32,
    pub default_sample_volume: i32,
    pub stack_leniency: f32,
    pub mode: i32,
    pub letterbox_in_breaks: bool,
    pub special_style: bool,
    pub widescreen_storyboard: bool,
    pub epilepsy_warning: bool,
    pub samples_match_playback_rate: bool,
    pub countdown: i32,
    pub countdown_offset: i32,
    // Editor section
    pub bookmarks: String,
    pub distance_spacing: f64,
    pub beat_divisor: i32,
    pub grid_size: i32,
    pub timeline_zoom: f64,
    // Metadata section
    pub title: String,
    pub title_unicode: String,
    pub artist: String,
    pub artist_unicode: String,
    pub creator: String,
    pub version: String,
    pub source: String,
    pub tags: String,
    pub beatmap_id: i32,
    pub beatmap_set_id: i32,
    // Difficulty section
    pub hp_drain_rate: f32,
    pub circle_size: f32,
    pub overall_difficulty: f32,
    pub approach_rate: f32,
    pub slider_multiplier: f64,
    pub slider_tick_rate: f64,
    // Events section
    pub background_file: String,
    pub audio_path: String,
    pub background_path: String,
}

/// Hit object row from hit_objects.parquet
#[derive(Debug, Clone)]
pub struct HitObjectRow {
    pub folder_id: String,
    pub osu_file: String,
    pub index: i32,
    pub start_time: f64,
    pub object_type: String,
    pub pos_x: Option<i32>,
    pub pos_y: Option<i32>,
    pub new_combo: bool,
    pub combo_offset: i32,
    pub curve_type: Option<String>,
    pub slides: Option<i32>,
    pub length: Option<f64>,
    pub end_time: Option<f64>,
}

/// Timing point row from timing_points.parquet
#[derive(Debug, Clone)]
pub struct TimingPointRow {
    pub folder_id: String,
    pub osu_file: String,
    pub time: f64,
    pub point_type: String,
    pub beat_length: Option<f64>,
    pub time_signature: Option<String>,
    pub slider_velocity: Option<f64>,
    pub kiai: Option<bool>,
    pub sample_bank: Option<String>,
    pub sample_volume: Option<i32>,
}

/// Storyboard element row from storyboard_elements.parquet
#[derive(Debug, Clone)]
pub struct StoryboardElementRow {
    pub folder_id: String,
    pub source_file: String,
    pub element_index: i32,
    pub layer_name: String,
    pub element_path: String,
    pub element_type: String,
    pub origin: String,
    pub initial_pos_x: f32,
    pub initial_pos_y: f32,
    pub frame_count: Option<i32>,
    pub frame_delay: Option<f64>,
    pub loop_type: Option<String>,
    pub is_embedded: bool,
}

/// Storyboard command row from storyboard_commands.parquet
#[derive(Debug, Clone)]
pub struct StoryboardCommandRow {
    pub folder_id: String,
    pub source_file: String,
    pub element_index: i32,
    pub command_type: String,
    pub start_time: f64,
    pub end_time: f64,
    pub start_value: String,
    pub end_value: String,
    pub easing: i32,
    pub is_embedded: bool,
}

/// Slider control point row from slider_control_points.parquet
#[derive(Debug, Clone)]
pub struct SliderControlPointRow {
    pub folder_id: String,
    pub osu_file: String,
    pub hit_object_index: i32,
    pub point_index: i32,
    pub pos_x: f32,
    pub pos_y: f32,
    pub path_type: Option<String>,
}

/// Slider data row from slider_data.parquet
#[derive(Debug, Clone)]
pub struct SliderDataRow {
    pub folder_id: String,
    pub osu_file: String,
    pub hit_object_index: i32,
    pub repeat_count: i32,
    pub velocity: f64,
    pub expected_dist: Option<f64>,
}

/// Break period row from breaks.parquet
#[derive(Debug, Clone)]
pub struct BreakRow {
    pub folder_id: String,
    pub osu_file: String,
    pub start_time: f64,
    pub end_time: f64,
}

/// Combo color row from combo_colors.parquet
#[derive(Debug, Clone)]
pub struct ComboColorRow {
    pub folder_id: String,
    pub osu_file: String,
    pub color_index: i32,
    pub color_type: String,
    pub custom_name: Option<String>,
    pub red: i32,
    pub green: i32,
    pub blue: i32,
}

/// Hit sample row from hit_samples.parquet
#[derive(Debug, Clone)]
pub struct HitSampleRow {
    pub folder_id: String,
    pub osu_file: String,
    pub hit_object_index: i32,
    pub sample_index: i32,
    pub name: String,
    pub bank: String,
    pub suffix: Option<String>,
    pub volume: i32,
}

/// Storyboard loop row from storyboard_loops.parquet
#[derive(Debug, Clone)]
pub struct StoryboardLoopRow {
    pub folder_id: String,
    pub source_file: String,
    pub element_index: i32,
    pub loop_index: i32,
    pub loop_start_time: f64,
    pub loop_count: i32,
    pub is_embedded: bool,
}

/// Storyboard trigger row from storyboard_triggers.parquet
#[derive(Debug, Clone)]
pub struct StoryboardTriggerRow {
    pub folder_id: String,
    pub source_file: String,
    pub element_index: i32,
    pub trigger_index: i32,
    pub trigger_name: String,
    pub trigger_start_time: f64,
    pub trigger_end_time: f64,
    pub group_number: i32,
    pub is_embedded: bool,
}

/// Complete dataset loaded from parquet files
#[derive(Debug, Default)]
pub struct Dataset {
    pub beatmaps: Vec<BeatmapRow>,
    pub hit_objects: Vec<HitObjectRow>,
    pub timing_points: Vec<TimingPointRow>,
    pub storyboard_elements: Vec<StoryboardElementRow>,
    pub storyboard_commands: Vec<StoryboardCommandRow>,
    pub slider_control_points: Vec<SliderControlPointRow>,
    pub slider_data: Vec<SliderDataRow>,
    pub breaks: Vec<BreakRow>,
    pub combo_colors: Vec<ComboColorRow>,
    pub hit_samples: Vec<HitSampleRow>,
    pub storyboard_loops: Vec<StoryboardLoopRow>,
    pub storyboard_triggers: Vec<StoryboardTriggerRow>,
}
