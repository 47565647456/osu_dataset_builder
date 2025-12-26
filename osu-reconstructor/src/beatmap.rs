//! Beatmap reconstruction from parquet rows

use anyhow::Result;
use rosu_map::Beatmap;
use rosu_map::section::colors::Color;
use rosu_map::section::events::BreakPeriod;
use rosu_map::section::general::{GameMode, CountdownType};
use rosu_map::section::hit_objects::{
    HitObject, HitObjectKind, HitObjectCircle, HitObjectSlider, HitObjectSpinner, HitObjectHold,
    SliderPath, PathControlPoint, PathType,
    hit_samples::{HitSampleInfo, HitSampleInfoName, SampleBank},
};
use rosu_map::section::timing_points::{TimingPoint, DifficultyPoint, EffectPoint};
use rosu_map::util::Pos;
use std::collections::HashMap;

use crate::types::*;

/// Reconstructor for building Beatmap objects from parquet row data
pub struct BeatmapReconstructor;

impl BeatmapReconstructor {
    /// Reconstruct a Beatmap from row data
    pub fn reconstruct(
        beatmap_row: &BeatmapRow,
        hit_object_rows: &[HitObjectRow],
        timing_point_rows: &[TimingPointRow],
        slider_control_point_rows: &[SliderControlPointRow],
        slider_data_rows: &[SliderDataRow],
        break_rows: &[BreakRow],
        combo_color_rows: &[ComboColorRow],
        hit_sample_rows: &[HitSampleRow],
    ) -> Result<Beatmap> {
        let mut beatmap = Beatmap::default();
        let folder_id = &beatmap_row.folder_id;
        let osu_file = &beatmap_row.osu_file;

        // Set metadata fields
        Self::set_metadata(&mut beatmap, beatmap_row);

        // Add break periods
        for br in break_rows
            .iter()
            .filter(|b| b.folder_id == *folder_id && b.osu_file == *osu_file)
        {
            beatmap.breaks.push(BreakPeriod {
                start_time: br.start_time,
                end_time: br.end_time,
            });
        }

        // Add combo colors
        for cc in combo_color_rows
            .iter()
            .filter(|c| c.folder_id == *folder_id && c.osu_file == *osu_file && c.color_type == "combo")
        {
            beatmap.custom_combo_colors.push(Color::new(
                cc.red as u8,
                cc.green as u8,
                cc.blue as u8,
                255,
            ));
        }

        // Add custom colors (slider track, etc.)
        for cc in combo_color_rows
            .iter()
            .filter(|c| c.folder_id == *folder_id && c.osu_file == *osu_file && c.color_type == "custom")
        {
            if let Some(name) = &cc.custom_name {
                beatmap.custom_colors.push(rosu_map::section::colors::CustomColor {
                    name: name.clone(),
                    color: Color::new(cc.red as u8, cc.green as u8, cc.blue as u8, 255),
                });
            }
        }

        // Build lookup tables for slider data
        let slider_data_map: HashMap<i32, &SliderDataRow> = slider_data_rows
            .iter()
            .filter(|sd| sd.folder_id == *folder_id && sd.osu_file == *osu_file)
            .map(|sd| (sd.hit_object_index, sd))
            .collect();

        let mut slider_cp_map: HashMap<i32, Vec<&SliderControlPointRow>> = HashMap::new();
        for cp in slider_control_point_rows
            .iter()
            .filter(|cp| cp.folder_id == *folder_id && cp.osu_file == *osu_file)
        {
            slider_cp_map.entry(cp.hit_object_index).or_default().push(cp);
        }
        for cps in slider_cp_map.values_mut() {
            cps.sort_by_key(|cp| cp.point_index);
        }

        // Build lookup table for hit samples
        let mut hit_sample_map: HashMap<i32, Vec<&HitSampleRow>> = HashMap::new();
        for hs in hit_sample_rows
            .iter()
            .filter(|hs| hs.folder_id == *folder_id && hs.osu_file == *osu_file)
        {
            hit_sample_map.entry(hs.hit_object_index).or_default().push(hs);
        }
        for samples in hit_sample_map.values_mut() {
            samples.sort_by_key(|s| s.sample_index);
        }

        // Reconstruct hit objects
        let matching_hit_objects: Vec<_> = hit_object_rows
            .iter()
            .filter(|ho| ho.folder_id == *folder_id && ho.osu_file == *osu_file)
            .collect();

        for ho in &matching_hit_objects {
            if let Some(mut hit_obj) = Self::reconstruct_hit_object(ho, &beatmap.mode, &slider_data_map, &slider_cp_map) {
                // Add samples for this hit object
                if let Some(samples) = hit_sample_map.get(&ho.index) {
                    hit_obj.samples = samples
                        .iter()
                        .map(|s| Self::reconstruct_hit_sample(s))
                        .collect();
                }
                beatmap.hit_objects.push(hit_obj);
            }
        }

        // Reconstruct timing points
        for tp in timing_point_rows
            .iter()
            .filter(|tp| tp.folder_id == *folder_id && tp.osu_file == *osu_file)
        {
            Self::add_timing_point(&mut beatmap, tp);
        }

        Ok(beatmap)
    }

