//! Hit object rendering with white outlines and combo numbers

use crate::beatmap::{BeatmapView, RenderObject, RenderObjectKind, PLAYFIELD_WIDTH, PLAYFIELD_HEIGHT};
use egui::{Color32, FontId, Pos2, Rect, Stroke, Vec2};

/// Playfield renderer with coordinate transformation
pub struct PlayfieldRenderer {
    /// Scale factor to fit playfield in available space
    pub scale: f32,
    /// Offset to center playfield
    pub offset: Vec2,
    /// Playfield rect in screen coordinates
    pub playfield_rect: Rect,
}

impl PlayfieldRenderer {
    /// Create a new renderer sized for the given available rect
    pub fn new(available_rect: Rect) -> Self {
        let available_width = available_rect.width();
        let available_height = available_rect.height();

        // Calculate scale to fit playfield with some padding
        let padding = 20.0;
        let usable_width = available_width - padding * 2.0;
        let usable_height = available_height - padding * 2.0;

        let scale_x = usable_width / PLAYFIELD_WIDTH;
        let scale_y = usable_height / PLAYFIELD_HEIGHT;
        let scale = scale_x.min(scale_y);

        // Center the playfield
        let playfield_width = PLAYFIELD_WIDTH * scale;
        let playfield_height = PLAYFIELD_HEIGHT * scale;
        let offset_x = available_rect.min.x + (available_width - playfield_width) / 2.0;
        let offset_y = available_rect.min.y + (available_height - playfield_height) / 2.0;

        let playfield_rect = Rect::from_min_size(
            Pos2::new(offset_x, offset_y),
            Vec2::new(playfield_width, playfield_height),
        );

        Self {
            scale,
            offset: Vec2::new(offset_x, offset_y),
            playfield_rect,
        }
    }

    /// Convert osu! coordinates to screen coordinates
    pub fn osu_to_screen(&self, x: f32, y: f32) -> Pos2 {
        Pos2::new(
            self.offset.x + x * self.scale,
            self.offset.y + y * self.scale,
        )
    }

    /// Convert screen radius from osu! radius
    pub fn scale_radius(&self, radius: f32) -> f32 {
        radius * self.scale
    }

    /// Draw the playfield background
    pub fn draw_playfield_bg(&self, painter: &egui::Painter) {
        // Dark background
        painter.rect_filled(
            self.playfield_rect,
            0.0,
            Color32::from_rgb(20, 20, 30),
        );

        // Border
        painter.rect_stroke(
            self.playfield_rect,
            0.0,
            Stroke::new(2.0, Color32::from_rgb(60, 60, 80)),
        );
    }

    /// Draw a hit circle with white outline and combo number
    pub fn draw_circle(
        &self,
        painter: &egui::Painter,
        obj: &RenderObject,
        opacity: f32,
        circle_radius: f32,
        _approach_scale: f32,
        current_time: f64,
    ) {
        let center = self.osu_to_screen(obj.x, obj.y);
        let radius = self.scale_radius(circle_radius);
        let alpha = (opacity * 255.0) as u8;

        // Main circle outline
        let stroke_color = Color32::from_rgba_unmultiplied(255, 255, 255, alpha);
        painter.circle_stroke(center, radius, Stroke::new(3.0, stroke_color));

        // Draw approach circle if not hit yet
        let time_until_hit = obj.start_time - current_time;
        if time_until_hit > 0.0 {
            let approach_scale = 1.0 + (time_until_hit / 600.0) as f32 * 2.0; // Scale from 3x to 1x
            let approach_alpha = (opacity * 0.6 * 255.0) as u8;
            let approach_color = Color32::from_rgba_unmultiplied(255, 255, 255, approach_alpha);
            painter.circle_stroke(
                center,
                radius * approach_scale,
                Stroke::new(2.0, approach_color),
            );
        }

        // Combo number in center
        if obj.combo_number > 0 {
            let text_alpha = (opacity * 255.0) as u8;
            let text_color = Color32::from_rgba_unmultiplied(255, 255, 255, text_alpha);
            let font_size = (radius * 0.8).max(12.0);
            
            painter.text(
                center,
                egui::Align2::CENTER_CENTER,
                format!("{}", obj.combo_number),
                FontId::proportional(font_size),
                text_color,
            );
        }
    }

