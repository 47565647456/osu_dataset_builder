//! Batch-wise parquet writers for memory-efficient data export

use anyhow::Result;
use arrow::array::*;
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use parquet::arrow::ArrowWriter;
use parquet::file::properties::WriterProperties;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use crate::{
    BeatmapRow, HitObjectRow, TimingPointRow, StoryboardElementRow, 
    StoryboardCommandRow, SliderControlPointRow, SliderDataRow,
    BreakRow, ComboColorRow, HitSampleRow, StoryboardLoopRow, StoryboardTriggerRow,
};

const DEFAULT_BATCH_SIZE: usize = 100;

/// Generic batch writer for parquet files
pub struct BatchWriter<T, F: Fn(&[T]) -> Result<RecordBatch>> {
    writer: ArrowWriter<File>,
    buffer: Vec<T>,
    batch_size: usize,
    to_batch: F,
    total_rows: usize,
}

impl<T, F: Fn(&[T]) -> Result<RecordBatch>> BatchWriter<T, F> {
    pub fn new(path: &Path, schema: Arc<Schema>, to_batch: F) -> Result<Self> {
        Self::with_batch_size(path, schema, to_batch, DEFAULT_BATCH_SIZE)
    }

    pub fn with_batch_size(path: &Path, schema: Arc<Schema>, to_batch: F, batch_size: usize) -> Result<Self> {
        let file = File::create(path)?;
        let props = WriterProperties::builder()
            .set_compression(parquet::basic::Compression::SNAPPY)
            .build();
        let writer = ArrowWriter::try_new(file, schema, Some(props))?;
        
        Ok(Self {
            writer,
            buffer: Vec::with_capacity(batch_size),
            batch_size,
            to_batch,
            total_rows: 0,
        })
    }

    pub fn write(&mut self, row: T) -> Result<()> {
        self.buffer.push(row);
        if self.buffer.len() >= self.batch_size {
            self.flush()?;
        }
        Ok(())
    }

    pub fn flush(&mut self) -> Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let batch = (self.to_batch)(&self.buffer)?;
        self.total_rows += self.buffer.len();
        self.writer.write(&batch)?;
        self.buffer.clear();
        Ok(())
    }

    pub fn close(mut self) -> Result<usize> {
        self.flush()?;
        self.writer.close()?;
        Ok(self.total_rows)
    }
}

// ============ Schema Definitions ============

pub fn beatmap_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("folder_id", DataType::Utf8, false),
        Field::new("osu_file", DataType::Utf8, false),
        Field::new("format_version", DataType::Int32, false),
        Field::new("audio_file", DataType::Utf8, false),
        Field::new("audio_lead_in", DataType::Float64, false),
        Field::new("preview_time", DataType::Int32, false),
        // General section - new fields
        Field::new("default_sample_bank", DataType::Int32, false),
        Field::new("default_sample_volume", DataType::Int32, false),
        Field::new("stack_leniency", DataType::Float32, false),
        Field::new("mode", DataType::Int32, false),
        Field::new("letterbox_in_breaks", DataType::Boolean, false),
        Field::new("special_style", DataType::Boolean, false),
        Field::new("widescreen_storyboard", DataType::Boolean, false),
        Field::new("epilepsy_warning", DataType::Boolean, false),
        Field::new("samples_match_playback_rate", DataType::Boolean, false),
        Field::new("countdown", DataType::Int32, false),
        Field::new("countdown_offset", DataType::Int32, false),
        // Editor section
        Field::new("bookmarks", DataType::Utf8, false),
        Field::new("distance_spacing", DataType::Float64, false),
        Field::new("beat_divisor", DataType::Int32, false),
        Field::new("grid_size", DataType::Int32, false),
        Field::new("timeline_zoom", DataType::Float64, false),
        // Metadata section
        Field::new("title", DataType::Utf8, false),
        Field::new("title_unicode", DataType::Utf8, false),
        Field::new("artist", DataType::Utf8, false),
        Field::new("artist_unicode", DataType::Utf8, false),
        Field::new("creator", DataType::Utf8, false),
        Field::new("version", DataType::Utf8, false),
        Field::new("source", DataType::Utf8, false),
        Field::new("tags", DataType::Utf8, false),
        Field::new("beatmap_id", DataType::Int32, false),
        Field::new("beatmap_set_id", DataType::Int32, false),
        // Difficulty section
        Field::new("hp_drain_rate", DataType::Float32, false),
        Field::new("circle_size", DataType::Float32, false),
        Field::new("overall_difficulty", DataType::Float32, false),
        Field::new("approach_rate", DataType::Float32, false),
        Field::new("slider_multiplier", DataType::Float64, false),
        Field::new("slider_tick_rate", DataType::Float64, false),
        // Events section
        Field::new("background_file", DataType::Utf8, false),
        Field::new("audio_path", DataType::Utf8, false),
        Field::new("background_path", DataType::Utf8, false),
    ]))
}

