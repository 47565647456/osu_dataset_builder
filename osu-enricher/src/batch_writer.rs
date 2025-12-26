//! Batch writers for memory-efficient parquet output
//! Writes data in batches to temp files, then merges with existing on close.

use anyhow::Result;
use arrow::array::*;
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ArrowWriter;
use parquet::file::properties::WriterProperties;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::{BeatmapRow, CommentRow};

const BATCH_SIZE: usize = 100;

/// Merge existing parquet file with new temp file, writing result to final path
fn merge_parquet_files(existing_path: &Path, temp_path: &Path, schema: Arc<Schema>) -> Result<usize> {
    let mut all_batches: Vec<RecordBatch> = Vec::new();
    
    // Read existing file if it exists
    if existing_path.exists() {
        let file = File::open(existing_path)?;
        let reader = ParquetRecordBatchReaderBuilder::try_new(file)?.build()?;
        for batch in reader {
            all_batches.push(batch?);
        }
    }
    
    // Read temp file
    if temp_path.exists() {
        let file = File::open(temp_path)?;
        let reader = ParquetRecordBatchReaderBuilder::try_new(file)?.build()?;
        for batch in reader {
            all_batches.push(batch?);
        }
    }
    
    // Count total rows
    let total_rows: usize = all_batches.iter().map(|b| b.num_rows()).sum();
    
    if total_rows == 0 {
        let _ = fs::remove_file(temp_path);
        return Ok(0);
    }
    
    // Write merged result
    let file = File::create(existing_path)?;
    let props = WriterProperties::builder()
        .set_compression(parquet::basic::Compression::SNAPPY)
        .build();
    let mut writer = ArrowWriter::try_new(file, schema, Some(props))?;
    
    for batch in &all_batches {
        writer.write(batch)?;
    }
    writer.close()?;
    
    // Remove temp file
    let _ = fs::remove_file(temp_path);
    
    Ok(total_rows)
}

// ============ Enriched Beatmap Writer ============

pub fn enriched_schema() -> Arc<Schema> {
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
        
        // is_convert
        Field::new("is_convert", DataType::Boolean, true),
    ]))
}

pub struct EnrichedBatchWriter {
    writer: ArrowWriter<File>,
    buffer: Vec<BeatmapRow>,
    total_rows: usize,
    final_path: PathBuf,
    temp_path: PathBuf,
    schema: Arc<Schema>,
}

impl EnrichedBatchWriter {
    pub fn new(path: &Path) -> Result<Self> {
        let schema = enriched_schema();
        let temp_path = path.with_extension("parquet.tmp");
        let file = File::create(&temp_path)?;
        let props = WriterProperties::builder()
            .set_compression(parquet::basic::Compression::SNAPPY)
            .build();
        let writer = ArrowWriter::try_new(file, schema.clone(), Some(props))?;
        
        Ok(Self {
            writer,
            buffer: Vec::with_capacity(BATCH_SIZE),
            total_rows: 0,
            final_path: path.to_path_buf(),
            temp_path,
            schema,
        })
    }

