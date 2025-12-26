//! Slider rendering with combo numbers

use bevy::prelude::*;

use crate::beatmap::{BeatmapView, RenderObject, RenderObjectKind};
use crate::rendering::circles::draw_number_gizmo;
use crate::rendering::PlayfieldTransform;

pub struct SlidersPlugin;

impl Plugin for SlidersPlugin {
    fn build(&self, _app: &mut App) {
        // Sliders rendering is now done in the unified render_all_objects system
    }
}

/// Render a single slider
pub fn render_slider(
    gizmos: &mut Gizmos,
    obj: &RenderObject,
    opacity: f32,
    radius: f32,
    current_time: f64,
    transform: &PlayfieldTransform,
    beatmap: &BeatmapView,
) {
    let (path_points, repeats) = match &obj.kind {
        RenderObjectKind::Slider { path_points, repeats, .. } => (path_points, *repeats),
        _ => return,
    };

    if path_points.len() < 2 {
        return;
    }

    let alpha = opacity;

    // Draw slider body as connected line segments
    let outline_color = Color::srgba(1.0, 1.0, 1.0, alpha);
    let body_color = Color::srgba(0.2, 0.2, 0.3, alpha * 0.8);

    // Sample path points for rendering
    let step = (path_points.len() / 100).max(1);
    let sampled: Vec<Vec2> = path_points
        .iter()
        .step_by(step)
        .map(|(x, y)| transform.osu_to_screen(*x, *y))
        .collect();

    // Draw path lines
    for window in sampled.windows(2) {
        gizmos.line_2d(window[0], window[1], outline_color);
    }

    // Draw start cap (head)
    let head_pos = transform.osu_to_screen(obj.x, obj.y);
    gizmos.circle_2d(head_pos, radius, outline_color);
    gizmos.circle_2d(head_pos, radius * 0.9, body_color);

    // Draw end cap
    if let Some(&(ex, ey)) = path_points.last() {
        let end_pos = transform.osu_to_screen(ex, ey);
        gizmos.circle_2d(end_pos, radius, outline_color);
        gizmos.circle_2d(end_pos, radius * 0.9, body_color);
    }

    // Draw combo number on slider head
    if obj.combo_number > 0 {
        draw_number_gizmo(gizmos, head_pos, obj.combo_number, radius * 0.5, alpha);
    }

    // Approach circle
    let time_until_hit = obj.start_time - current_time;
    if time_until_hit > 0.0 {
        let approach_scale = beatmap.approach_scale(obj, current_time);
        let approach_alpha = alpha * 0.6;
        let approach_color = Color::srgba(1.0, 1.0, 1.0, approach_alpha);
        gizmos.circle_2d(head_pos, radius * approach_scale, approach_color);
    }

    // Slider ball
    if let Some((ball_x, ball_y)) = beatmap.slider_ball_position(obj, current_time) {
        let ball_pos = transform.osu_to_screen(ball_x, ball_y);
        let ball_color = Color::srgba(1.0, 1.0, 1.0, alpha);
        gizmos.circle_2d(ball_pos, radius * 0.6, ball_color);
    }

    // Reverse arrows
    if repeats > 0 {
        // End arrow
        if let Some(&(ex, ey)) = path_points.last() {
            let end_pos = transform.osu_to_screen(ex, ey);
            if path_points.len() >= 2 {
                let prev = path_points[path_points.len() - 2];
                let prev_pos = transform.osu_to_screen(prev.0, prev.1);
                draw_arrow(gizmos, end_pos, prev_pos, radius * 0.5, alpha);
            }
        }
    }

    if repeats >= 2 {
        // Start arrow
        let start_pos = transform.osu_to_screen(path_points[0].0, path_points[0].1);
        if path_points.len() >= 2 {
            let next = path_points[1];
            let next_pos = transform.osu_to_screen(next.0, next.1);
            draw_arrow(gizmos, start_pos, next_pos, radius * 0.5, alpha);
        }
    }
}

/// Draw a chevron arrow pointing from `from` toward `to`
fn draw_arrow(gizmos: &mut Gizmos, from: Vec2, to: Vec2, size: f32, alpha: f32) {
    let dir = (to - from).normalize_or_zero();
    let perp = Vec2::new(-dir.y, dir.x);

    let tip = from + dir * size * 0.3;
    let base1 = tip - dir * size + perp * size * 0.6;
    let base2 = tip - dir * size - perp * size * 0.6;

    let arrow_color = Color::srgba(1.0, 1.0, 1.0, alpha);
    gizmos.line_2d(base1, tip, arrow_color);
    gizmos.line_2d(base2, tip, arrow_color);
}