pub fn hit_object_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("folder_id", DataType::Utf8, false),
        Field::new("osu_file", DataType::Utf8, false),
        Field::new("index", DataType::Int32, false),
        Field::new("start_time", DataType::Float64, false),
        Field::new("object_type", DataType::Utf8, false),
        Field::new("pos_x", DataType::Int32, true),
        Field::new("pos_y", DataType::Int32, true),
        Field::new("new_combo", DataType::Boolean, false),
        Field::new("combo_offset", DataType::Int32, false),
        Field::new("curve_type", DataType::Utf8, true),
        Field::new("slides", DataType::Int32, true),
        Field::new("length", DataType::Float64, true),
        Field::new("end_time", DataType::Float64, true),
    ]))
}

pub fn timing_point_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("folder_id", DataType::Utf8, false),
        Field::new("osu_file", DataType::Utf8, false),
        Field::new("time", DataType::Float64, false),
        Field::new("point_type", DataType::Utf8, false),
        Field::new("beat_length", DataType::Float64, true),
        Field::new("time_signature", DataType::Utf8, true),
        Field::new("slider_velocity", DataType::Float64, true),
        Field::new("kiai", DataType::Boolean, true),
        Field::new("sample_bank", DataType::Utf8, true),
        Field::new("sample_volume", DataType::Int32, true),
    ]))
}

pub fn storyboard_element_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("folder_id", DataType::Utf8, false),
        Field::new("source_file", DataType::Utf8, false),
        Field::new("element_index", DataType::Int32, false),
        Field::new("layer_name", DataType::Utf8, false),
        Field::new("element_path", DataType::Utf8, false),
        Field::new("element_type", DataType::Utf8, false),
        Field::new("origin", DataType::Utf8, false),
        Field::new("initial_pos_x", DataType::Float32, false),
        Field::new("initial_pos_y", DataType::Float32, false),
        Field::new("frame_count", DataType::Int32, true),
        Field::new("frame_delay", DataType::Float64, true),
        Field::new("loop_type", DataType::Utf8, true),
        Field::new("is_embedded", DataType::Boolean, false),
    ]))
}

pub fn storyboard_command_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("folder_id", DataType::Utf8, false),
        Field::new("source_file", DataType::Utf8, false),
        Field::new("element_index", DataType::Int32, false),
        Field::new("command_type", DataType::Utf8, false),
        Field::new("start_time", DataType::Float64, false),
        Field::new("end_time", DataType::Float64, false),
        Field::new("start_value", DataType::Utf8, false),
        Field::new("end_value", DataType::Utf8, false),
        Field::new("easing", DataType::Int32, false),
        Field::new("is_embedded", DataType::Boolean, false),
    ]))
}

