//! UI module for overlays, timeline, controls, and HUD

mod controls;
mod hud;
mod overlays;
mod timeline;

use bevy::prelude::*;

pub use controls::*;
pub use hud::*;
pub use overlays::*;
pub use timeline::*;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(OverlaysPlugin)
            .add_plugins(HudPlugin)
            .add_plugins(TimelinePlugin)
            .add_plugins(ControlsPlugin);
    }
}