    fn reconstruct_hit_sample(hs: &HitSampleRow) -> HitSampleInfo {
        let name = match hs.name.as_str() {
            "Normal" => HitSampleInfoName::Default(rosu_map::section::hit_objects::hit_samples::HitSampleDefaultName::Normal),
            "Whistle" => HitSampleInfoName::Default(rosu_map::section::hit_objects::hit_samples::HitSampleDefaultName::Whistle),
            "Finish" => HitSampleInfoName::Default(rosu_map::section::hit_objects::hit_samples::HitSampleDefaultName::Finish),
            "Clap" => HitSampleInfoName::Default(rosu_map::section::hit_objects::hit_samples::HitSampleDefaultName::Clap),
            other => HitSampleInfoName::File(other.to_string()),
        };
        let bank = match hs.bank.as_str() {
            "Normal" => SampleBank::Normal,
            "Soft" => SampleBank::Soft,
            "Drum" => SampleBank::Drum,
            _ => SampleBank::None,
        };
        HitSampleInfo {
            name,
            bank,
            suffix: hs.suffix.as_ref().and_then(|s| s.parse().ok()).and_then(std::num::NonZeroU32::new),
            volume: hs.volume,
            custom_sample_bank: 0,
            bank_specified: true,
            is_layered: false,
        }
    }


    fn set_metadata(beatmap: &mut Beatmap, row: &BeatmapRow) {
        beatmap.format_version = row.format_version;
        beatmap.audio_file = row.audio_file.clone();
        beatmap.audio_lead_in = row.audio_lead_in;
        beatmap.preview_time = row.preview_time;
        // General section
        beatmap.default_sample_bank = match row.default_sample_bank {
            0 => SampleBank::None,
            1 => SampleBank::Normal,
            2 => SampleBank::Soft,
            3 => SampleBank::Drum,
            _ => SampleBank::None,
        };
        beatmap.default_sample_volume = row.default_sample_volume;
        beatmap.stack_leniency = row.stack_leniency;
        beatmap.mode = match row.mode {
            0 => GameMode::Osu,
            1 => GameMode::Taiko,
            2 => GameMode::Catch,
            3 => GameMode::Mania,
            _ => GameMode::Osu,
        };
        beatmap.letterbox_in_breaks = row.letterbox_in_breaks;
        beatmap.special_style = row.special_style;
        beatmap.widescreen_storyboard = row.widescreen_storyboard;
        beatmap.epilepsy_warning = row.epilepsy_warning;
        beatmap.samples_match_playback_rate = row.samples_match_playback_rate;
        beatmap.countdown = match row.countdown {
            0 => CountdownType::None,
            1 => CountdownType::Normal,
            2 => CountdownType::HalfSpeed,
            3 => CountdownType::DoubleSpeed,
            _ => CountdownType::Normal,
        };
        beatmap.countdown_offset = row.countdown_offset;
        // Editor section
        beatmap.bookmarks = row.bookmarks.split(',').filter_map(|s| s.trim().parse().ok()).collect();
        beatmap.distance_spacing = row.distance_spacing;
        beatmap.beat_divisor = row.beat_divisor;
        beatmap.grid_size = row.grid_size;
        beatmap.timeline_zoom = row.timeline_zoom;
        // Metadata section
        beatmap.title = row.title.clone();
        beatmap.title_unicode = row.title_unicode.clone();
        beatmap.artist = row.artist.clone();
        beatmap.artist_unicode = row.artist_unicode.clone();
        beatmap.creator = row.creator.clone();
        beatmap.version = row.version.clone();
        beatmap.source = row.source.clone();
        beatmap.tags = row.tags.clone();
        beatmap.beatmap_id = row.beatmap_id;
        beatmap.beatmap_set_id = row.beatmap_set_id;
        // Difficulty section
        beatmap.hp_drain_rate = row.hp_drain_rate;
        beatmap.circle_size = row.circle_size;
        beatmap.overall_difficulty = row.overall_difficulty;
        beatmap.approach_rate = row.approach_rate;
        beatmap.slider_multiplier = row.slider_multiplier;
        beatmap.slider_tick_rate = row.slider_tick_rate;
        // Events section
        beatmap.background_file = row.background_file.clone();
    }