pub fn slider_control_point_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("folder_id", DataType::Utf8, false),
        Field::new("osu_file", DataType::Utf8, false),
        Field::new("hit_object_index", DataType::Int32, false),
        Field::new("point_index", DataType::Int32, false),
        Field::new("pos_x", DataType::Float32, false),
        Field::new("pos_y", DataType::Float32, false),
        Field::new("path_type", DataType::Utf8, true),
    ]))
}

pub fn slider_data_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("folder_id", DataType::Utf8, false),
        Field::new("osu_file", DataType::Utf8, false),
        Field::new("hit_object_index", DataType::Int32, false),
        Field::new("repeat_count", DataType::Int32, false),
        Field::new("velocity", DataType::Float64, false),
        Field::new("expected_dist", DataType::Float64, true),
    ]))
}

pub fn break_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("folder_id", DataType::Utf8, false),
        Field::new("osu_file", DataType::Utf8, false),
        Field::new("start_time", DataType::Float64, false),
        Field::new("end_time", DataType::Float64, false),
    ]))
}

pub fn combo_color_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("folder_id", DataType::Utf8, false),
        Field::new("osu_file", DataType::Utf8, false),
        Field::new("color_index", DataType::Int32, false),
        Field::new("color_type", DataType::Utf8, false),
        Field::new("custom_name", DataType::Utf8, true),
        Field::new("red", DataType::Int32, false),
        Field::new("green", DataType::Int32, false),
        Field::new("blue", DataType::Int32, false),
    ]))
}

pub fn hit_sample_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("folder_id", DataType::Utf8, false),
        Field::new("osu_file", DataType::Utf8, false),
        Field::new("hit_object_index", DataType::Int32, false),
        Field::new("sample_index", DataType::Int32, false),
        Field::new("name", DataType::Utf8, false),
        Field::new("bank", DataType::Utf8, false),
        Field::new("suffix", DataType::Utf8, true),
        Field::new("volume", DataType::Int32, false),
    ]))
}

pub fn storyboard_loop_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("folder_id", DataType::Utf8, false),
        Field::new("source_file", DataType::Utf8, false),
        Field::new("element_index", DataType::Int32, false),
        Field::new("loop_index", DataType::Int32, false),
        Field::new("loop_start_time", DataType::Float64, false),
        Field::new("loop_count", DataType::Int32, false),
        Field::new("is_embedded", DataType::Boolean, false),
    ]))
}

pub fn storyboard_trigger_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("folder_id", DataType::Utf8, false),
        Field::new("source_file", DataType::Utf8, false),
        Field::new("element_index", DataType::Int32, false),
        Field::new("trigger_index", DataType::Int32, false),
        Field::new("trigger_name", DataType::Utf8, false),
        Field::new("trigger_start_time", DataType::Float64, false),
        Field::new("trigger_end_time", DataType::Float64, false),
        Field::new("group_number", DataType::Int32, false),
        Field::new("is_embedded", DataType::Boolean, false),
    ]))
}

// ============ Batch Conversion Functions ============

