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

/// Resource holding the loaded UI font handle
#[derive(Resource)]
pub struct UiFont(pub Handle<Font>);

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, load_ui_font)
            .add_plugins(OverlaysPlugin)
            .add_plugins(HudPlugin)
            .add_plugins(TimelinePlugin)
            .add_plugins(ControlsPlugin);
    }
}

fn load_ui_font(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font: Handle<Font> = asset_server.load("fonts/NotoSansCJKjp-Regular.otf");
    commands.insert_resource(UiFont(font));
}
