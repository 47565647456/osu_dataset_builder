//! Hit object rendering with white outlines and combo numbers

use crate::beatmap::{BeatmapView, RenderObject, RenderObjectKind, PLAYFIELD_WIDTH, PLAYFIELD_HEIGHT};
use egui::{Color32, FontId, Pos2, Rect, Stroke, Vec2, Mesh, epaint::Vertex};

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

    /// Generate a tessellated mesh for a slider body
    fn generate_slider_mesh(
        &self,
        screen_points: &[Pos2],
        radius: f32,
        body_color: Color32,
        border_color: Color32,
    ) -> (Mesh, Mesh) {
        let mut body_mesh = Mesh::default();
        let mut border_mesh = Mesh::default();
        
        if screen_points.len() < 2 {
            return (body_mesh, border_mesh);
        }
        
        let border_width = 2.0;
        let inner_radius = radius - border_width;
        
        // Generate offset points along the path
        let mut left_points_outer: Vec<Pos2> = Vec::with_capacity(screen_points.len());
        let mut right_points_outer: Vec<Pos2> = Vec::with_capacity(screen_points.len());
        let mut left_points_inner: Vec<Pos2> = Vec::with_capacity(screen_points.len());
        let mut right_points_inner: Vec<Pos2> = Vec::with_capacity(screen_points.len());
        
        for i in 0..screen_points.len() {
            // Calculate tangent direction
            let tangent = if i == 0 {
                let dx = screen_points[1].x - screen_points[0].x;
                let dy = screen_points[1].y - screen_points[0].y;
                let len = (dx * dx + dy * dy).sqrt().max(0.001);
                Vec2::new(dx / len, dy / len)
            } else if i == screen_points.len() - 1 {
                let dx = screen_points[i].x - screen_points[i - 1].x;
                let dy = screen_points[i].y - screen_points[i - 1].y;
                let len = (dx * dx + dy * dy).sqrt().max(0.001);
                Vec2::new(dx / len, dy / len)
            } else {
                // Average of incoming and outgoing tangent
                let dx1 = screen_points[i].x - screen_points[i - 1].x;
                let dy1 = screen_points[i].y - screen_points[i - 1].y;
                let dx2 = screen_points[i + 1].x - screen_points[i].x;
                let dy2 = screen_points[i + 1].y - screen_points[i].y;
                let dx = dx1 + dx2;
                let dy = dy1 + dy2;
                let len = (dx * dx + dy * dy).sqrt().max(0.001);
                Vec2::new(dx / len, dy / len)
            };
            
            // Perpendicular (normal) direction
            let normal = Vec2::new(-tangent.y, tangent.x);
            
            let point = screen_points[i];
            left_points_outer.push(Pos2::new(point.x + normal.x * radius, point.y + normal.y * radius));
            right_points_outer.push(Pos2::new(point.x - normal.x * radius, point.y - normal.y * radius));
            left_points_inner.push(Pos2::new(point.x + normal.x * inner_radius, point.y + normal.y * inner_radius));
            right_points_inner.push(Pos2::new(point.x - normal.x * inner_radius, point.y - normal.y * inner_radius));
        }
        
        // Build body mesh (inner fill)
        let white_uv = Pos2::new(0.0, 0.0); // egui uses this for solid colors
        
        for i in 0..screen_points.len() - 1 {
            let base_idx = body_mesh.vertices.len() as u32;
            
            // Add 4 vertices for this segment
            body_mesh.vertices.push(Vertex { pos: left_points_inner[i], uv: white_uv, color: body_color });
            body_mesh.vertices.push(Vertex { pos: right_points_inner[i], uv: white_uv, color: body_color });
            body_mesh.vertices.push(Vertex { pos: left_points_inner[i + 1], uv: white_uv, color: body_color });
            body_mesh.vertices.push(Vertex { pos: right_points_inner[i + 1], uv: white_uv, color: body_color });
            
            // Two triangles for the quad
            body_mesh.indices.extend_from_slice(&[
                base_idx, base_idx + 1, base_idx + 2,
                base_idx + 1, base_idx + 3, base_idx + 2,
            ]);
        }
        
        // Build border mesh (outer ring)
        for i in 0..screen_points.len() - 1 {
            let base_idx = border_mesh.vertices.len() as u32;
            
            // Left border strip
            border_mesh.vertices.push(Vertex { pos: left_points_outer[i], uv: white_uv, color: border_color });
            border_mesh.vertices.push(Vertex { pos: left_points_inner[i], uv: white_uv, color: border_color });
            border_mesh.vertices.push(Vertex { pos: left_points_outer[i + 1], uv: white_uv, color: border_color });
            border_mesh.vertices.push(Vertex { pos: left_points_inner[i + 1], uv: white_uv, color: border_color });
            
            border_mesh.indices.extend_from_slice(&[
                base_idx, base_idx + 1, base_idx + 2,
                base_idx + 1, base_idx + 3, base_idx + 2,
            ]);
            
            // Right border strip
            let base_idx = border_mesh.vertices.len() as u32;
            border_mesh.vertices.push(Vertex { pos: right_points_inner[i], uv: white_uv, color: border_color });
            border_mesh.vertices.push(Vertex { pos: right_points_outer[i], uv: white_uv, color: border_color });
            border_mesh.vertices.push(Vertex { pos: right_points_inner[i + 1], uv: white_uv, color: border_color });
            border_mesh.vertices.push(Vertex { pos: right_points_outer[i + 1], uv: white_uv, color: border_color });
            
            border_mesh.indices.extend_from_slice(&[
                base_idx, base_idx + 1, base_idx + 2,
                base_idx + 1, base_idx + 3, base_idx + 2,
            ]);
        }
        
        (body_mesh, border_mesh)
    }
    
    /// Generate a circle mesh for slider caps
    fn generate_circle_mesh(&self, center: Pos2, radius: f32, color: Color32, segments: usize) -> Mesh {
        let mut mesh = Mesh::default();
        let white_uv = Pos2::new(0.0, 0.0);
        
        // Center vertex
        let center_idx = mesh.vertices.len() as u32;
        mesh.vertices.push(Vertex { pos: center, uv: white_uv, color });
        
        // Perimeter vertices
        for i in 0..=segments {
            let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
            let x = center.x + angle.cos() * radius;
            let y = center.y + angle.sin() * radius;
            mesh.vertices.push(Vertex { pos: Pos2::new(x, y), uv: white_uv, color });
        }
        
        // Triangle fan
        for i in 0..segments as u32 {
            mesh.indices.extend_from_slice(&[center_idx, center_idx + 1 + i, center_idx + 2 + i]);
        }
        
        mesh
    }
    
    /// Generate a circle border mesh (ring)
    fn generate_circle_border_mesh(&self, center: Pos2, outer_radius: f32, inner_radius: f32, color: Color32, segments: usize) -> Mesh {
        let mut mesh = Mesh::default();
        let white_uv = Pos2::new(0.0, 0.0);
        
        for i in 0..=segments {
            let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
            let cos_a = angle.cos();
            let sin_a = angle.sin();
            
            mesh.vertices.push(Vertex { 
                pos: Pos2::new(center.x + cos_a * outer_radius, center.y + sin_a * outer_radius), 
                uv: white_uv, 
                color 
            });
            mesh.vertices.push(Vertex { 
                pos: Pos2::new(center.x + cos_a * inner_radius, center.y + sin_a * inner_radius), 
                uv: white_uv, 
                color 
            });
        }
        
        for i in 0..segments as u32 {
            let base = i * 2;
            mesh.indices.extend_from_slice(&[
                base, base + 1, base + 2,
                base + 1, base + 3, base + 2,
            ]);
        }
        
        mesh
    }

    /// Draw a slider with tessellated mesh body
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
            let border_width = 2.0;
            let inner_radius = radius - border_width;

            if path_points.len() >= 2 {
                // Adaptive detail: more points for complex sliders
                // Calculate approximate path length to determine detail level
                let path_len: f32 = path_points.windows(2)
                    .map(|w| {
                        let dx = w[1].0 - w[0].0;
                        let dy = w[1].1 - w[0].1;
                        (dx * dx + dy * dy).sqrt()
                    })
                    .sum();
                
                // Target ~1 point per 3 osu!pixels for smooth curves
                // But cap between 50-500 points for performance
                let target_points = ((path_len / 3.0) as usize).clamp(50, 500);
                let step = (path_points.len() / target_points).max(1);
                
                let screen_points: Vec<Pos2> = path_points
                    .iter()
                    .step_by(step)
                    .chain(std::iter::once(path_points.last().unwrap())) // Always include last point
                    .map(|(x, y)| self.osu_to_screen(*x, *y))
                    .collect();

                // Generate and draw slider body mesh
                let (body_mesh, border_mesh) = self.generate_slider_mesh(&screen_points, radius, body_color, stroke_color);
                
                painter.add(egui::Shape::mesh(body_mesh));
                painter.add(egui::Shape::mesh(border_mesh));

                // Draw end cap (filled circle + border ring)
                if let Some(&last) = screen_points.last() {
                    let cap_body = self.generate_circle_mesh(last, inner_radius, body_color, 24);
                    let cap_border = self.generate_circle_border_mesh(last, radius, inner_radius, stroke_color, 24);
                    painter.add(egui::Shape::mesh(cap_body));
                    painter.add(egui::Shape::mesh(cap_border));
                }
            }

            // Draw start circle (head) with mesh
            let center = self.osu_to_screen(obj.x, obj.y);
            let head_body = self.generate_circle_mesh(center, inner_radius, body_color, 24);
            let head_border = self.generate_circle_border_mesh(center, radius, inner_radius, stroke_color, 24);
            painter.add(egui::Shape::mesh(head_body));
            painter.add(egui::Shape::mesh(head_border));

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
            
            // Draw reverse arrows LAST so they appear on top of everything
            if path_points.len() >= 2 {
                // For arrows, we need: first point, second point (for start arrow direction)
                // and last point, second-to-last point (for end arrow direction)
                let first_screen = self.osu_to_screen(path_points[0].0, path_points[0].1);
                let second_screen = self.osu_to_screen(path_points[1].0, path_points[1].1);
                let last_idx = path_points.len() - 1;
                let second_last_idx = path_points.len().saturating_sub(2);
                let last_screen = self.osu_to_screen(path_points[last_idx].0, path_points[last_idx].1);
                let second_last_screen = self.osu_to_screen(path_points[second_last_idx].0, path_points[second_last_idx].1);
                
                // Build minimal screen_points for arrow drawing:
                // [first, second, ..., second_last, last]
                let arrow_points = vec![first_screen, second_screen, second_last_screen, last_screen];
                
                // Draw reverse arrow at end if there are repeats
                if *repeats > 0 {
                    self.draw_reverse_arrow(painter, &arrow_points, false, radius, alpha);
                }
                
                // Draw reverse arrow at start if there are 2+ repeats
                if *repeats >= 2 {
                    self.draw_reverse_arrow(painter, &arrow_points, true, radius, alpha);
                }
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
        let arrow_size = radius * 0.7;
        
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
        
        // Arrow tip position (at the circle edge, pointing inward)
        let tip = Pos2::new(
            endpoint.x + dir_x * arrow_size * 0.3,
            endpoint.y + dir_y * arrow_size * 0.3,
        );
        
        // Arrow base positions (perpendicular to direction)
        let perp_x = -dir_y;
        let perp_y = dir_x;
        
        let base1 = Pos2::new(
            tip.x - dir_x * arrow_size + perp_x * arrow_size * 0.6,
            tip.y - dir_y * arrow_size + perp_y * arrow_size * 0.6,
        );
        let base2 = Pos2::new(
            tip.x - dir_x * arrow_size - perp_x * arrow_size * 0.6,
            tip.y - dir_y * arrow_size - perp_y * arrow_size * 0.6,
        );
        
        // Draw arrow as chevron with thicker lines
        painter.line_segment([base1, tip], Stroke::new(4.0, arrow_color));
        painter.line_segment([base2, tip], Stroke::new(4.0, arrow_color));
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
    
    /// Draw countdown overlay (3, 2, 1, Go!)
    pub fn draw_countdown(
        &self,
        painter: &egui::Painter,
        beatmap: &BeatmapView,
        current_time: f64,
    ) {
        use crate::beatmap::CountdownState;
        
        let state = beatmap.get_countdown_state(current_time);
        
        let text = match state {
            CountdownState::None => return,
            CountdownState::Number(n) => format!("{}", n),
            CountdownState::Go => "Go!".to_string(),
        };
        
        let center = Pos2::new(
            self.playfield_rect.center().x,
            self.playfield_rect.center().y,
        );
        
        // Calculate pulse animation
        let beat_len = beatmap.countdown_beat_length;
        let time_in_beat = current_time % beat_len;
        let beat_progress = (time_in_beat / beat_len) as f32;
        let scale = 1.0 + (1.0 - beat_progress) * 0.3; // Pulse from 1.3 to 1.0
        
        // Large countdown number/text
        let font_size = 120.0 * scale;
        let alpha = ((1.0 - beat_progress * 0.5) * 255.0) as u8;
        
        // Color based on countdown value
        let color = match state {
            CountdownState::Number(3) => Color32::from_rgba_unmultiplied(255, 100, 100, alpha),
            CountdownState::Number(2) => Color32::from_rgba_unmultiplied(255, 200, 100, alpha),
            CountdownState::Number(1) => Color32::from_rgba_unmultiplied(100, 255, 100, alpha),
            CountdownState::Number(_) => Color32::from_rgba_unmultiplied(255, 255, 255, alpha), // fallback
            CountdownState::Go => Color32::from_rgba_unmultiplied(100, 200, 255, alpha),
            CountdownState::None => return,
        };
        
        // Draw shadow
        painter.text(
            Pos2::new(center.x + 4.0, center.y + 4.0),
            egui::Align2::CENTER_CENTER,
            &text,
            FontId::proportional(font_size),
            Color32::from_rgba_unmultiplied(0, 0, 0, alpha / 2),
        );
        
        // Draw main text
        painter.text(
            center,
            egui::Align2::CENTER_CENTER,
            &text,
            FontId::proportional(font_size),
            color,
        );
    }
    
    /// Draw break overlay
    pub fn draw_break(
        &self,
        painter: &egui::Painter,
        beatmap: &BeatmapView,
        current_time: f64,
    ) {
        if let Some(break_period) = beatmap.is_in_break(current_time) {
            let break_duration = break_period.end_time - break_period.start_time;
            let time_in_break = current_time - break_period.start_time;
            let time_remaining = break_period.end_time - current_time;
            
            // Only show break indicator if break is long enough (> 2 seconds)
            if break_duration < 2000.0 {
                return;
            }
            
            let center = Pos2::new(
                self.playfield_rect.center().x,
                self.playfield_rect.min.y + 60.0,
            );
            
            // Fade in/out
            let fade_duration = 500.0;
            let alpha = if time_in_break < fade_duration {
                (time_in_break / fade_duration * 255.0) as u8
            } else if time_remaining < fade_duration {
                (time_remaining / fade_duration * 255.0) as u8
            } else {
                255
            };
            
            // Draw "Break" text
            let text_color = Color32::from_rgba_unmultiplied(255, 255, 255, alpha);
            painter.text(
                center,
                egui::Align2::CENTER_CENTER,
                "Break",
                FontId::proportional(36.0),
                text_color,
            );
            
            // Draw progress bar
            let bar_width = 200.0;
            let bar_height = 6.0;
            let bar_y = center.y + 30.0;
            
            let bar_bg = Rect::from_center_size(
                Pos2::new(center.x, bar_y),
                Vec2::new(bar_width, bar_height),
            );
            
            let progress = (time_in_break / break_duration) as f32;
            let bar_fill = Rect::from_min_size(
                Pos2::new(bar_bg.min.x, bar_bg.min.y),
                Vec2::new(bar_width * progress, bar_height),
            );
            
            let bg_alpha = alpha / 2;
            painter.rect_filled(bar_bg, 3.0, Color32::from_rgba_unmultiplied(40, 40, 40, bg_alpha));
            painter.rect_filled(bar_fill, 3.0, Color32::from_rgba_unmultiplied(255, 255, 255, alpha));
            painter.rect_stroke(bar_bg, 3.0, Stroke::new(1.0, Color32::from_rgba_unmultiplied(100, 100, 100, bg_alpha)));
            
            // Time remaining
            let seconds_remaining = (time_remaining / 1000.0).ceil() as i32;
            painter.text(
                Pos2::new(center.x, bar_y + 20.0),
                egui::Align2::CENTER_CENTER,
                format!("{}s", seconds_remaining),
                FontId::proportional(16.0),
                Color32::from_rgba_unmultiplied(180, 180, 180, alpha),
            );
        }
    }
    
    /// Draw combo counter in top-left corner
    pub fn draw_combo_counter(
        &self,
        painter: &egui::Painter,
        beatmap: &BeatmapView,
        current_time: f64,
    ) {
        let current_combo = beatmap.get_current_combo(current_time);
        let total_combo = beatmap.total_combo;
        
        let pos = Pos2::new(self.playfield_rect.min.x + 10.0, self.playfield_rect.min.y + 10.0);
        
        // Background
        let text = format!("{} / {}x", current_combo, total_combo);
        let bg_rect = Rect::from_min_size(pos, Vec2::new(100.0, 28.0));
        painter.rect_filled(bg_rect, 4.0, Color32::from_rgba_unmultiplied(0, 0, 0, 180));
        
        // Text
        painter.text(
            Pos2::new(pos.x + 6.0, pos.y + 4.0),
            egui::Align2::LEFT_TOP,
            "Combo",
            FontId::proportional(10.0),
            Color32::from_rgb(150, 150, 150),
        );
        
        painter.text(
            Pos2::new(pos.x + 6.0, pos.y + 14.0),
            egui::Align2::LEFT_TOP,
            text,
            FontId::proportional(12.0),
            Color32::WHITE,
        );
    }
    
    /// Draw map stats panel on the left side
    pub fn draw_map_stats(
        &self,
        painter: &egui::Painter,
        beatmap: &BeatmapView,
    ) {
        let bm = &beatmap.beatmap;
        
        // Position below combo counter
        let pos = Pos2::new(self.playfield_rect.min.x + 10.0, self.playfield_rect.min.y + 48.0);
        
        // Stats to display
        let stats = [
            ("AR", bm.approach_rate),
            ("CS", bm.circle_size),
            ("OD", bm.overall_difficulty),
            ("HP", bm.hp_drain_rate),
        ];
        
        // Calculate BPM from first timing point
        let bpm = bm.control_points.timing_points
            .first()
            .map(|tp| 60000.0 / tp.beat_len)
            .unwrap_or(0.0);
        
        // Background
        let bg_height = 14.0 * (stats.len() + 1) as f32 + 8.0;
        let bg_rect = Rect::from_min_size(pos, Vec2::new(70.0, bg_height));
        painter.rect_filled(bg_rect, 4.0, Color32::from_rgba_unmultiplied(0, 0, 0, 180));
        
        // Draw stats
        let mut y = pos.y + 4.0;
        for (label, value) in stats {
            painter.text(
                Pos2::new(pos.x + 6.0, y),
                egui::Align2::LEFT_TOP,
                format!("{}: {:.1}", label, value),
                FontId::monospace(11.0),
                Color32::WHITE,
            );
            y += 14.0;
        }
        
        // BPM
        painter.text(
            Pos2::new(pos.x + 6.0, y),
            egui::Align2::LEFT_TOP,
            format!("BPM: {:.0}", bpm),
            FontId::monospace(11.0),
            Color32::from_rgb(255, 200, 100),
        );
    }
}