pub fn beatmap_rows_to_batch(rows: &[BeatmapRow]) -> Result<RecordBatch> {
    Ok(RecordBatch::try_new(
        beatmap_schema(),
        vec![
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.folder_id.as_str()))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.osu_file.as_str()))),
            Arc::new(Int32Array::from_iter_values(rows.iter().map(|r| r.format_version))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.audio_file.as_str()))),
            Arc::new(Float64Array::from_iter_values(rows.iter().map(|r| r.audio_lead_in))),
            Arc::new(Int32Array::from_iter_values(rows.iter().map(|r| r.preview_time))),
            // General section - new fields
            Arc::new(Int32Array::from_iter_values(rows.iter().map(|r| r.default_sample_bank))),
            Arc::new(Int32Array::from_iter_values(rows.iter().map(|r| r.default_sample_volume))),
            Arc::new(Float32Array::from_iter_values(rows.iter().map(|r| r.stack_leniency))),
            Arc::new(Int32Array::from_iter_values(rows.iter().map(|r| r.mode))),
            Arc::new(BooleanArray::from_iter(rows.iter().map(|r| Some(r.letterbox_in_breaks)))),
            Arc::new(BooleanArray::from_iter(rows.iter().map(|r| Some(r.special_style)))),
            Arc::new(BooleanArray::from_iter(rows.iter().map(|r| Some(r.widescreen_storyboard)))),
            Arc::new(BooleanArray::from_iter(rows.iter().map(|r| Some(r.epilepsy_warning)))),
            Arc::new(BooleanArray::from_iter(rows.iter().map(|r| Some(r.samples_match_playback_rate)))),
            Arc::new(Int32Array::from_iter_values(rows.iter().map(|r| r.countdown))),
            Arc::new(Int32Array::from_iter_values(rows.iter().map(|r| r.countdown_offset))),
            // Editor section
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.bookmarks.as_str()))),
            Arc::new(Float64Array::from_iter_values(rows.iter().map(|r| r.distance_spacing))),
            Arc::new(Int32Array::from_iter_values(rows.iter().map(|r| r.beat_divisor))),
            Arc::new(Int32Array::from_iter_values(rows.iter().map(|r| r.grid_size))),
            Arc::new(Float64Array::from_iter_values(rows.iter().map(|r| r.timeline_zoom))),
            // Metadata section
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.title.as_str()))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.title_unicode.as_str()))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.artist.as_str()))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.artist_unicode.as_str()))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.creator.as_str()))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.version.as_str()))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.source.as_str()))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.tags.as_str()))),
            Arc::new(Int32Array::from_iter_values(rows.iter().map(|r| r.beatmap_id))),
            Arc::new(Int32Array::from_iter_values(rows.iter().map(|r| r.beatmap_set_id))),
            // Difficulty section
            Arc::new(Float32Array::from_iter_values(rows.iter().map(|r| r.hp_drain_rate))),
            Arc::new(Float32Array::from_iter_values(rows.iter().map(|r| r.circle_size))),
            Arc::new(Float32Array::from_iter_values(rows.iter().map(|r| r.overall_difficulty))),
            Arc::new(Float32Array::from_iter_values(rows.iter().map(|r| r.approach_rate))),
            Arc::new(Float64Array::from_iter_values(rows.iter().map(|r| r.slider_multiplier))),
            Arc::new(Float64Array::from_iter_values(rows.iter().map(|r| r.slider_tick_rate))),
            // Events section
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.background_file.as_str()))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.audio_path.as_str()))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.background_path.as_str()))),
        ],
    )?)
}

pub fn hit_object_rows_to_batch(rows: &[HitObjectRow]) -> Result<RecordBatch> {
    Ok(RecordBatch::try_new(
        hit_object_schema(),
        vec![
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.folder_id.as_str()))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.osu_file.as_str()))),
            Arc::new(Int32Array::from_iter_values(rows.iter().map(|r| r.index))),
            Arc::new(Float64Array::from_iter_values(rows.iter().map(|r| r.start_time))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.object_type.as_str()))),
            Arc::new(Int32Array::from(rows.iter().map(|r| r.pos_x).collect::<Vec<_>>())),
            Arc::new(Int32Array::from(rows.iter().map(|r| r.pos_y).collect::<Vec<_>>())),
            Arc::new(BooleanArray::from_iter(rows.iter().map(|r| Some(r.new_combo)))),
            Arc::new(Int32Array::from_iter_values(rows.iter().map(|r| r.combo_offset))),
            Arc::new(StringArray::from(rows.iter().map(|r| r.curve_type.as_deref()).collect::<Vec<_>>())),
            Arc::new(Int32Array::from(rows.iter().map(|r| r.slides).collect::<Vec<_>>())),
            Arc::new(Float64Array::from(rows.iter().map(|r| r.length).collect::<Vec<_>>())),
            Arc::new(Float64Array::from(rows.iter().map(|r| r.end_time).collect::<Vec<_>>())),
        ],
    )?)
}

