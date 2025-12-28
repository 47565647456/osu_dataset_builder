//! Spinner rendering

use bevy::prelude::*;

pub struct SpinnersPlugin;

impl Plugin for SpinnersPlugin {
    fn build(&self, _app: &mut App) {
        // Spinners rendering is now done in the unified render_all_objects system
    }
}

