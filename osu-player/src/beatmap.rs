//! Beatmap wrapper with rendering-optimized data structures
//! Ported from osu-viewer with Bevy Resource integration

use bevy::prelude::*;
use rosu_map::section::general::CountdownType;
use rosu_map::section::hit_objects::{CurveBuffers, HitObjectKind};

/// osu! standard playfield dimensions
pub const PLAYFIELD_WIDTH: f32 = 512.0;
pub const PLAYFIELD_HEIGHT: f32 = 384.0;

/// Precomputed rendering data for a hit object
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct RenderObject {
    /// Start time in milliseconds
    pub start_time: f64,
    /// End time in milliseconds (same as start_time for circles)
    pub end_time: f64,
    /// Position on playfield
    pub x: f32,
    pub y: f32,
    /// Combo number (1-indexed within combo)
    pub combo_number: u32,
    /// Combo color index (0-indexed into combo_colors)
    pub combo_color_index: usize,
    /// Object-specific data
    pub kind: RenderObjectKind,
}

#[derive(Debug, Clone)]
pub enum RenderObjectKind {
    Circle,
    Slider {
        /// Precomputed path points for rendering
        path_points: Vec<(f32, f32)>,
        /// Duration in milliseconds
        duration: f64,
        /// Number of repeats
        repeats: u32,
    },
    Spinner {
        /// Duration in milliseconds
        duration: f64,
    },
}

/// A break period in the beatmap
#[derive(Debug, Clone, Copy)]
pub struct BreakPeriod {
    pub start_time: f64,
    pub end_time: f64,
}

/// Countdown state at a given time
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CountdownState {
    None,
    /// Number to display (3, 2, 1)
    Number(i32),
    Go,
}

/// Wrapper around a parsed beatmap with rendering data
#[derive(Resource)]
pub struct BeatmapView {
    /// Original beatmap data
    pub beatmap: rosu_map::Beatmap,
    /// Precomputed rendering objects, sorted by start_time
    pub objects: Vec<RenderObject>,
    /// Circle radius in osu!pixels
    pub circle_radius: f32,
    /// Approach rate timing (ms before hit that approach circle appears)
    pub approach_time: f64,
    /// Fade in time (ms before hit that object starts fading in)
    pub fade_in_time: f64,
    /// Total duration of the map in milliseconds
    pub total_duration: f64,
    /// Break periods
    pub breaks: Vec<BreakPeriod>,
    /// Countdown type (None, Normal, HalfSpeed, DoubleSpeed)
    pub countdown_type: CountdownType,
    /// First object time
    pub first_object_time: f64,
    /// BPM for countdown timing (from first timing point)
    pub countdown_beat_length: f64,
    /// Total combo count
    pub total_combo: u32,
    /// Combo colors (RGB values)
    pub combo_colors: Vec<[u8; 3]>,
}