pub fn timing_point_rows_to_batch(rows: &[TimingPointRow]) -> Result<RecordBatch> {
    Ok(RecordBatch::try_new(
        timing_point_schema(),
        vec![
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.folder_id.as_str()))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.osu_file.as_str()))),
            Arc::new(Float64Array::from_iter_values(rows.iter().map(|r| r.time))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.point_type.as_str()))),
            Arc::new(Float64Array::from(rows.iter().map(|r| r.beat_length).collect::<Vec<_>>())),
            Arc::new(StringArray::from(rows.iter().map(|r| r.time_signature.as_deref()).collect::<Vec<_>>())),
            Arc::new(Float64Array::from(rows.iter().map(|r| r.slider_velocity).collect::<Vec<_>>())),
            Arc::new(BooleanArray::from(rows.iter().map(|r| r.kiai).collect::<Vec<_>>())),
            Arc::new(StringArray::from(rows.iter().map(|r| r.sample_bank.as_deref()).collect::<Vec<_>>())),
            Arc::new(Int32Array::from(rows.iter().map(|r| r.sample_volume).collect::<Vec<_>>())),
        ],
    )?)
}

pub fn storyboard_element_rows_to_batch(rows: &[StoryboardElementRow]) -> Result<RecordBatch> {
    Ok(RecordBatch::try_new(
        storyboard_element_schema(),
        vec![
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.folder_id.as_str()))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.source_file.as_str()))),
            Arc::new(Int32Array::from_iter_values(rows.iter().map(|r| r.element_index))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.layer_name.as_str()))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.element_path.as_str()))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.element_type.as_str()))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.origin.as_str()))),
            Arc::new(Float32Array::from_iter_values(rows.iter().map(|r| r.initial_pos_x))),
            Arc::new(Float32Array::from_iter_values(rows.iter().map(|r| r.initial_pos_y))),
            Arc::new(Int32Array::from(rows.iter().map(|r| r.frame_count).collect::<Vec<_>>())),
            Arc::new(Float64Array::from(rows.iter().map(|r| r.frame_delay).collect::<Vec<_>>())),
            Arc::new(StringArray::from(rows.iter().map(|r| r.loop_type.as_deref()).collect::<Vec<_>>())),
            Arc::new(BooleanArray::from_iter(rows.iter().map(|r| Some(r.is_embedded)))),
        ],
    )?)
}

pub fn storyboard_command_rows_to_batch(rows: &[StoryboardCommandRow]) -> Result<RecordBatch> {
    Ok(RecordBatch::try_new(
        storyboard_command_schema(),
        vec![
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.folder_id.as_str()))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.source_file.as_str()))),
            Arc::new(Int32Array::from_iter_values(rows.iter().map(|r| r.element_index))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.command_type.as_str()))),
            Arc::new(Float64Array::from_iter_values(rows.iter().map(|r| r.start_time))),
            Arc::new(Float64Array::from_iter_values(rows.iter().map(|r| r.end_time))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.start_value.as_str()))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.end_value.as_str()))),
            Arc::new(Int32Array::from_iter_values(rows.iter().map(|r| r.easing))),
            Arc::new(BooleanArray::from_iter(rows.iter().map(|r| Some(r.is_embedded)))),
        ],
    )?)
}

pub fn slider_control_point_rows_to_batch(rows: &[SliderControlPointRow]) -> Result<RecordBatch> {
    Ok(RecordBatch::try_new(
        slider_control_point_schema(),
        vec![
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.folder_id.as_str()))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.osu_file.as_str()))),
            Arc::new(Int32Array::from_iter_values(rows.iter().map(|r| r.hit_object_index))),
            Arc::new(Int32Array::from_iter_values(rows.iter().map(|r| r.point_index))),
            Arc::new(Float32Array::from_iter_values(rows.iter().map(|r| r.pos_x))),
            Arc::new(Float32Array::from_iter_values(rows.iter().map(|r| r.pos_y))),
            Arc::new(StringArray::from(rows.iter().map(|r| r.path_type.as_deref()).collect::<Vec<_>>())),
        ],
    )?)
}