    pub fn write(&mut self, row: BeatmapRow) -> Result<()> {
        self.buffer.push(row);
        if self.buffer.len() >= BATCH_SIZE {
            self.flush()?;
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        
        let rows = &self.buffer;
        let batch = RecordBatch::try_new(
            self.schema.clone(),
            vec![
                Arc::new(UInt32Array::from_iter_values(rows.iter().map(|r| r.beatmap_id))),
                Arc::new(UInt32Array::from_iter_values(rows.iter().map(|r| r.beatmapset_id))),
                Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.folder_id.as_str()))),
                Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.osu_file.as_str()))),
                Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.mode.as_str()))),
                Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.version.as_str()))),
                Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.url.as_str()))),
                Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.status.as_str()))),
                Arc::new(BooleanArray::from_iter(rows.iter().map(|r| Some(r.is_scoreable)))),
                Arc::new(BooleanArray::from_iter(rows.iter().map(|r| Some(r.convert)))),
                Arc::new(Float32Array::from_iter_values(rows.iter().map(|r| r.ar))),
                Arc::new(Float32Array::from_iter_values(rows.iter().map(|r| r.cs))),
                Arc::new(Float32Array::from_iter_values(rows.iter().map(|r| r.od))),
                Arc::new(Float32Array::from_iter_values(rows.iter().map(|r| r.hp))),
                Arc::new(Float32Array::from_iter_values(rows.iter().map(|r| r.bpm))),
                Arc::new(UInt32Array::from_iter_values(rows.iter().map(|r| r.count_circles))),
                Arc::new(UInt32Array::from_iter_values(rows.iter().map(|r| r.count_sliders))),
                Arc::new(UInt32Array::from_iter_values(rows.iter().map(|r| r.count_spinners))),
                Arc::new(UInt32Array::from_iter_values(rows.iter().map(|r| r.seconds_drain))),
                Arc::new(UInt32Array::from_iter_values(rows.iter().map(|r| r.seconds_total))),
                Arc::new(UInt32Array::from_iter_values(rows.iter().map(|r| r.playcount))),
                Arc::new(UInt32Array::from_iter_values(rows.iter().map(|r| r.passcount))),
                Arc::new(UInt32Array::from(rows.iter().map(|r| r.max_combo_api).collect::<Vec<_>>())),
                Arc::new(Float32Array::from_iter_values(rows.iter().map(|r| r.stars_api))),
                Arc::new(StringArray::from_iter_values(rows.iter().map(|r| r.checksum.as_str()))),
                Arc::new(UInt32Array::from_iter_values(rows.iter().map(|r| r.creator_id))),
                Arc::new(Int64Array::from(rows.iter().map(|r| r.last_updated).collect::<Vec<_>>())),
                Arc::new(Float64Array::from_iter_values(rows.iter().map(|r| r.stars_calc))),
                Arc::new(Float64Array::from_iter_values(rows.iter().map(|r| r.max_pp))),
                Arc::new(UInt32Array::from_iter_values(rows.iter().map(|r| r.max_combo_calc))),
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
                Arc::new(Float64Array::from(rows.iter().map(|r| r.taiko_stamina).collect::<Vec<_>>())),
                Arc::new(Float64Array::from(rows.iter().map(|r| r.taiko_rhythm).collect::<Vec<_>>())),
                Arc::new(Float64Array::from(rows.iter().map(|r| r.taiko_color).collect::<Vec<_>>())),
                Arc::new(Float64Array::from(rows.iter().map(|r| r.taiko_reading).collect::<Vec<_>>())),
                Arc::new(Float64Array::from(rows.iter().map(|r| r.taiko_great_hit_window).collect::<Vec<_>>())),
                Arc::new(Float64Array::from(rows.iter().map(|r| r.taiko_ok_hit_window).collect::<Vec<_>>())),
                Arc::new(Float64Array::from(rows.iter().map(|r| r.taiko_mono_stamina_factor).collect::<Vec<_>>())),
                Arc::new(Float64Array::from(rows.iter().map(|r| r.catch_ar).collect::<Vec<_>>())),
                Arc::new(UInt32Array::from(rows.iter().map(|r| r.catch_n_fruits).collect::<Vec<_>>())),
                Arc::new(UInt32Array::from(rows.iter().map(|r| r.catch_n_droplets).collect::<Vec<_>>())),
                Arc::new(UInt32Array::from(rows.iter().map(|r| r.catch_n_tiny_droplets).collect::<Vec<_>>())),
                Arc::new(UInt32Array::from(rows.iter().map(|r| r.mania_n_objects).collect::<Vec<_>>())),
                Arc::new(UInt32Array::from(rows.iter().map(|r| r.mania_n_hold_notes).collect::<Vec<_>>())),
                Arc::new(BooleanArray::from(rows.iter().map(|r| r.is_convert).collect::<Vec<_>>())),
            ],
        )?;
        
        self.total_rows += self.buffer.len();
        self.writer.write(&batch)?;
        self.buffer.clear();
        Ok(())
    }

    pub fn close(mut self) -> Result<usize> {
        self.flush()?;
        self.writer.close()?;
        
        if self.total_rows == 0 {
            let _ = fs::remove_file(&self.temp_path);
            if self.final_path.exists() {
                let file = File::open(&self.final_path)?;
                let reader = ParquetRecordBatchReaderBuilder::try_new(file)?.build()?;
                let count: usize = reader.map(|b| b.map(|b| b.num_rows()).unwrap_or(0)).sum();
                return Ok(count);
            }
            return Ok(0);
        }
        
        merge_parquet_files(&self.final_path, &self.temp_path, self.schema)
    }
}

// ============ Comments Writer ============

pub fn comments_schema() -> Arc<Schema> {
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

pub struct CommentsBatchWriter {
    writer: ArrowWriter<File>,
    buffer: Vec<CommentRow>,
    total_rows: usize,
    final_path: PathBuf,
    temp_path: PathBuf,
    schema: Arc<Schema>,
}

impl CommentsBatchWriter {
    pub fn new(path: &Path) -> Result<Self> {
        let schema = comments_schema();
        let temp_path = path.with_extension("parquet.tmp");
        let file = File::create(&temp_path)?;
        let props = WriterProperties::builder()
            .set_compression(parquet::basic::Compression::SNAPPY)
            .build();
        let writer = ArrowWriter::try_new(file, schema.clone(), Some(props))?;
        
        Ok(Self {
            writer,
            buffer: Vec::with_capacity(BATCH_SIZE),
            total_rows: 0,
            final_path: path.to_path_buf(),
            temp_path,
            schema,
        })
    }

    pub fn write(&mut self, row: CommentRow) -> Result<()> {
        self.buffer.push(row);
        if self.buffer.len() >= BATCH_SIZE {
            self.flush()?;
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        
        let rows = &self.buffer;
        let batch = RecordBatch::try_new(
            self.schema.clone(),
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
        
        self.total_rows += self.buffer.len();
        self.writer.write(&batch)?;
        self.buffer.clear();
        Ok(())
    }

    pub fn close(mut self) -> Result<usize> {
        self.flush()?;
        self.writer.close()?;
        
        if self.total_rows == 0 {
            let _ = fs::remove_file(&self.temp_path);
            if self.final_path.exists() {
                let file = File::open(&self.final_path)?;
                let reader = ParquetRecordBatchReaderBuilder::try_new(file)?.build()?;
                let count: usize = reader.map(|b| b.map(|b| b.num_rows()).unwrap_or(0)).sum();
                return Ok(count);
            }
            return Ok(0);
        }
        
        merge_parquet_files(&self.final_path, &self.temp_path, self.schema)
    }
}