    /// Draw a slider with white outline path and combo number
    pub fn draw_slider(
        &self,
        painter: &egui::Painter,
        obj: &RenderObject,
        opacity: f32,
        circle_radius: f32,
        current_time: f64,
        beatmap: &BeatmapView,
    ) {
        if let RenderObjectKind::Slider { path_points, repeats, .. } = &obj.kind {
            let alpha = (opacity * 255.0) as u8;
            let stroke_color = Color32::from_rgba_unmultiplied(255, 255, 255, alpha);
            let body_color = Color32::from_rgba_unmultiplied(60, 60, 80, alpha);
            let radius = self.scale_radius(circle_radius);

            // Draw slider body as thick line segments
            if path_points.len() >= 2 {
                let screen_points: Vec<Pos2> = path_points
                    .iter()
                    .map(|(x, y)| self.osu_to_screen(*x, *y))
                    .collect();

                // Draw the body (filled path)
                for window in screen_points.windows(2) {
                    // Draw thick body line
                    painter.line_segment(
                        [window[0], window[1]],
                        Stroke::new(radius * 2.0, body_color),
                    );
                }
                
                // Draw the outline on top (thinner white line)
                for window in screen_points.windows(2) {
                    painter.line_segment(
                        [window[0], window[1]],
                        Stroke::new(2.0, stroke_color),
                    );
                }

                // Draw end circle
                if let Some(last) = screen_points.last() {
                    painter.circle_filled(*last, radius, body_color);
                    painter.circle_stroke(*last, radius, Stroke::new(2.0, stroke_color));
                    
                    // Draw reverse arrow at end if there are repeats
                    if *repeats > 0 {
                        self.draw_reverse_arrow(painter, &screen_points, false, radius, alpha);
                    }
                }
                
                // Draw reverse arrow at start if there are 2+ repeats
                if *repeats >= 2 {
                    self.draw_reverse_arrow(painter, &screen_points, true, radius, alpha);
                }
            }

            // Draw start circle (head)
            let center = self.osu_to_screen(obj.x, obj.y);
            painter.circle_filled(center, radius, body_color);
            painter.circle_stroke(center, radius, Stroke::new(3.0, stroke_color));

            // Approach circle
            let time_until_hit = obj.start_time - current_time;
            if time_until_hit > 0.0 {
                let approach_scale = 1.0 + (time_until_hit / 600.0) as f32 * 2.0;
                let approach_alpha = (opacity * 0.6 * 255.0) as u8;
                let approach_color = Color32::from_rgba_unmultiplied(255, 255, 255, approach_alpha);
                painter.circle_stroke(
                    center,
                    radius * approach_scale,
                    Stroke::new(2.0, approach_color),
                );
            }

            // Slider ball during active time
            if let Some((ball_x, ball_y)) = beatmap.slider_ball_position(obj, current_time) {
                let ball_pos = self.osu_to_screen(ball_x, ball_y);
                let ball_alpha = (opacity * 255.0) as u8;
                painter.circle_filled(
                    ball_pos,
                    radius * 0.6,
                    Color32::from_rgba_unmultiplied(255, 255, 255, ball_alpha),
                );
            }

            // Combo number
            if obj.combo_number > 0 {
                let text_alpha = (opacity * 255.0) as u8;
                let text_color = Color32::from_rgba_unmultiplied(255, 255, 255, text_alpha);
                let font_size = (radius * 0.8).max(12.0);
                
                painter.text(
                    center,
                    egui::Align2::CENTER_CENTER,
                    format!("{}", obj.combo_number),
                    FontId::proportional(font_size),
                    text_color,
                );
            }
        }
    }
    
