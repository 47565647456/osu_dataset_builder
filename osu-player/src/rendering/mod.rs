//! Rendering module for osu! hit objects

mod circles;
mod playfield;
pub mod sdf_materials;
pub mod sdf_render;
mod sliders;
mod spinners;

use bevy::prelude::*;

pub use circles::*;
pub use playfield::*;
pub use sdf_materials::SdfMaterialsPlugin;
pub use sdf_render::SdfRenderPlugin;
pub use sliders::*;
pub use spinners::*;

use crate::beatmap::{BeatmapView, RenderObjectKind};
use crate::playback::PlaybackStateRes;

pub struct RenderingPlugin;

impl Plugin for RenderingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(PlayfieldPlugin)
            .add_plugins(CirclesPlugin)
            .add_plugins(SlidersPlugin)
            .add_plugins(SpinnersPlugin)
            .add_plugins(SdfMaterialsPlugin)
            .add_plugins(SdfRenderPlugin)
            .add_systems(Update, render_all_objects);
    }
}

/// Unified rendering system that draws all objects in correct order
/// Objects that should be hit FIRST appear on TOP (drawn last)
/// NOTE: With SDF rendering enabled, this now only renders combo numbers and slider extras
fn render_all_objects(
    mut gizmos: Gizmos,
    beatmap: Res<BeatmapView>,
    playback: Res<PlaybackStateRes>,
    transform: Res<PlayfieldTransform>,
) {
    let current_time = playback.current_time;
    let visible = beatmap.visible_objects(current_time);
    let radius = transform.scale_radius(beatmap.circle_radius);

    // With SDF rendering, we only use gizmos for:
    // - Combo numbers (drawn on top of SDF objects)
    // - Slider ball
    // - Reverse arrows
    // - Spinners (not yet SDF)
    for (_idx, obj, opacity) in visible.iter().rev() {
        match &obj.kind {
            RenderObjectKind::Circle => {
                // Only draw combo number on top of SDF circle
                render_circle_overlay(&mut gizmos, obj, *opacity, radius, &transform);
            }
            RenderObjectKind::Slider { .. } => {
                // Draw combo number and slider extras on top of SDF body
                render_slider_overlay(&mut gizmos, obj, *opacity, radius, current_time, &transform, &beatmap);
            }
            RenderObjectKind::Spinner { .. } => {
                render_spinner(&mut gizmos, obj, *opacity, current_time, &transform);
            }
        }
    }
}

/// Render overlay elements for circles (currently empty - SDF handles rendering, Text2d handles combo numbers)
fn render_circle_overlay(
    _gizmos: &mut Gizmos,
    _obj: &crate::beatmap::RenderObject,
    _opacity: f32,
    _radius: f32,
    _transform: &PlayfieldTransform,
) {
    // Combo numbers are now rendered as Text2d entities in sdf_render.rs
}

/// Render overlay elements for sliders (combo number, slider ball, arrows)
fn render_slider_overlay(
    gizmos: &mut Gizmos,
    obj: &crate::beatmap::RenderObject,
    opacity: f32,
    radius: f32,
    current_time: f64,
    transform: &PlayfieldTransform,
    _beatmap: &BeatmapView,
) {
    let (path_points, repeats, duration) = match &obj.kind {
        RenderObjectKind::Slider { path_points, repeats, duration } => (path_points, *repeats, *duration),
        _ => return,
    };

    if path_points.len() < 2 {
        return;
    }

    // Note: Slider head, tail, ball, and combo numbers are now handled by SDF/Text2d rendering
    // Only reverse arrows use gizmos

    // Reverse arrows - show based on which direction the ball will travel next
    // repeats = 0: no repeats (head -> end)
    // repeats = 1: one repeat (head -> end -> head)
    // repeats = 2: two repeats (head -> end -> head -> end)
    if repeats > 0 {
        // Calculate which pass we're on during active slider
        let elapsed = current_time - obj.start_time;
        let single_pass = duration / (repeats + 1) as f64;
        let current_pass = if elapsed > 0.0 { (elapsed / single_pass) as u32 } else { 0 };
        
        // End arrow (pointing back toward start) - shows when ball is heading to end
        if let Some(&(ex, ey)) = path_points.last() {
            let end_pos = transform.osu_to_screen(ex, ey);
            // Show if we haven't reached the end yet or if there are more passes after
            let show_end_arrow = current_pass < repeats && (elapsed < 0.0 || current_pass % 2 == 0);
            if show_end_arrow {
                let prev = path_points[path_points.len() - 2];
                let prev_pos = transform.osu_to_screen(prev.0, prev.1);
                draw_arrow(gizmos, end_pos, prev_pos, radius * 0.5, opacity);
            }
        }

        // Start arrow (pointing back toward end) - shows when ball is heading back to start
        if repeats >= 2 {
            let start_pos = transform.osu_to_screen(path_points[0].0, path_points[0].1);
            // Show start arrow if there's a repeat coming back and forward again
            let show_start_arrow = current_pass < repeats - 1 && elapsed > 0.0 && current_pass % 2 == 1;
            if show_start_arrow || elapsed < 0.0 {
                let next = path_points[1];
                let next_pos = transform.osu_to_screen(next.0, next.1);
                draw_arrow(gizmos, start_pos, next_pos, radius * 0.5, opacity);
            }
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