impl BeatmapView {
    pub fn new(mut beatmap: rosu_map::Beatmap) -> Self {
        // Calculate circle size (CS) to radius
        // Formula: radius = 54.4 - 4.48 * CS
        let cs = beatmap.circle_size as f32;
        let circle_radius = 54.4 - 4.48 * cs;

        // Calculate approach rate timing
        let ar = beatmap.approach_rate as f64;
        let approach_time = if ar < 5.0 {
            1800.0 - ar * 120.0
        } else {
            1200.0 - (ar - 5.0) * 150.0
        };

        // Fade in is typically 400ms or 2/3 of approach time
        let fade_in_time = (approach_time * 2.0 / 3.0).min(400.0);

        let countdown_type = beatmap.countdown;

        let countdown_beat_length = beatmap
            .control_points
            .timing_points
            .first()
            .map(|tp| tp.beat_len)
            .unwrap_or(500.0);

        // Extract combo colors from beatmap (default to white if none specified)
        let combo_colors: Vec<[u8; 3]> = if beatmap.custom_combo_colors.is_empty() {
            // Default combo colors similar to osu! defaults
            vec![
                [255, 192, 0],   // Orange
                [0, 202, 0],     // Green
                [18, 124, 255],  // Blue
                [242, 24, 57],   // Red
            ]
        } else {
            beatmap.custom_combo_colors.iter()
                .map(|c| [c.0[0], c.0[1], c.0[2]])
                .collect()
        };
        let combo_color_count = combo_colors.len();

        // Process hit objects
        let mut objects = Vec::with_capacity(beatmap.hit_objects.len());
        let mut combo_number = 0u32;
        let mut combo_color_index = 0usize;
        let mut curve_buffers = CurveBuffers::default();

        for hit_object in beatmap.hit_objects.iter_mut() {
            let (is_new_combo, color_skip) = match &hit_object.kind {
                HitObjectKind::Circle(c) => (c.new_combo, c.combo_offset as usize),
                HitObjectKind::Slider(s) => (s.new_combo, s.combo_offset as usize),
                HitObjectKind::Spinner(s) => (s.new_combo, 0),
                HitObjectKind::Hold(_) => (false, 0),
            };

            if is_new_combo {
                combo_number = 1;
                combo_color_index = (combo_color_index + 1 + color_skip) % combo_color_count;
            } else {
                combo_number += 1;
            }

            let render_obj = match &mut hit_object.kind {
                HitObjectKind::Circle(circle) => RenderObject {
                    start_time: hit_object.start_time,
                    end_time: hit_object.start_time,
                    x: circle.pos.x,
                    y: circle.pos.y,
                    combo_number,
                    combo_color_index,
                    kind: RenderObjectKind::Circle,
                },
                HitObjectKind::Slider(slider) => {
                    let slider_x = slider.pos.x;
                    let slider_y = slider.pos.y;

                    let path_points: Vec<(f32, f32)> = {
                        let curve = slider.path.curve_with_bufs(&mut curve_buffers);
                        curve
                            .path()
                            .iter()
                            .map(|pos| (slider_x + pos.x, slider_y + pos.y))
                            .collect()
                    };

                    let total_duration = slider.duration_with_bufs(&mut curve_buffers);
                    let span_count = slider.span_count() as u32;
                    let end_time = hit_object.start_time + total_duration;

                    RenderObject {
                        start_time: hit_object.start_time,
                        end_time,
                        x: slider.pos.x,
                        y: slider.pos.y,
                        combo_number,
                        combo_color_index,
                        kind: RenderObjectKind::Slider {
                            path_points,
                            duration: total_duration,
                            repeats: span_count.saturating_sub(1),
                        },
                    }
                }
                HitObjectKind::Spinner(spinner) => {
                    let end_time = hit_object.start_time + spinner.duration;
                    RenderObject {
                        start_time: hit_object.start_time,
                        end_time,
                        x: PLAYFIELD_WIDTH / 2.0,
                        y: PLAYFIELD_HEIGHT / 2.0,
                        combo_number: 0,
                        combo_color_index: 0,
                        kind: RenderObjectKind::Spinner {
                            duration: spinner.duration,
                        },
                    }
                }
                HitObjectKind::Hold(_) => continue,
            };

            objects.push(render_obj);
        }

        let first_object_time = objects.first().map(|o| o.start_time).unwrap_or(0.0);

        let total_duration = objects
            .iter()
            .map(|o| o.end_time)
            .fold(0.0f64, |a, b| a.max(b))
            + 2000.0;

        let breaks: Vec<BreakPeriod> = beatmap
            .breaks
            .iter()
            .map(|b| BreakPeriod {
                start_time: b.start_time,
                end_time: b.end_time,
            })
            .collect();

        let total_combo: u32 = objects
            .iter()
            .map(|obj| match &obj.kind {
                RenderObjectKind::Circle => 1,
                RenderObjectKind::Slider { repeats, .. } => repeats + 2,
                RenderObjectKind::Spinner { .. } => 1,
            })
            .sum();

        Self {
            beatmap,
            objects,
            circle_radius,
            approach_time,
            fade_in_time,
            total_duration,
            breaks,
            countdown_type,
            first_object_time,
            countdown_beat_length,
            total_combo,
            combo_colors,
        }
    }

