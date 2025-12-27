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

/// Render overlay elements for sliders (currently empty - SDF handles everything including arrows)
fn render_slider_overlay(
    _gizmos: &mut Gizmos,
    _obj: &crate::beatmap::RenderObject,
    _opacity: f32,
    _radius: f32,
    _current_time: f64,
    _transform: &PlayfieldTransform,
    _beatmap: &BeatmapView,
) {
    // All slider elements including arrows are now handled by SDF/Text2d rendering
}

