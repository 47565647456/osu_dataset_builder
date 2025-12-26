//! Timeline UI component with scrubbing and mini-map

use crate::beatmap::BeatmapView;
use egui::{Color32, Pos2, Rect, Sense, Stroke, Ui, Vec2};

/// Timeline height in pixels
const TIMELINE_HEIGHT: f32 = 60.0;
const MINIMAP_HEIGHT: f32 = 20.0;
const SCRUBBER_HEIGHT: f32 = 30.0;

/// Timeline UI component
pub struct Timeline {
    /// Cached density data for minimap (normalized 0-1 values)
    density_cache: Vec<f32>,
    /// Number of buckets for density calculation
    num_buckets: usize,
}

impl Timeline {
    pub fn new() -> Self {
        Self {
            density_cache: Vec::new(),
            num_buckets: 200,
        }
    }

    /// Compute object density for minimap
    pub fn compute_density(&mut self, beatmap: &BeatmapView) {
        let bucket_duration = beatmap.total_duration / self.num_buckets as f64;
        let mut buckets = vec![0u32; self.num_buckets];

        for obj in &beatmap.objects {
            let start_bucket = ((obj.start_time / bucket_duration) as usize).min(self.num_buckets - 1);
            let end_bucket = ((obj.end_time / bucket_duration) as usize).min(self.num_buckets - 1);
            
            for bucket in start_bucket..=end_bucket {
                buckets[bucket] += 1;
            }
        }

        // Normalize
        let max_density = buckets.iter().copied().max().unwrap_or(1) as f32;
        self.density_cache = buckets
            .into_iter()
            .map(|count| count as f32 / max_density)
            .collect();
    }

    /// Draw the timeline and return if user is seeking
    pub fn show(
        &mut self,
        ui: &mut Ui,
        beatmap: &BeatmapView,
        current_time: f64,
        total_duration: f64,
        current_time_str: &str,
        total_time_str: &str,
    ) -> Option<f64> {
        // Compute density if not cached
        if self.density_cache.is_empty() {
            self.compute_density(beatmap);
        }

        let available_width = ui.available_width();
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(available_width, TIMELINE_HEIGHT),
            Sense::click_and_drag(),
        );

        let painter = ui.painter_at(rect);

        // Background
        painter.rect_filled(rect, 4.0, Color32::from_rgb(30, 30, 40));

        // Calculate regions
        let minimap_rect = Rect::from_min_size(
            rect.min + Vec2::new(60.0, 5.0),
            Vec2::new(rect.width() - 120.0, MINIMAP_HEIGHT),
        );
        let scrubber_rect = Rect::from_min_size(
            Pos2::new(minimap_rect.min.x, minimap_rect.max.y + 5.0),
            Vec2::new(minimap_rect.width(), SCRUBBER_HEIGHT),
        );

        // Draw minimap (object density visualization)
        self.draw_minimap(&painter, minimap_rect);

        // Draw scrubber track
        painter.rect_filled(scrubber_rect, 4.0, Color32::from_rgb(50, 50, 60));

        // Draw progress bar
        let progress = (current_time / total_duration).clamp(0.0, 1.0) as f32;
        let progress_width = scrubber_rect.width() * progress;
        let progress_rect = Rect::from_min_size(
            scrubber_rect.min,
            Vec2::new(progress_width, scrubber_rect.height()),
        );
        painter.rect_filled(progress_rect, 4.0, Color32::from_rgb(100, 150, 255));

        // Draw playhead
        let playhead_x = scrubber_rect.min.x + progress_width;
        let playhead_rect = Rect::from_center_size(
            Pos2::new(playhead_x, scrubber_rect.center().y),
            Vec2::new(12.0, scrubber_rect.height() + 8.0),
        );
        painter.rect_filled(playhead_rect, 4.0, Color32::WHITE);

        // Time labels
        painter.text(
            Pos2::new(rect.min.x + 5.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            current_time_str,
            egui::FontId::monospace(14.0),
            Color32::WHITE,
        );
        painter.text(
            Pos2::new(rect.max.x - 5.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            total_time_str,
            egui::FontId::monospace(14.0),
            Color32::from_rgb(150, 150, 150),
        );

        // Handle seeking via click/drag
        let seek_area = Rect::from_min_max(
            Pos2::new(minimap_rect.min.x, rect.min.y),
            Pos2::new(minimap_rect.max.x, rect.max.y),
        );

        if response.dragged() || response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                if seek_area.contains(pos) {
                    let relative_x = (pos.x - minimap_rect.min.x) / minimap_rect.width();
                    let seek_time = relative_x as f64 * total_duration;
                    return Some(seek_time.clamp(0.0, total_duration));
                }
            }
        }

        None
    }

    /// Draw the minimap showing object density
    fn draw_minimap(&self, painter: &egui::Painter, rect: Rect) {
        if self.density_cache.is_empty() {
            return;
        }

        // Background
        painter.rect_filled(rect, 2.0, Color32::from_rgb(20, 20, 30));

        // Draw density bars
        let bar_width = rect.width() / self.density_cache.len() as f32;
        
        for (i, &density) in self.density_cache.iter().enumerate() {
            if density > 0.0 {
                let bar_height = rect.height() * density;
                let bar_rect = Rect::from_min_size(
                    Pos2::new(
                        rect.min.x + i as f32 * bar_width,
                        rect.max.y - bar_height,
                    ),
                    Vec2::new(bar_width.max(1.0), bar_height),
                );

                // Color based on density (blue to red)
                let color = if density > 0.8 {
                    Color32::from_rgb(255, 100, 100)
                } else if density > 0.5 {
                    Color32::from_rgb(255, 200, 100)
                } else {
                    Color32::from_rgb(100, 150, 255)
                };

                painter.rect_filled(bar_rect, 0.0, color);
            }
        }

        // Border
        painter.rect_stroke(rect, 2.0, Stroke::new(1.0, Color32::from_rgb(60, 60, 80)));
    }
}

impl Default for Timeline {
    fn default() -> Self {
        Self::new()
    }
}