    fn reconstruct_hit_object(
        ho: &HitObjectRow,
        mode: &GameMode,
        slider_data_map: &HashMap<i32, &SliderDataRow>,
        slider_cp_map: &HashMap<i32, Vec<&SliderControlPointRow>>,
    ) -> Option<HitObject> {
        match ho.object_type.as_str() {
            "circle" => {
                let circle = HitObjectCircle {
                    pos: Pos {
                        x: ho.pos_x.unwrap_or(0) as f32,
                        y: ho.pos_y.unwrap_or(0) as f32,
                    },
                    new_combo: ho.new_combo,
                    combo_offset: ho.combo_offset,
                };
                Some(HitObject {
                    start_time: ho.start_time,
                    kind: HitObjectKind::Circle(circle),
                    samples: Vec::new(),
                })
            }
            "slider" => {
                let sd = slider_data_map.get(&ho.index)?;
                let control_points: Vec<PathControlPoint> = slider_cp_map
                    .get(&ho.index)
                    .map(|cps| {
                        cps.iter()
                            .map(|cp| {
                                let path_type = cp.path_type.as_ref().and_then(|pt| {
                                    match pt.as_str() {
                                        "Bezier" => Some(PathType::BEZIER),
                                        "Linear" => Some(PathType::LINEAR),
                                        "Catmull" => Some(PathType::CATMULL),
                                        "PerfectCurve" => Some(PathType::PERFECT_CURVE),
                                        _ => None,
                                    }
                                });
                                PathControlPoint {
                                    pos: Pos { x: cp.pos_x, y: cp.pos_y },
                                    path_type,
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                let slider_path = SliderPath::new(*mode, control_points, sd.expected_dist);

                let slider = HitObjectSlider {
                    pos: Pos {
                        x: ho.pos_x.unwrap_or(0) as f32,
                        y: ho.pos_y.unwrap_or(0) as f32,
                    },
                    new_combo: ho.new_combo,
                    combo_offset: ho.combo_offset,
                    path: slider_path,
                    node_samples: Vec::new(),
                    repeat_count: sd.repeat_count,
                    velocity: sd.velocity,
                };
                Some(HitObject {
                    start_time: ho.start_time,
                    kind: HitObjectKind::Slider(slider),
                    samples: Vec::new(),
                })
            }
            "spinner" => {
                let spinner = HitObjectSpinner {
                    pos: Pos {
                        x: ho.pos_x.unwrap_or(256) as f32,
                        y: ho.pos_y.unwrap_or(192) as f32,
                    },
                    duration: ho.end_time.unwrap_or(0.0),
                    new_combo: ho.new_combo,
                };
                Some(HitObject {
                    start_time: ho.start_time,
                    kind: HitObjectKind::Spinner(spinner),
                    samples: Vec::new(),
                })
            }
            "hold" => {
                let hold = HitObjectHold {
                    pos_x: ho.pos_x.unwrap_or(0) as f32,
                    duration: ho.end_time.unwrap_or(0.0),
                };
                Some(HitObject {
                    start_time: ho.start_time,
                    kind: HitObjectKind::Hold(hold),
                    samples: Vec::new(),
                })
            }
            _ => None,
        }
    }

    fn add_timing_point(beatmap: &mut Beatmap, tp: &TimingPointRow) {
        match tp.point_type.as_str() {
            "timing" => {
                beatmap.control_points.timing_points.push(TimingPoint {
                    time: tp.time,
                    beat_len: tp.beat_length.unwrap_or(500.0),
                    ..Default::default()
                });
            }
            "difficulty" => {
                beatmap.control_points.difficulty_points.push(DifficultyPoint {
                    time: tp.time,
                    slider_velocity: tp.slider_velocity.unwrap_or(1.0),
                    ..Default::default()
                });
            }
            "effect" => {
                beatmap.control_points.effect_points.push(EffectPoint {
                    time: tp.time,
                    kiai: tp.kiai.unwrap_or(false),
                    ..Default::default()
                });
            }
            _ => {}
        }
    }
}