    /// Get objects visible at the current time with opacity
    pub fn visible_objects(&self, current_time: f64) -> Vec<(usize, &RenderObject, f32)> {
        let approach = self.approach_time;
        let fade_in = self.fade_in_time;
        let start = current_time - 200.0;
        let end = current_time + approach;

        // Binary search to find the first object that could be visible
        // Objects are sorted by start_time, so we search for the first object
        // where start_time > start - some buffer for end_time consideration
        let search_start = start;
        let first_idx = match self.objects.binary_search_by(|obj| {
            if obj.start_time < search_start {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            }
        }) {
            Ok(idx) => idx,
            Err(idx) => idx.saturating_sub(1), // Go back one to catch objects that started earlier but might still be visible
        };

        // Iterate only from the first potentially visible object
        self.objects[first_idx..]
            .iter()
            .enumerate()
            .map(|(i, obj)| (first_idx + i, obj)) // Adjust index to account for slice offset
            .take_while(|(_, obj)| obj.start_time <= end) // Stop when objects are too far in the future
            .filter(|(_, obj)| obj.end_time >= start) // Filter out objects that ended too early
            .filter_map(|(idx, obj)| {
                let time_until_hit = obj.start_time - current_time;
                let time_since_end = current_time - obj.end_time;

                let opacity = if time_since_end > 0.0 {
                    (1.0 - (time_since_end / 200.0) as f32).max(0.0)
                } else if time_until_hit > approach {
                    0.0
                } else if time_until_hit > approach - fade_in {
                    ((approach - time_until_hit) / fade_in) as f32
                } else {
                    1.0
                };

                if opacity > 0.0 {
                    Some((idx, obj, opacity))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get approach circle scale for a hit object
    pub fn approach_scale(&self, obj: &RenderObject, current_time: f64) -> f32 {
        let time_until_hit = obj.start_time - current_time;
        if time_until_hit <= 0.0 {
            1.0
        } else {
            let progress = (time_until_hit / self.approach_time) as f32;
            1.0 + progress * 2.5
        }
    }

    /// Get combo color for a hit object as RGB (0.0-1.0)
    pub fn combo_color(&self, obj: &RenderObject) -> (f32, f32, f32) {
        let rgb = self.combo_colors.get(obj.combo_color_index)
            .copied()
            .unwrap_or([255, 255, 255]);
        (rgb[0] as f32 / 255.0, rgb[1] as f32 / 255.0, rgb[2] as f32 / 255.0)
    }

    /// Get slider ball position at current time
    pub fn slider_ball_position(&self, obj: &RenderObject, current_time: f64) -> Option<(f32, f32)> {
        if let RenderObjectKind::Slider {
            path_points,
            duration,
            repeats,
        } = &obj.kind
        {
            if current_time < obj.start_time
                || current_time > obj.end_time
                || path_points.is_empty()
            {
                return None;
            }

            let elapsed = current_time - obj.start_time;
            let single_pass_duration = *duration / (*repeats + 1) as f64;
            let pass_number = (elapsed / single_pass_duration) as u32;
            let pass_progress = (elapsed % single_pass_duration) / single_pass_duration;

            let progress = if pass_number % 2 == 0 {
                pass_progress
            } else {
                1.0 - pass_progress
            };

            let path_len = path_points.len();
            let float_idx = progress * (path_len - 1) as f64;
            let idx = float_idx as usize;
            let frac = float_idx.fract() as f32;

            if idx >= path_len - 1 {
                Some(path_points[path_len - 1])
            } else {
                let (x1, y1) = path_points[idx];
                let (x2, y2) = path_points[idx + 1];
                Some((x1 + (x2 - x1) * frac, y1 + (y2 - y1) * frac))
            }
        } else {
            None
        }
    }

    /// Check if we're in a break period
    pub fn is_in_break(&self, current_time: f64) -> Option<&BreakPeriod> {
        self.breaks
            .iter()
            .find(|b| current_time >= b.start_time && current_time <= b.end_time)
    }

    /// Get countdown state at current time
    pub fn get_countdown_state(&self, current_time: f64) -> CountdownState {
        if self.countdown_type == CountdownType::None {
            return CountdownState::None;
        }

        let speed_mult = match self.countdown_type {
            CountdownType::None => return CountdownState::None,
            CountdownType::Normal => 1.0,
            CountdownType::HalfSpeed => 0.5,
            CountdownType::DoubleSpeed => 2.0,
        };

        let beat_len = self.countdown_beat_length / speed_mult;
        let countdown_start = self.first_object_time - beat_len * 4.0;

        if current_time < countdown_start {
            return CountdownState::None;
        }

        if current_time >= self.first_object_time {
            if current_time < self.first_object_time + beat_len * 0.5 {
                return CountdownState::Go;
            }
            return CountdownState::None;
        }

        let time_since_start = current_time - countdown_start;
        let beat_number = (time_since_start / beat_len) as i32;

        match beat_number {
            0 => CountdownState::Number(3),
            1 => CountdownState::Number(2),
            2 => CountdownState::Number(1),
            3 => CountdownState::Go,
            _ => CountdownState::None,
        }
    }

    /// Get current combo count at a given time
    pub fn get_current_combo(&self, current_time: f64) -> u32 {
        // Binary search to find the first object that ends after current_time
        let completed_count = match self.objects.binary_search_by(|obj| {
            if obj.end_time <= current_time {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            }
        }) {
            Ok(idx) => idx,
            Err(idx) => idx, // idx is the insertion point, which is the count of completed objects
        };

        // Sum combo points for all completed objects
        self.objects[..completed_count]
            .iter()
            .map(|obj| match &obj.kind {
                RenderObjectKind::Circle => 1,
                RenderObjectKind::Slider { repeats, .. } => repeats + 2,
                RenderObjectKind::Spinner { .. } => 1,
            })
            .sum()
    }
}
