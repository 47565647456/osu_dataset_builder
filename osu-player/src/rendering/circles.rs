//! Hit circle rendering (now handled by SDF)

use bevy::prelude::*;

pub struct CirclesPlugin;

impl Plugin for CirclesPlugin {
    fn build(&self, _app: &mut App) {
        // Circles rendering is now done via SDF in sdf_render.rs
        // Combo numbers are rendered as Text2d entities
        // This plugin is kept for organization but registers nothing
    }
}