pub fn slider_data_rows_to_batch(rows: &[SliderDataRow]) -> Result<RecordBatch> {
    Ok(RecordBatch::try_new(
        slider_data_schema(),
        vec![
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.folder_id.as_str()))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.osu_file.as_str()))),
            Arc::new(Int32Array::from_iter_values(rows.iter().map(|r| r.hit_object_index))),
            Arc::new(Int32Array::from_iter_values(rows.iter().map(|r| r.repeat_count))),
            Arc::new(Float64Array::from_iter_values(rows.iter().map(|r| r.velocity))),
            Arc::new(Float64Array::from(rows.iter().map(|r| r.expected_dist).collect::<Vec<_>>())),
        ],
    )?)
}

pub fn break_rows_to_batch(rows: &[BreakRow]) -> Result<RecordBatch> {
    Ok(RecordBatch::try_new(
        break_schema(),
        vec![
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.folder_id.as_str()))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.osu_file.as_str()))),
            Arc::new(Float64Array::from_iter_values(rows.iter().map(|r| r.start_time))),
            Arc::new(Float64Array::from_iter_values(rows.iter().map(|r| r.end_time))),
        ],
    )?)
}

pub fn combo_color_rows_to_batch(rows: &[ComboColorRow]) -> Result<RecordBatch> {
    Ok(RecordBatch::try_new(
        combo_color_schema(),
        vec![
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.folder_id.as_str()))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.osu_file.as_str()))),
            Arc::new(Int32Array::from_iter_values(rows.iter().map(|r| r.color_index))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.color_type.as_str()))),
            Arc::new(StringArray::from(rows.iter().map(|r| r.custom_name.as_deref()).collect::<Vec<_>>())),
            Arc::new(Int32Array::from_iter_values(rows.iter().map(|r| r.red))),
            Arc::new(Int32Array::from_iter_values(rows.iter().map(|r| r.green))),
            Arc::new(Int32Array::from_iter_values(rows.iter().map(|r| r.blue))),
        ],
    )?)
}

pub fn hit_sample_rows_to_batch(rows: &[HitSampleRow]) -> Result<RecordBatch> {
    Ok(RecordBatch::try_new(
        hit_sample_schema(),
        vec![
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.folder_id.as_str()))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.osu_file.as_str()))),
            Arc::new(Int32Array::from_iter_values(rows.iter().map(|r| r.hit_object_index))),
            Arc::new(Int32Array::from_iter_values(rows.iter().map(|r| r.sample_index))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.name.as_str()))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.bank.as_str()))),
            Arc::new(StringArray::from(rows.iter().map(|r| r.suffix.as_deref()).collect::<Vec<_>>())),
            Arc::new(Int32Array::from_iter_values(rows.iter().map(|r| r.volume))),
        ],
    )?)
}

pub fn storyboard_loop_rows_to_batch(rows: &[StoryboardLoopRow]) -> Result<RecordBatch> {
    Ok(RecordBatch::try_new(
        storyboard_loop_schema(),
        vec![
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.folder_id.as_str()))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.source_file.as_str()))),
            Arc::new(Int32Array::from_iter_values(rows.iter().map(|r| r.element_index))),
            Arc::new(Int32Array::from_iter_values(rows.iter().map(|r| r.loop_index))),
            Arc::new(Float64Array::from_iter_values(rows.iter().map(|r| r.loop_start_time))),
            Arc::new(Int32Array::from_iter_values(rows.iter().map(|r| r.loop_count))),
            Arc::new(BooleanArray::from_iter(rows.iter().map(|r| Some(r.is_embedded)))),
        ],
    )?)
}

