//! Slider rendering with combo numbers

use bevy::prelude::*;

pub struct SlidersPlugin;

impl Plugin for SlidersPlugin {
    fn build(&self, _app: &mut App) {
        // Sliders rendering is now done in the unified render_all_objects system
    }
}
