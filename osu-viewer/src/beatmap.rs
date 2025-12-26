//! Beatmap wrapper with rendering-optimized data structures

use rosu_map::section::hit_objects::{HitObjectKind, CurveBuffers};

/// osu! standard playfield dimensions
pub const PLAYFIELD_WIDTH: f32 = 512.0;
pub const PLAYFIELD_HEIGHT: f32 = 384.0;

/// Precomputed rendering data for a hit object
#[derive(Debug, Clone)]
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

/// Wrapper around a parsed beatmap with rendering data
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
}

impl BeatmapView {
    pub fn new(mut beatmap: rosu_map::Beatmap) -> Self {
        // Calculate circle size (CS) to radius
        // Formula: radius = 54.4 - 4.48 * CS
        let cs = beatmap.circle_size as f32;
        let circle_radius = 54.4 - 4.48 * cs;

        // Calculate approach rate timing
        // AR < 5: approach_time = 1800 - AR * 120
        // AR >= 5: approach_time = 1200 - (AR - 5) * 150
        let ar = beatmap.approach_rate as f64;
        let approach_time = if ar < 5.0 {
            1800.0 - ar * 120.0
        } else {
            1200.0 - (ar - 5.0) * 150.0
        };

        // Fade in is typically 400ms or 2/3 of approach time, whichever is smaller
        let fade_in_time = (approach_time * 2.0 / 3.0).min(400.0);

        // Process hit objects
        let mut objects = Vec::with_capacity(beatmap.hit_objects.len());
        let mut combo_number = 0u32;
        let mut curve_buffers = CurveBuffers::default();

        for hit_object in beatmap.hit_objects.iter_mut() {
            // Check for new combo
            let is_new_combo = match &hit_object.kind {
                HitObjectKind::Circle(c) => c.new_combo,
                HitObjectKind::Slider(s) => s.new_combo,
                HitObjectKind::Spinner(s) => s.new_combo,
                HitObjectKind::Hold(_) => false, // mania, ignore
            };

            if is_new_combo {
                combo_number = 1;
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
                    kind: RenderObjectKind::Circle,
                },
                HitObjectKind::Slider(slider) => {
                    // Compute slider path points using the curve's path() method
                    // Note: curve.path() returns points relative to the first control point,
                    // so we need to offset them by the slider's absolute position
                    let slider_x = slider.pos.x;
                    let slider_y = slider.pos.y;
                    
                    let path_points: Vec<(f32, f32)> = {
                        let curve = slider.path.curve_with_bufs(&mut curve_buffers);
                        curve.path()
                            .iter()
                            .map(|pos| (slider_x + pos.x, slider_y + pos.y))
                            .collect()
                    };
                    
                    // Get slider duration and span count
                    let duration = slider.duration_with_bufs(&mut curve_buffers);
                    let span_count = slider.span_count() as u32;
                    let end_time = hit_object.start_time + duration * span_count as f64;

                    RenderObject {
                        start_time: hit_object.start_time,
                        end_time,
                        x: slider.pos.x,
                        y: slider.pos.y,
                        combo_number,
                        kind: RenderObjectKind::Slider {
                            path_points,
                            duration: end_time - hit_object.start_time,
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
                        combo_number: 0, // Spinners don't show combo numbers
                        kind: RenderObjectKind::Spinner {
                            duration: spinner.duration,
                        },
                    }
                }
                HitObjectKind::Hold(_) => continue, // Skip mania hold notes
            };

            objects.push(render_obj);
        }

        // Calculate total duration
        let total_duration = objects
            .iter()
            .map(|o| o.end_time)
            .fold(0.0f64, |a, b| a.max(b))
            + 2000.0; // Add 2 seconds buffer

        Self {
            beatmap,
            objects,
            circle_radius,
            approach_time,
            fade_in_time,
            total_duration,
        }
    }

    /// Get objects visible at the current time
    /// Returns objects within approach_time before and a small buffer after
    pub fn visible_objects(&self, current_time: f64) -> impl Iterator<Item = (usize, &RenderObject, f32)> {
        let approach = self.approach_time;
        let fade_in = self.fade_in_time;
        let start = current_time - 200.0; // Small buffer after hit for fade out
        let end = current_time + approach;

        self.objects
            .iter()
            .enumerate()
            .filter(move |(_, obj)| {
                obj.start_time <= end && obj.end_time >= start
            })
            .map(move |(idx, obj)| {
                // Calculate opacity based on time
                let time_until_hit = obj.start_time - current_time;
                let time_since_end = current_time - obj.end_time;

                let opacity = if time_since_end > 0.0 {
                    // Object has ended, fade out quickly
                    (1.0 - (time_since_end / 200.0) as f32).max(0.0)
                } else if time_until_hit > approach {
                    // Not visible yet
                    0.0
                } else if time_until_hit > approach - fade_in {
                    // Fading in
                    ((approach - time_until_hit) / fade_in) as f32
                } else {
                    // Fully visible
                    1.0
                };

                (idx, obj, opacity)
            })
            .filter(|(_, _, opacity)| *opacity > 0.0)
    }

    /// Get approach circle scale for a hit object (1.0 = full size, 0.0 = at object)
    pub fn approach_scale(&self, obj: &RenderObject, current_time: f64) -> f32 {
        let time_until_hit = obj.start_time - current_time;
        if time_until_hit <= 0.0 {
            1.0 // At hit time, approach circle is at object size
        } else {
            // Scale from 4x to 1x as we approach
            let progress = (time_until_hit / self.approach_time) as f32;
            1.0 + progress * 3.0 // Goes from 4.0 to 1.0
        }
    }

    /// Get slider ball position at current time
    pub fn slider_ball_position(&self, obj: &RenderObject, current_time: f64) -> Option<(f32, f32)> {
        if let RenderObjectKind::Slider { path_points, duration, repeats } = &obj.kind {
            if current_time < obj.start_time || current_time > obj.end_time || path_points.is_empty() {
                return None;
            }

            let elapsed = current_time - obj.start_time;
            let single_pass_duration = *duration / (*repeats + 1) as f64;
            let pass_number = (elapsed / single_pass_duration) as u32;
            let pass_progress = (elapsed % single_pass_duration) / single_pass_duration;

            // Reverse direction on odd passes
            let progress = if pass_number % 2 == 0 {
                pass_progress
            } else {
                1.0 - pass_progress
            };

            // Interpolate along path
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
}