pub fn storyboard_trigger_rows_to_batch(rows: &[StoryboardTriggerRow]) -> Result<RecordBatch> {
    Ok(RecordBatch::try_new(
        storyboard_trigger_schema(),
        vec![
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.folder_id.as_str()))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.source_file.as_str()))),
            Arc::new(Int32Array::from_iter_values(rows.iter().map(|r| r.element_index))),
            Arc::new(Int32Array::from_iter_values(rows.iter().map(|r| r.trigger_index))),
            Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.trigger_name.as_str()))),
            Arc::new(Float64Array::from_iter_values(rows.iter().map(|r| r.trigger_start_time))),
            Arc::new(Float64Array::from_iter_values(rows.iter().map(|r| r.trigger_end_time))),
            Arc::new(Int32Array::from_iter_values(rows.iter().map(|r| r.group_number))),
            Arc::new(BooleanArray::from_iter(rows.iter().map(|r| Some(r.is_embedded)))),
        ],
    )?)
}

// ============ Convenience Type Aliases ============

pub type BeatmapWriter = BatchWriter<BeatmapRow, fn(&[BeatmapRow]) -> Result<RecordBatch>>;
pub type HitObjectWriter = BatchWriter<HitObjectRow, fn(&[HitObjectRow]) -> Result<RecordBatch>>;
pub type TimingPointWriter = BatchWriter<TimingPointRow, fn(&[TimingPointRow]) -> Result<RecordBatch>>;
pub type StoryboardElementWriter = BatchWriter<StoryboardElementRow, fn(&[StoryboardElementRow]) -> Result<RecordBatch>>;
pub type StoryboardCommandWriter = BatchWriter<StoryboardCommandRow, fn(&[StoryboardCommandRow]) -> Result<RecordBatch>>;
pub type SliderControlPointWriter = BatchWriter<SliderControlPointRow, fn(&[SliderControlPointRow]) -> Result<RecordBatch>>;
pub type SliderDataWriter = BatchWriter<SliderDataRow, fn(&[SliderDataRow]) -> Result<RecordBatch>>;
pub type BreakWriter = BatchWriter<BreakRow, fn(&[BreakRow]) -> Result<RecordBatch>>;
pub type ComboColorWriter = BatchWriter<ComboColorRow, fn(&[ComboColorRow]) -> Result<RecordBatch>>;
pub type HitSampleWriter = BatchWriter<HitSampleRow, fn(&[HitSampleRow]) -> Result<RecordBatch>>;
pub type StoryboardLoopWriter = BatchWriter<StoryboardLoopRow, fn(&[StoryboardLoopRow]) -> Result<RecordBatch>>;
pub type StoryboardTriggerWriter = BatchWriter<StoryboardTriggerRow, fn(&[StoryboardTriggerRow]) -> Result<RecordBatch>>;

/// Create all batch writers for the dataset
pub struct DatasetWriters {
    pub beatmaps: BeatmapWriter,
    pub hit_objects: HitObjectWriter,
    pub timing_points: TimingPointWriter,
    pub storyboard_elements: StoryboardElementWriter,
    pub storyboard_commands: StoryboardCommandWriter,
    pub slider_control_points: SliderControlPointWriter,
    pub slider_data: SliderDataWriter,
    pub breaks: BreakWriter,
    pub combo_colors: ComboColorWriter,
    pub hit_samples: HitSampleWriter,
    pub storyboard_loops: StoryboardLoopWriter,
    pub storyboard_triggers: StoryboardTriggerWriter,
}