    /// Draw a reverse arrow at a slider endpoint
    fn draw_reverse_arrow(
        &self,
        painter: &egui::Painter,
        screen_points: &[Pos2],
        at_start: bool,
        radius: f32,
        alpha: u8,
    ) {
        let arrow_color = Color32::from_rgba_unmultiplied(255, 255, 255, alpha);
        let arrow_size = radius * 0.5;
        
        // Get the endpoint and a nearby point to calculate direction
        let (endpoint, direction_point) = if at_start {
            // Arrow at start, pointing towards second point
            if screen_points.len() < 2 { return; }
            (screen_points[0], screen_points[1.min(screen_points.len() - 1)])
        } else {
            // Arrow at end, pointing towards second-to-last point
            if screen_points.len() < 2 { return; }
            let last = screen_points.len() - 1;
            (screen_points[last], screen_points[last.saturating_sub(1)])
        };
        
        // Calculate direction vector (pointing inward towards the path)
        let dx = direction_point.x - endpoint.x;
        let dy = direction_point.y - endpoint.y;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 0.001 { return; }
        
        let dir_x = dx / len;
        let dir_y = dy / len;
        
        // Arrow tip position (slightly inside the circle)
        let tip = Pos2::new(
            endpoint.x + dir_x * arrow_size * 0.5,
            endpoint.y + dir_y * arrow_size * 0.5,
        );
        
        // Arrow base positions (perpendicular to direction)
        let perp_x = -dir_y;
        let perp_y = dir_x;
        
        let base1 = Pos2::new(
            tip.x - dir_x * arrow_size + perp_x * arrow_size * 0.5,
            tip.y - dir_y * arrow_size + perp_y * arrow_size * 0.5,
        );
        let base2 = Pos2::new(
            tip.x - dir_x * arrow_size - perp_x * arrow_size * 0.5,
            tip.y - dir_y * arrow_size - perp_y * arrow_size * 0.5,
        );
        
        // Draw arrow as three lines forming a chevron
        painter.line_segment([base1, tip], Stroke::new(3.0, arrow_color));
        painter.line_segment([base2, tip], Stroke::new(3.0, arrow_color));
    }

    /// Draw a spinner with concentric circles
    pub fn draw_spinner(
        &self,
        painter: &egui::Painter,
        obj: &RenderObject,
        opacity: f32,
        current_time: f64,
    ) {
        if let RenderObjectKind::Spinner { duration } = &obj.kind {
            let center = self.osu_to_screen(PLAYFIELD_WIDTH / 2.0, PLAYFIELD_HEIGHT / 2.0);
            let alpha = (opacity * 255.0) as u8;
            let stroke_color = Color32::from_rgba_unmultiplied(255, 255, 255, alpha);

            // Calculate progress through spinner
            let elapsed = (current_time - obj.start_time).max(0.0);
            let progress = (elapsed / duration).min(1.0) as f32;

            // Draw concentric circles
            let max_radius = self.playfield_rect.height() * 0.4;
            for i in 0..3 {
                let radius = max_radius * (0.3 + i as f32 * 0.3);
                painter.circle_stroke(center, radius, Stroke::new(2.0, stroke_color));
            }

            // Progress indicator
            let inner_alpha = (opacity * progress * 255.0) as u8;
            let inner_color = Color32::from_rgba_unmultiplied(255, 255, 255, inner_alpha);
            painter.circle_filled(center, max_radius * 0.2 * progress, inner_color);

            // Rotating line indicator
            let angle = (current_time / 50.0).to_radians() as f32;
            let line_end = Pos2::new(
                center.x + angle.cos() * max_radius,
                center.y + angle.sin() * max_radius,
            );
            painter.line_segment([center, line_end], Stroke::new(2.0, stroke_color));
        }
    }

    /// Draw all visible objects
    pub fn draw_objects(
        &self,
        painter: &egui::Painter,
        beatmap: &BeatmapView,
        current_time: f64,
    ) {
        // Collect visible objects (reverse so earlier objects draw on top)
        let mut visible: Vec<_> = beatmap.visible_objects(current_time).collect();
        visible.reverse();

        for (_, obj, opacity) in visible {
            let approach_scale = beatmap.approach_scale(obj, current_time);

            match &obj.kind {
                RenderObjectKind::Circle => {
                    self.draw_circle(painter, obj, opacity, beatmap.circle_radius, approach_scale, current_time);
                }
                RenderObjectKind::Slider { .. } => {
                    self.draw_slider(painter, obj, opacity, beatmap.circle_radius, current_time, beatmap);
                }
                RenderObjectKind::Spinner { .. } => {
                    self.draw_spinner(painter, obj, opacity, current_time);
                }
            }
        }
    }
}
