//! Rendering module for osu! hit objects

mod circles;
mod playfield;
mod sliders;
mod spinners;

use bevy::prelude::*;

pub use circles::*;
pub use playfield::*;
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
            .add_systems(Update, render_all_objects);
    }
}

/// Unified rendering system that draws all objects in correct order
/// Objects that should be hit FIRST appear on TOP (drawn last)
fn render_all_objects(
    mut gizmos: Gizmos,
    beatmap: Res<BeatmapView>,
    playback: Res<PlaybackStateRes>,
    transform: Res<PlayfieldTransform>,
) {
    let current_time = playback.current_time;
    let visible = beatmap.visible_objects(current_time);
    let radius = transform.scale_radius(beatmap.circle_radius);

    // Iterate in REVERSE order: draw later objects first (they go behind)
    // So that earlier objects (to be hit first) end up on top
    for (_idx, obj, opacity) in visible.iter().rev() {
        match &obj.kind {
            RenderObjectKind::Circle => {
                render_circle(&mut gizmos, obj, *opacity, radius, current_time, &transform, &beatmap);
            }
            RenderObjectKind::Slider { .. } => {
                render_slider(&mut gizmos, obj, *opacity, radius, current_time, &transform, &beatmap);
            }
            RenderObjectKind::Spinner { .. } => {
                render_spinner(&mut gizmos, obj, *opacity, current_time, &transform);
            }
        }
    }
}