impl DatasetWriters {
    pub fn new(output_dir: &Path) -> Result<Self> {
        Ok(Self {
            beatmaps: BatchWriter::new(
                &output_dir.join("beatmaps.parquet"),
                beatmap_schema(),
                beatmap_rows_to_batch as fn(&[BeatmapRow]) -> Result<RecordBatch>,
            )?,
            hit_objects: BatchWriter::new(
                &output_dir.join("hit_objects.parquet"),
                hit_object_schema(),
                hit_object_rows_to_batch as fn(&[HitObjectRow]) -> Result<RecordBatch>,
            )?,
            timing_points: BatchWriter::new(
                &output_dir.join("timing_points.parquet"),
                timing_point_schema(),
                timing_point_rows_to_batch as fn(&[TimingPointRow]) -> Result<RecordBatch>,
            )?,
            storyboard_elements: BatchWriter::new(
                &output_dir.join("storyboard_elements.parquet"),
                storyboard_element_schema(),
                storyboard_element_rows_to_batch as fn(&[StoryboardElementRow]) -> Result<RecordBatch>,
            )?,
            storyboard_commands: BatchWriter::new(
                &output_dir.join("storyboard_commands.parquet"),
                storyboard_command_schema(),
                storyboard_command_rows_to_batch as fn(&[StoryboardCommandRow]) -> Result<RecordBatch>,
            )?,
            slider_control_points: BatchWriter::new(
                &output_dir.join("slider_control_points.parquet"),
                slider_control_point_schema(),
                slider_control_point_rows_to_batch as fn(&[SliderControlPointRow]) -> Result<RecordBatch>,
            )?,
            slider_data: BatchWriter::new(
                &output_dir.join("slider_data.parquet"),
                slider_data_schema(),
                slider_data_rows_to_batch as fn(&[SliderDataRow]) -> Result<RecordBatch>,
            )?,
            breaks: BatchWriter::new(
                &output_dir.join("breaks.parquet"),
                break_schema(),
                break_rows_to_batch as fn(&[BreakRow]) -> Result<RecordBatch>,
            )?,
            combo_colors: BatchWriter::new(
                &output_dir.join("combo_colors.parquet"),
                combo_color_schema(),
                combo_color_rows_to_batch as fn(&[ComboColorRow]) -> Result<RecordBatch>,
            )?,
            hit_samples: BatchWriter::new(
                &output_dir.join("hit_samples.parquet"),
                hit_sample_schema(),
                hit_sample_rows_to_batch as fn(&[HitSampleRow]) -> Result<RecordBatch>,
            )?,
            storyboard_loops: BatchWriter::new(
                &output_dir.join("storyboard_loops.parquet"),
                storyboard_loop_schema(),
                storyboard_loop_rows_to_batch as fn(&[StoryboardLoopRow]) -> Result<RecordBatch>,
            )?,
            storyboard_triggers: BatchWriter::new(
                &output_dir.join("storyboard_triggers.parquet"),
                storyboard_trigger_schema(),
                storyboard_trigger_rows_to_batch as fn(&[StoryboardTriggerRow]) -> Result<RecordBatch>,
            )?,
        })
    }

    pub fn close(self) -> Result<DatasetStats> {
        Ok(DatasetStats {
            beatmaps: self.beatmaps.close()?,
            hit_objects: self.hit_objects.close()?,
            timing_points: self.timing_points.close()?,
            storyboard_elements: self.storyboard_elements.close()?,
            storyboard_commands: self.storyboard_commands.close()?,
            slider_control_points: self.slider_control_points.close()?,
            slider_data: self.slider_data.close()?,
            breaks: self.breaks.close()?,
            combo_colors: self.combo_colors.close()?,
            hit_samples: self.hit_samples.close()?,
            storyboard_loops: self.storyboard_loops.close()?,
            storyboard_triggers: self.storyboard_triggers.close()?,
        })
    }
}

pub struct DatasetStats {
    pub beatmaps: usize,
    pub hit_objects: usize,
    pub timing_points: usize,
    pub storyboard_elements: usize,
    pub storyboard_commands: usize,
    pub slider_control_points: usize,
    pub slider_data: usize,
    pub breaks: usize,
    pub combo_colors: usize,
    pub hit_samples: usize,
    pub storyboard_loops: usize,
    pub storyboard_triggers: usize,
}
