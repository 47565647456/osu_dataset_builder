//! Spinner rendering

use bevy::prelude::*;

use crate::beatmap::{RenderObject, RenderObjectKind, PLAYFIELD_HEIGHT, PLAYFIELD_WIDTH};
use crate::rendering::PlayfieldTransform;

pub struct SpinnersPlugin;

impl Plugin for SpinnersPlugin {
    fn build(&self, _app: &mut App) {
        // Spinners rendering is now done in the unified render_all_objects system
    }
}

/// Render a single spinner
pub fn render_spinner(
    gizmos: &mut Gizmos,
    obj: &RenderObject,
    opacity: f32,
    current_time: f64,
    transform: &PlayfieldTransform,
) {
    let duration = match &obj.kind {
        RenderObjectKind::Spinner { duration } => *duration,
        _ => return,
    };

    let alpha = opacity;
    let center = transform.osu_to_screen(PLAYFIELD_WIDTH / 2.0, PLAYFIELD_HEIGHT / 2.0);

    // Calculate progress
    let elapsed = (current_time - obj.start_time).max(0.0);
    let progress = (elapsed / duration).min(1.0) as f32;

    // Draw concentric circles
    let max_radius = transform.scale_radius(150.0);
    let spinner_color = Color::srgba(1.0, 1.0, 1.0, alpha);

    for i in 0..3 {
        let radius = max_radius * (0.3 + i as f32 * 0.3);
        gizmos.circle_2d(center, radius, spinner_color);
    }

    // Progress indicator (filled inner circle)
    let inner_color = Color::srgba(1.0, 1.0, 1.0, alpha * progress);
    gizmos.circle_2d(center, max_radius * 0.2 * progress, inner_color);

    // Rotating line indicator
    let angle = (current_time / 50.0).to_radians() as f32;
    let line_end = center + Vec2::new(angle.cos(), angle.sin()) * max_radius;
    gizmos.line_2d(center, line_end, spinner_color);
}
